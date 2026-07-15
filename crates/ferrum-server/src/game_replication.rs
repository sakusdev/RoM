use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
    sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use ferrum_game::{GameEvent, PLAYER_INVENTORY_SLOTS, PlayerUuid};
use ferrum_play::EntityProtocolRegistry;

use crate::{
    authoritative_runtime::PlayOutput,
    game_runtime::{GameEventSubscription, SharedGameRuntime},
    play_connection::{PlayOutputSubmitError, PlayReaderEndpoint},
};

const DEFAULT_PENDING_OUTPUT_LIMIT: usize = 256;
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_COMMANDS_PER_POLL: usize = 256;
const MAX_EVENTS_PER_POLL: usize = 1_024;

#[derive(Debug, Clone)]
pub struct GameReplicationConfig {
    pub event_capacity: NonZeroUsize,
    pub command_capacity: NonZeroUsize,
    pub pending_output_limit: NonZeroUsize,
    pub poll_interval: Duration,
    pub entity_protocol_ids: EntityProtocolRegistry,
}

impl Default for GameReplicationConfig {
    fn default() -> Self {
        Self {
            event_capacity: NonZeroUsize::new(4_096).expect("4096 is non-zero"),
            command_capacity: NonZeroUsize::new(1_024).expect("1024 is non-zero"),
            pending_output_limit: NonZeroUsize::new(DEFAULT_PENDING_OUTPUT_LIMIT)
                .expect("pending output limit is non-zero"),
            poll_interval: DEFAULT_POLL_INTERVAL,
            entity_protocol_ids: EntityProtocolRegistry::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameReplicationExit {
    pub commands: u64,
    pub events: u64,
    pub produced_outputs: u64,
    pub sent_outputs: u64,
    pub deferred_outputs: u64,
    pub dropped_outputs: u64,
    pub disconnected_connections: u64,
    pub inventory_snapshots: u64,
    pub rejected_inventory_interactions: u64,
    pub dropped_item_stacks: u64,
}

#[derive(Debug)]
struct ReplicationConnection {
    endpoint: PlayReaderEndpoint,
    pending: VecDeque<PlayOutput>,
    pending_limit: usize,
    next_teleport_id: i32,
}

impl ReplicationConnection {
    fn new(endpoint: PlayReaderEndpoint, pending_limit: usize) -> Self {
        Self {
            endpoint,
            pending: VecDeque::new(),
            pending_limit,
            next_teleport_id: 2,
        }
    }

    fn queue(&mut self, output: PlayOutput, exit: &mut GameReplicationExit) {
        if self.pending.len() == self.pending_limit {
            self.pending.pop_front();
            exit.dropped_outputs = exit.dropped_outputs.saturating_add(1);
        }
        self.pending.push_back(output);
        exit.produced_outputs = exit.produced_outputs.saturating_add(1);
    }

    fn queue_teleport(
        &mut self,
        transform: ferrum_game::Transform,
        exit: &mut GameReplicationExit,
    ) {
        let teleport_id = self.next_teleport_id;
        self.next_teleport_id = self.next_teleport_id.saturating_add(1);
        self.queue(
            PlayOutput::PlayerTeleport {
                teleport_id,
                transform,
            },
            exit,
        );
    }

    fn flush(&mut self, exit: &mut GameReplicationExit) -> bool {
        while let Some(output) = self.pending.pop_front() {
            match self.endpoint.try_submit_output(output) {
                Ok(()) => exit.sent_outputs = exit.sent_outputs.saturating_add(1),
                Err(PlayOutputSubmitError::Full(output)) => {
                    self.pending.push_front(output);
                    return true;
                }
                Err(PlayOutputSubmitError::RuntimeDisconnected(_)) => return false,
            }
        }
        true
    }
}

#[derive(Debug)]
enum ReplicationCommand {
    Register {
        uuid: PlayerUuid,
        endpoint: PlayReaderEndpoint,
        reply: SyncSender<Result<(), String>>,
    },
    SyncInventory {
        uuid: PlayerUuid,
        reply: SyncSender<Result<(), String>>,
    },
    Unregister {
        uuid: PlayerUuid,
        reply: SyncSender<()>,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct GameReplicationControl {
    commands: SyncSender<ReplicationCommand>,
}

impl GameReplicationControl {
    pub fn register(&self, uuid: PlayerUuid, endpoint: PlayReaderEndpoint) -> Result<()> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(ReplicationCommand::Register {
                uuid,
                endpoint,
                reply,
            })
            .context("game replication service is disconnected")?;
        response
            .recv()
            .context("game replication service dropped registration response")?
            .map_err(anyhow::Error::msg)
    }

    pub fn sync_inventory(&self, uuid: PlayerUuid) -> Result<()> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(ReplicationCommand::SyncInventory { uuid, reply })
            .context("game replication service is disconnected")?;
        response
            .recv()
            .context("game replication service dropped inventory sync response")?
            .map_err(anyhow::Error::msg)
    }

    pub fn unregister(&self, uuid: PlayerUuid) -> Result<()> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(ReplicationCommand::Unregister { uuid, reply })
            .context("game replication service is disconnected")?;
        response
            .recv()
            .context("game replication service dropped unregistration response")?;
        Ok(())
    }

    pub fn request_shutdown(&self) {
        let _ = self.commands.send(ReplicationCommand::Shutdown);
    }
}

#[derive(Debug)]
pub struct GameReplicationService {
    control: GameReplicationControl,
    worker: Option<JoinHandle<Result<GameReplicationExit>>>,
}

impl GameReplicationService {
    #[must_use]
    pub fn control(&self) -> GameReplicationControl {
        self.control.clone()
    }

    pub fn shutdown(mut self) -> Result<GameReplicationExit> {
        self.control.request_shutdown();
        self.join_worker()
    }

    fn join_worker(&mut self) -> Result<GameReplicationExit> {
        let worker = self
            .worker
            .take()
            .context("game replication worker was already joined")?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("game replication worker panicked"))?
    }
}

impl Drop for GameReplicationService {
    fn drop(&mut self) {
        self.control.request_shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn spawn_game_replication(
    runtime: &SharedGameRuntime,
    config: GameReplicationConfig,
) -> Result<GameReplicationService> {
    if config.poll_interval.is_zero() {
        bail!("game replication poll interval must be greater than zero");
    }
    if config.pending_output_limit.get() < PLAYER_INVENTORY_SLOTS {
        bail!("game replication pending output limit must be at least {PLAYER_INVENTORY_SLOTS}");
    }
    let subscription = runtime.subscribe(config.event_capacity)?;
    let runtime = runtime.clone();
    let (commands, receiver) = sync_channel(config.command_capacity.get());
    let control = GameReplicationControl { commands };
    let worker = thread::Builder::new()
        .name("rom-game-replication".to_owned())
        .spawn(move || run_replication(runtime, subscription, receiver, config))
        .context("cannot spawn game replication service")?;
    Ok(GameReplicationService {
        control,
        worker: Some(worker),
    })
}

fn run_replication(
    runtime: SharedGameRuntime,
    subscription: GameEventSubscription,
    commands: Receiver<ReplicationCommand>,
    config: GameReplicationConfig,
) -> Result<GameReplicationExit> {
    let mut connections = BTreeMap::new();
    let mut exit = GameReplicationExit::default();
    loop {
        if process_commands(
            &runtime,
            &commands,
            &mut connections,
            config.pending_output_limit.get(),
            &mut exit,
        )? {
            flush_connections(&mut connections, &mut exit);
            exit.deferred_outputs = connections
                .values()
                .map(|connection| connection.pending.len() as u64)
                .sum();
            return Ok(exit);
        }

        match subscription.recv_timeout(config.poll_interval) {
            Ok(event) => {
                dispatch_event(event, &mut connections, &mut exit);
                for _ in 1..MAX_EVENTS_PER_POLL {
                    match subscription.try_recv() {
                        Ok(event) => dispatch_event(event, &mut connections, &mut exit),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            bail!("game event publisher disconnected")
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => bail!("game event publisher disconnected"),
        }
        flush_connections(&mut connections, &mut exit);
    }
}

fn process_commands(
    runtime: &SharedGameRuntime,
    commands: &Receiver<ReplicationCommand>,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    pending_limit: usize,
    exit: &mut GameReplicationExit,
) -> Result<bool> {
    for _ in 0..MAX_COMMANDS_PER_POLL {
        let command = match commands.try_recv() {
            Ok(command) => command,
            Err(TryRecvError::Empty) => return Ok(false),
            Err(TryRecvError::Disconnected) => return Ok(true),
        };
        exit.commands = exit.commands.saturating_add(1);
        match command {
            ReplicationCommand::Register {
                uuid,
                endpoint,
                reply,
            } => {
                let result = match connections.entry(uuid) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(ReplicationConnection::new(endpoint, pending_limit));
                        Ok(())
                    }
                    std::collections::btree_map::Entry::Occupied(_) => Err(format!(
                        "player {uuid:?} is already registered for replication"
                    )),
                };
                let _ = reply.send(result);
            }
            ReplicationCommand::SyncInventory { uuid, reply } => {
                let result = if let Some(connection) = connections.get_mut(&uuid) {
                    runtime
                        .with_state(|state| {
                            state
                                .player(uuid)
                                .map(|player| player.inventory.slots().to_vec())
                        })
                        .map_err(|error| error.to_string())
                        .and_then(|slots| {
                            slots.ok_or_else(|| {
                                format!("player {uuid:?} is missing from authoritative state")
                            })
                        })
                        .and_then(|slots| {
                            if slots.len() != PLAYER_INVENTORY_SLOTS {
                                return Err(format!(
                                    "player inventory has {} slots; expected {PLAYER_INVENTORY_SLOTS}",
                                    slots.len()
                                ));
                            }
                            connection.queue(
                                PlayOutput::SetContainerContent {
                                    container_id: ferrum_game::PLAYER_CONTAINER_ID,
                                    state_id: 0,
                                    slots,
                                    carried: None,
                                },
                                exit,
                            );
                            exit.inventory_snapshots =
                                exit.inventory_snapshots.saturating_add(1);
                            Ok(())
                        })
                } else {
                    Err(format!("player {uuid:?} is not registered for replication"))
                };
                let _ = reply.send(result);
            }
            ReplicationCommand::Unregister { uuid, reply } => {
                connections.remove(&uuid);
                let _ = reply.send(());
            }
            ReplicationCommand::Shutdown => return Ok(true),
        }
    }
    Ok(false)
}

fn dispatch_event(
    event: GameEvent,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    exit: &mut GameReplicationExit,
) {
    exit.events = exit.events.saturating_add(1);
    match event {
        GameEvent::PlayerConnected { uuid, name, .. } => broadcast_except(
            connections,
            uuid,
            PlayOutput::SystemChat {
                message: format!("{name} joined the game"),
                overlay: false,
            },
            exit,
        ),
        GameEvent::PlayerDisconnected { uuid, name, .. } => broadcast_except(
            connections,
            uuid,
            PlayOutput::SystemChat {
                message: format!("{name} left the game"),
                overlay: false,
            },
            exit,
        ),
        GameEvent::Broadcast { message } => broadcast(
            connections,
            PlayOutput::SystemChat {
                message,
                overlay: false,
            },
            exit,
        ),
        GameEvent::PlayerTeleported {
            uuid, transform, ..
        } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue_teleport(transform, exit);
            }
        }
        GameEvent::PlayerGameModeChanged { uuid, current, .. } => target_chat(
            connections,
            uuid,
            format!("Game mode changed to {current:?}"),
            true,
            exit,
        ),
        GameEvent::InventoryChanged {
            uuid,
            inserted,
            item,
        } => target_chat(connections, uuid, format!("+{inserted} {item}"), true, exit),
        GameEvent::InventorySlotChanged { uuid, slot, stack } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(PlayOutput::SetPlayerInventory { slot, stack }, exit);
            }
        }
        GameEvent::ContainerContentChanged { uuid, snapshot } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(
                    PlayOutput::SetContainerContent {
                        container_id: snapshot.container_id,
                        state_id: snapshot.state_id,
                        slots: snapshot.slots,
                        carried: snapshot.carried,
                    },
                    exit,
                );
                exit.inventory_snapshots = exit.inventory_snapshots.saturating_add(1);
            }
        }
        GameEvent::InventoryInteractionRejected {
            uuid,
            reason,
            snapshot,
        } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(
                    PlayOutput::SetContainerContent {
                        container_id: snapshot.container_id,
                        state_id: snapshot.state_id,
                        slots: snapshot.slots,
                        carried: snapshot.carried,
                    },
                    exit,
                );
                connection.queue(
                    PlayOutput::SystemChat {
                        message: format!("Inventory resynchronized: {reason}"),
                        overlay: true,
                    },
                    exit,
                );
                exit.inventory_snapshots = exit.inventory_snapshots.saturating_add(1);
                exit.rejected_inventory_interactions =
                    exit.rejected_inventory_interactions.saturating_add(1);
            }
        }
        GameEvent::ItemsDropped { uuid, stacks } => {
            exit.dropped_item_stacks = exit.dropped_item_stacks.saturating_add(stacks.len() as u64);
            target_chat(
                connections,
                uuid,
                format!("Dropped {} item stack(s)", stacks.len()),
                true,
                exit,
            );
        }
        GameEvent::PlayerKilled { uuid } => {
            target_chat(connections, uuid, "You died".to_owned(), false, exit)
        }
        GameEvent::PlayerMoved { .. }
        | GameEvent::SelectedHotbarChanged { .. }
        | GameEvent::TimeChanged { .. }
        | GameEvent::SaveRequested
        | GameEvent::ShutdownRequested => {}
    }
}

fn target_chat(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    uuid: PlayerUuid,
    message: String,
    overlay: bool,
    exit: &mut GameReplicationExit,
) {
    if let Some(connection) = connections.get_mut(&uuid) {
        connection.queue(PlayOutput::SystemChat { message, overlay }, exit);
    }
}

fn broadcast(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    output: PlayOutput,
    exit: &mut GameReplicationExit,
) {
    for connection in connections.values_mut() {
        connection.queue(output.clone(), exit);
    }
}

fn broadcast_except(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    excluded: PlayerUuid,
    output: PlayOutput,
    exit: &mut GameReplicationExit,
) {
    for (uuid, connection) in connections {
        if *uuid != excluded {
            connection.queue(output.clone(), exit);
        }
    }
}

fn flush_connections(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    exit: &mut GameReplicationExit,
) {
    let previous = connections.len();
    connections.retain(|_, connection| connection.flush(exit));
    exit.disconnected_connections = exit
        .disconnected_connections
        .saturating_add((previous - connections.len()) as u64);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play_connection::register_play_connection;
    use ferrum_game::{CommandSource, Transform};
    use ferrum_runtime::{BoundedInputQueue, ConnectionId, worker_channel};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    fn ingest(
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) {
        workers.ingest_available(inputs, 64).unwrap();
    }

    fn recv_output(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) -> PlayOutput {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            ingest(workers, inputs);
            match writer.try_recv_output() {
                Ok(output) => return output,
                Err(ferrum_runtime::WorkerReceiveError::Empty) => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "replication output timeout"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
                Err(ferrum_runtime::WorkerReceiveError::RuntimeDisconnected) => {
                    panic!("replication runtime disconnected")
                }
            }
        }
    }

    #[test]
    fn broadcasts_chat_and_routes_teleports_only_to_the_target() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, GameReplicationConfig::default()).unwrap();
        let steve = PlayerUuid::new(1);
        let alex = PlayerUuid::new(2);

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(64).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(1),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let (alex_reader, alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(2),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(64).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().register(alex, alex_reader).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex joined the game"
        ));

        game.execute_command(&CommandSource::console(), "/say hello")
            .unwrap();
        game.execute_command(&CommandSource::console(), "/tp Steve 4 70 8")
            .unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, overlay: false } if message == "[Server] hello"
        ));
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::PlayerTeleport { teleport_id: 2, transform }
                if transform.position == [4.0, 70.0, 8.0]
        ));
        let alex_output = recv_output(&alex_writer, &mut workers, &mut inputs);
        let alex_output = match alex_output {
            PlayOutput::SystemChat {
                message,
                overlay: false,
            } if message == "Steve joined the game" => {
                recv_output(&alex_writer, &mut workers, &mut inputs)
            }
            output => output,
        };
        assert!(matches!(
            alex_output,
            PlayOutput::SystemChat { message, overlay: false } if message == "[Server] hello"
        ));
        assert!(alex_writer.try_recv_output().is_err());
        service.shutdown().unwrap();
    }

    #[test]
    fn synchronizes_full_inventory_and_give_slot_changes() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, GameReplicationConfig::default()).unwrap();
        let steve = PlayerUuid::new(30);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(128).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(30),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(128).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().sync_inventory(steve).unwrap();
        assert!(matches!(
            recv_output(&writer, &mut workers, &mut inputs),
            PlayOutput::SetContainerContent {
                container_id: 0,
                state_id: 0,
                slots,
                carried: None,
            } if slots.len() == PLAYER_INVENTORY_SLOTS && slots.iter().all(Option::is_none)
        ));

        game.execute_command(&CommandSource::console(), "/give Steve minecraft:stone 1")
            .unwrap();
        assert!(matches!(
            recv_output(&writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { .. }
        ));
        assert!(matches!(
            recv_output(&writer, &mut workers, &mut inputs),
            PlayOutput::SetPlayerInventory {
                slot: 9,
                stack: Some(stack),
            } if stack.item() == "minecraft:stone" && stack.count() == 1
        ));
        service.shutdown().unwrap();
    }

    #[test]
    fn join_and_leave_messages_exclude_the_subject_connection() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, GameReplicationConfig::default()).unwrap();
        let steve = PlayerUuid::new(3);
        let alex = PlayerUuid::new(4);

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(3),
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
        let (alex_reader, _alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(4),
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().register(alex, alex_reader).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex joined the game"
        ));

        service.control().unregister(alex).unwrap();
        game.disconnect_player(alex).unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex left the game"
        ));
        service.shutdown().unwrap();
    }
}
