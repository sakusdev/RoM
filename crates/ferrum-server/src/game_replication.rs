use std::{collections::VecDeque, num::NonZeroUsize, sync::mpsc::TryRecvError};

use anyhow::{Context, Result};
use ferrum_game::{GameEvent, PlayerUuid};

use crate::{
    authoritative_runtime::PlayOutput,
    game_runtime::{GameEventSubscription, SharedGameRuntime},
    play_connection::{PlayOutputSubmitError, PlayReaderEndpoint},
};

const DEFAULT_PENDING_OUTPUT_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplicationDrainReport {
    pub received_events: usize,
    pub produced_outputs: usize,
    pub sent_outputs: usize,
    pub deferred_outputs: usize,
    pub ignored_events: usize,
}

#[derive(Debug)]
pub struct GameplayReplication {
    player_uuid: PlayerUuid,
    subscription: GameEventSubscription,
    pending: VecDeque<PlayOutput>,
    pending_limit: usize,
}

impl GameplayReplication {
    pub fn subscribe(
        runtime: &SharedGameRuntime,
        player_uuid: PlayerUuid,
        event_capacity: NonZeroUsize,
    ) -> Result<Self> {
        Ok(Self {
            player_uuid,
            subscription: runtime.subscribe(event_capacity)?,
            pending: VecDeque::new(),
            pending_limit: DEFAULT_PENDING_OUTPUT_LIMIT,
        })
    }

    #[cfg(test)]
    fn with_pending_limit(mut self, pending_limit: usize) -> Self {
        self.pending_limit = pending_limit;
        self
    }

    pub fn drain_to(
        &mut self,
        endpoint: &PlayReaderEndpoint,
        event_limit: usize,
    ) -> Result<ReplicationDrainReport> {
        let mut report = ReplicationDrainReport::default();
        if !self.flush_pending(endpoint, &mut report)? {
            report.deferred_outputs = self.pending.len();
            return Ok(report);
        }

        for _ in 0..event_limit {
            let event = match self.subscription.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    anyhow::bail!("gameplay event subscription is disconnected")
                }
            };
            report.received_events = report.received_events.saturating_add(1);
            let outputs = outputs_for_event(self.player_uuid, event);
            if outputs.is_empty() {
                report.ignored_events = report.ignored_events.saturating_add(1);
                continue;
            }
            report.produced_outputs = report.produced_outputs.saturating_add(outputs.len());
            for output in outputs {
                if self.pending.len() == self.pending_limit {
                    self.pending.pop_front();
                }
                self.pending.push_back(output);
            }
            if !self.flush_pending(endpoint, &mut report)? {
                break;
            }
        }
        report.deferred_outputs = self.pending.len();
        Ok(report)
    }

    fn flush_pending(
        &mut self,
        endpoint: &PlayReaderEndpoint,
        report: &mut ReplicationDrainReport,
    ) -> Result<bool> {
        while let Some(output) = self.pending.pop_front() {
            match endpoint.try_submit_output(output) {
                Ok(()) => report.sent_outputs = report.sent_outputs.saturating_add(1),
                Err(PlayOutputSubmitError::Full(output)) => {
                    self.pending.push_front(output);
                    return Ok(false);
                }
                Err(PlayOutputSubmitError::RuntimeDisconnected(_)) => {
                    return Err(anyhow::anyhow!("authoritative Play runtime is disconnected"));
                }
            }
        }
        Ok(true)
    }
}

fn outputs_for_event(player_uuid: PlayerUuid, event: GameEvent) -> Vec<PlayOutput> {
    match event {
        GameEvent::PlayerConnected { name, .. } => vec![PlayOutput::SystemChat {
            message: format!("{name} joined the game"),
            overlay: false,
        }],
        GameEvent::PlayerDisconnected { name, .. } => vec![PlayOutput::SystemChat {
            message: format!("{name} left the game"),
            overlay: false,
        }],
        GameEvent::Broadcast { message } => vec![PlayOutput::SystemChat {
            message,
            overlay: false,
        }],
        GameEvent::PlayerTeleported {
            uuid, transform, ..
        } if uuid == player_uuid => vec![PlayOutput::PlayerTeleport { transform }],
        GameEvent::PlayerGameModeChanged { uuid, current, .. } if uuid == player_uuid => {
            vec![PlayOutput::SystemChat {
                message: format!("Game mode changed to {current:?}"),
                overlay: true,
            }]
        }
        GameEvent::InventoryChanged {
            uuid,
            inserted,
            item,
        } if uuid == player_uuid => vec![PlayOutput::SystemChat {
            message: format!("+{inserted} {item}"),
            overlay: true,
        }],
        GameEvent::PlayerKilled { uuid } if uuid == player_uuid => vec![PlayOutput::SystemChat {
            message: "You died".to_owned(),
            overlay: false,
        }],
        GameEvent::PlayerMoved { .. }
        | GameEvent::PlayerTeleported { .. }
        | GameEvent::PlayerGameModeChanged { .. }
        | GameEvent::InventoryChanged { .. }
        | GameEvent::PlayerKilled { .. }
        | GameEvent::TimeChanged { .. }
        | GameEvent::SaveRequested
        | GameEvent::ShutdownRequested => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        authoritative_runtime::PlayOutput,
        play_connection::register_play_connection,
    };
    use ferrum_game::{CommandSource, Transform};
    use ferrum_runtime::{BoundedInputQueue, ConnectionId, worker_channel};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn broadcasts_chat_and_routes_teleports_only_to_the_target() {
        let game = SharedGameRuntime::vanilla_overworld();
        let steve = PlayerUuid::new(1);
        let alex = PlayerUuid::new(2);
        game.connect_player(steve, "Steve", spawn()).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        let mut replication = GameplayReplication::subscribe(
            &game,
            steve,
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(16).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(1),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(16).unwrap();
        workers.ingest_available(&mut inputs, 1).unwrap();

        game.execute_command(&CommandSource::console(), "/say hello")
            .unwrap();
        game.execute_command(&CommandSource::console(), "/tp Steve 4 70 8")
            .unwrap();
        game.execute_command(&CommandSource::console(), "/tp Alex 9 70 9")
            .unwrap();
        let report = replication.drain_to(&reader, 16).unwrap();
        workers.ingest_available(&mut inputs, 16).unwrap();

        assert_eq!(report.received_events, 3);
        assert_eq!(report.produced_outputs, 2);
        assert!(matches!(
            writer.try_recv_output().unwrap(),
            PlayOutput::SystemChat { message, overlay: false } if message == "[Server] hello"
        ));
        assert!(matches!(
            writer.try_recv_output().unwrap(),
            PlayOutput::PlayerTeleport { transform } if transform.position == [4.0, 70.0, 8.0]
        ));
    }

    #[test]
    fn preserves_reliable_outputs_during_play_queue_backpressure() {
        let game = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(3);
        game.connect_player(uuid, "Notch", spawn()).unwrap();
        let mut replication = GameplayReplication::subscribe(
            &game,
            uuid,
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap()
        .with_pending_limit(8);

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(8).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(2),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(8).unwrap();
        workers.ingest_available(&mut inputs, 1).unwrap();
        game.execute_command(&CommandSource::console(), "/say one")
            .unwrap();
        game.execute_command(&CommandSource::console(), "/say two")
            .unwrap();

        let first = replication.drain_to(&reader, 8).unwrap();
        workers.ingest_available(&mut inputs, 8).unwrap();
        assert_eq!(first.deferred_outputs, 1);
        let _ = writer.try_recv_output().unwrap();

        let second = replication.drain_to(&reader, 8).unwrap();
        workers.ingest_available(&mut inputs, 8).unwrap();
        assert_eq!(second.deferred_outputs, 0);
        assert!(writer.try_recv_output().is_ok());
    }
}
