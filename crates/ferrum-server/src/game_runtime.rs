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

use ferrum_game::{
    CommandError, CommandOutcome, CommandSource, ContainerClick, DamageSource, EquipmentSlot,
    GameEvent, GameSnapshot, GameState, GameStateError, ItemStack, PersistenceError, PlayerUuid,
    StatusEffectInstance, Transform, Velocity, execute_command,
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
        self.publish(&events)?;
        Ok(events)
    }

    pub fn disconnect_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.disconnect_player(uuid)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn move_player(
        &self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.move_player(uuid, transform)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn select_hotbar(
        &self,
        uuid: PlayerUuid,
        selected_hotbar: u8,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.select_hotbar(uuid, selected_hotbar)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn consume_equipped_item(
        &self,
        uuid: PlayerUuid,
        slot: EquipmentSlot,
        expected_item: &str,
        amount: u32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self
            .write()?
            .consume_equipped_item(uuid, slot, expected_item, amount)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn damage_player(
        &self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.damage_player(uuid, amount)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn heal_player(
        &self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.heal_player(uuid, amount)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn kill_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.kill_player(uuid)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn respawn_player(
        &self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.respawn_player(uuid, transform)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn spawn_item_entity(
        &self,
        transform: Transform,
        stack: ItemStack,
        velocity: Velocity,
        owner: Option<PlayerUuid>,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self
            .write()?
            .spawn_item_entity(transform, stack, velocity, owner)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn pickup_nearby_items(
        &self,
        uuid: PlayerUuid,
        radius: f64,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.pickup_nearby_items(uuid, radius)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn damage_player_with_source(
        &self,
        uuid: PlayerUuid,
        amount: f32,
        source: DamageSource,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self
            .write()?
            .damage_player_with_source(uuid, amount, source)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn apply_knockback(
        &self,
        uuid: PlayerUuid,
        direction_xz: [f64; 2],
        strength: f64,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self
            .write()?
            .apply_knockback(uuid, direction_xz, strength)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn set_player_attribute_base(
        &self,
        uuid: PlayerUuid,
        attribute: &str,
        value: f64,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self
            .write()?
            .set_player_attribute_base(uuid, attribute, value)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn add_status_effect(
        &self,
        uuid: PlayerUuid,
        effect: StatusEffectInstance,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.add_status_effect(uuid, effect)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn remove_status_effect(
        &self,
        uuid: PlayerUuid,
        effect: &str,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.remove_status_effect(uuid, effect)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn click_container(
        &self,
        uuid: PlayerUuid,
        click: ContainerClick,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.click_container(uuid, click)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn close_container(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.close_container(uuid)?;
        self.publish(&events)?;
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
        self.publish(&events)?;
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
        self.publish(&outcome.events)?;
        Ok(outcome)
    }

    pub fn tick(&self) -> Result<(), GameRuntimeError> {
        let events = self.write()?.tick();
        self.publish(&events)?;
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
    Command(#[from] CommandError),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_game::{GameMode, PlayerState};
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
}
