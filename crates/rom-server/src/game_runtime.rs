use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    },
    time::Duration,
};

use rom_game::{
    CommandError, CommandOutcome, CommandSource, ContainerClick, GameEvent, GameSnapshot,
    GameState, GameStateError, GameplayTickError, ItemStack, PersistenceError, PlayerUuid,
    Transform, execute_command,
};
use thiserror::Error;

#[derive(Debug)]
struct GameRuntimeInner {
    state: RwLock<GameState>,
    subscribers: Mutex<BTreeMap<u64, SyncSender<GameEvent>>>,
    next_subscriber_id: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct SharedGameRuntime {
    inner: Arc<GameRuntimeInner>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameEventDispatchReport {
    pub events: usize,
    pub delivered: usize,
    pub full: usize,
    pub disconnected: usize,
}

#[derive(Debug)]
pub struct GameEventSubscription {
    receiver: Receiver<GameEvent>,
}

impl GameEventSubscription {
    pub fn try_recv(&self) -> Result<GameEvent, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<GameEvent, std::sync::mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

impl SharedGameRuntime {
    #[must_use]
    pub fn new(state: GameState) -> Self {
        Self {
            inner: Arc::new(GameRuntimeInner {
                state: RwLock::new(state),
                subscribers: Mutex::new(BTreeMap::new()),
                next_subscriber_id: AtomicU64::new(1),
            }),
        }
    }

    #[must_use]
    pub fn vanilla_overworld() -> Self {
        Self::new(GameState::default())
    }

    pub fn subscribe(
        &self,
        capacity: NonZeroUsize,
    ) -> Result<GameEventSubscription, GameRuntimeError> {
        let (sender, receiver) = sync_channel(capacity.get());
        let id = self
            .inner
            .next_subscriber_id
            .fetch_add(1, Ordering::Relaxed);
        self.subscribers()?.insert(id, sender);
        Ok(GameEventSubscription { receiver })
    }

    pub fn connect_player(
        &self,
        uuid: PlayerUuid,
        name: impl Into<String>,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.connect_player(uuid, name, transform)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn disconnect_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.disconnect_player(uuid)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn move_player(
        &self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.move_player_with_gameplay(uuid, transform)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn select_hotbar(
        &self,
        uuid: PlayerUuid,
        selected_hotbar: u8,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.select_hotbar(uuid, selected_hotbar)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn damage_player(
        &self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.damage_player(uuid, amount)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn heal_player(
        &self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.heal_player(uuid, amount)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn kill_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.kill_player(uuid)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn respawn_player(
        &self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.respawn_player(uuid, transform)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn click_container(
        &self,
        uuid: PlayerUuid,
        click: ContainerClick,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.click_container(uuid, click)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn close_container(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.close_container(uuid)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn set_creative_inventory_slot(
        &self,
        uuid: PlayerUuid,
        slot: i16,
        stack: Option<ItemStack>,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self
            .write()?
            .set_creative_inventory_slot(uuid, slot, stack)?;
        self.finalize_events(&events)?;
        Ok(events)
    }

    pub fn execute_command(
        &self,
        source: &CommandSource,
        input: &str,
    ) -> Result<CommandOutcome, GameRuntimeError> {
        let outcome = {
            let mut state = self.write()?;
            execute_command(&mut state, source, input)?
        };
        self.finalize_events(&outcome.events)?;
        Ok(outcome)
    }

    pub fn tick(&self) -> Result<(), GameRuntimeError> {
        let events = {
            let mut state = self.write()?;
            let mut events = state.tick_player_gameplay()?;
            events.extend(state.tick_gameplay()?.events);
            events
        };
        self.finalize_events(&events)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<GameSnapshot, GameRuntimeError> {
        Ok(self.read()?.snapshot())
    }

    pub fn replace_from_snapshot(&self, snapshot: GameSnapshot) -> Result<(), GameRuntimeError> {
        let restored = GameState::restore(snapshot)?;
        *self.write()? = restored;
        Ok(())
    }

    pub fn with_state<T>(
        &self,
        operation: impl FnOnce(&GameState) -> T,
    ) -> Result<T, GameRuntimeError> {
        let state = self.read()?;
        Ok(operation(&state))
    }

    pub fn with_state_mut<T>(
        &self,
        operation: impl FnOnce(&mut GameState) -> Result<T, GameRuntimeError>,
    ) -> Result<T, GameRuntimeError> {
        let mut state = self.write()?;
        operation(&mut state)
    }

    pub fn publish(
        &self,
        events: &[GameEvent],
    ) -> Result<GameEventDispatchReport, GameRuntimeError> {
        let mut report = GameEventDispatchReport {
            events: events.len(),
            ..GameEventDispatchReport::default()
        };
        if events.is_empty() {
            return Ok(report);
        }
        let mut subscribers = self.subscribers()?;
        subscribers.retain(|_, sender| {
            for event in events {
                match sender.try_send(event.clone()) {
                    Ok(()) => report.delivered = report.delivered.saturating_add(1),
                    Err(TrySendError::Full(_)) => {
                        report.full = report.full.saturating_add(1);
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        report.disconnected = report.disconnected.saturating_add(1);
                        return false;
                    }
                }
            }
            true
        });
        Ok(report)
    }

    fn finalize_events(&self, events: &[GameEvent]) -> Result<(), GameRuntimeError> {
        if events
            .iter()
            .any(|event| matches!(event, GameEvent::ItemsDropped { .. }))
        {
            self.write()?.materialize_drop_events(events)?;
        }
        self.publish(events)?;
        Ok(())
    }

    fn read(&self) -> Result<std::sync::RwLockReadGuard<'_, GameState>, GameRuntimeError> {
        self.inner
            .state
            .read()
            .map_err(|_| GameRuntimeError::PoisonedState)
    }

    fn write(&self) -> Result<std::sync::RwLockWriteGuard<'_, GameState>, GameRuntimeError> {
        self.inner
            .state
            .write()
            .map_err(|_| GameRuntimeError::PoisonedState)
    }

    fn subscribers(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<u64, SyncSender<GameEvent>>>, GameRuntimeError>
    {
        self.inner
            .subscribers
            .lock()
            .map_err(|_| GameRuntimeError::PoisonedSubscribers)
    }
}

#[derive(Debug, Error)]
pub enum GameRuntimeError {
    #[error("shared game-state lock is poisoned")]
    PoisonedState,
    #[error("shared game-event subscriber lock is poisoned")]
    PoisonedSubscribers,
    #[error(transparent)]
    State(#[from] GameStateError),
    #[error(transparent)]
    Gameplay(#[from] GameplayTickError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rom_game::{GameMode, PlayerState};
    use std::thread;

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn shares_players_and_commands_across_runtime_clones() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(1);
        runtime.connect_player(uuid, "Steve", spawn()).unwrap();
        let clone = runtime.clone();
        thread::spawn(move || {
            clone
                .execute_command(&CommandSource::console(), "/gamemode creative Steve")
                .unwrap();
        })
        .join()
        .unwrap();

        let game_mode = runtime
            .with_state(|state| {
                state
                    .player(uuid)
                    .map(|player: &PlayerState| player.game_mode)
            })
            .unwrap();
        assert_eq!(game_mode, Some(GameMode::Creative));
    }

    #[test]
    fn replaces_state_from_validated_snapshot() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(2);
        runtime.connect_player(uuid, "Alex", spawn()).unwrap();
        let snapshot = runtime.snapshot().unwrap();

        runtime.disconnect_player(uuid).unwrap();
        assert_eq!(
            runtime.with_state(GameState::online_player_count).unwrap(),
            0
        );
        runtime.replace_from_snapshot(snapshot).unwrap();
        assert_eq!(
            runtime.with_state(GameState::online_player_count).unwrap(),
            1
        );
    }

    #[test]
    fn broadcasts_mutations_to_bounded_subscribers() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let subscription = runtime.subscribe(NonZeroUsize::new(4).unwrap()).unwrap();
        let uuid = PlayerUuid::new(3);
        runtime.connect_player(uuid, "Notch", spawn()).unwrap();
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerConnected { uuid: event_uuid, .. } if event_uuid == uuid
        ));
    }

    #[test]
    fn full_subscribers_do_not_block_gameplay_mutations() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let _subscription = runtime.subscribe(NonZeroUsize::new(1).unwrap()).unwrap();
        let first = runtime
            .publish(&[
                GameEvent::Broadcast {
                    message: "one".to_owned(),
                },
                GameEvent::Broadcast {
                    message: "two".to_owned(),
                },
            ])
            .unwrap();
        assert_eq!(first.delivered, 1);
        assert_eq!(first.full, 1);
    }

    #[test]
    fn disconnected_subscribers_are_pruned() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let subscription = runtime.subscribe(NonZeroUsize::new(1).unwrap()).unwrap();
        drop(subscription);
        let report = runtime
            .publish(&[GameEvent::Broadcast {
                message: "test".to_owned(),
            }])
            .unwrap();
        assert_eq!(report.disconnected, 1);
    }

    #[test]
    fn publishes_damage_vitals_and_death_events() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let subscription = runtime.subscribe(NonZeroUsize::new(8).unwrap()).unwrap();
        let uuid = PlayerUuid::new(40);
        runtime.connect_player(uuid, "Steve", spawn()).unwrap();
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerConnected { .. }
        ));
        runtime.damage_player(uuid, 20.0).unwrap();
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerDamaged { .. }
        ));
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerVitalsChanged { vitals, .. } if vitals.health == 0.0
        ));
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerKilled { .. }
        ));
    }

    #[test]
    fn death_drops_are_materialized_as_world_entities() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(41);
        runtime.connect_player(uuid, "Herobrine", spawn()).unwrap();
        runtime
            .with_state_mut(|state| {
                state.give_item(uuid, ItemStack::new("minecraft:stone", 3).unwrap())?;
                Ok(())
            })
            .unwrap();
        runtime.kill_player(uuid).unwrap();
        let item_count = runtime
            .with_state(|state| {
                state
                    .entities()
                    .iter()
                    .filter(|(_, entity)| entity.entity_type.as_str() == "minecraft:item")
                    .count()
            })
            .unwrap();
        assert_eq!(item_count, 1);
    }
}
