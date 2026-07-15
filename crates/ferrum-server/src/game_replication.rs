use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
    sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use ferrum_game::{
    EntityId, EquipmentSlot, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerState,
    PlayerUuid, Transform, Velocity, Vitals,
};
use ferrum_play::{
    DataComponentProtocolRegistry, EncodedEntityMovement, EntityMovementKind,
    EntityProtocolRegistry, EquipmentEntry, ItemProtocolRegistry, PlayerInfoEntry,
    encode_add_entity, encode_empty_entity_data, encode_entity_movement, encode_player_info_remove,
    encode_player_info_update, encode_remove_entities, encode_rotate_head, encode_set_equipment,
    encode_set_health, encode_teleport_entity,
};
use ferrum_protocol::PacketKind;

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
    pub item_protocol_ids: ItemProtocolRegistry,
    pub data_component_protocol_ids: DataComponentProtocolRegistry,
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
            item_protocol_ids: ItemProtocolRegistry::default(),
            data_component_protocol_ids: DataComponentProtocolRegistry::default(),
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

#[derive(Debug, Clone)]
struct PlayerEntitySnapshot {
    uuid: PlayerUuid,
    name: String,
    entity_id: EntityId,
    game_mode: GameMode,
    transform: Transform,
    velocity: Velocity,
    equipment: Vec<EquipmentEntry>,
    selected_hotbar: u8,
}

#[derive(Debug)]
struct ReplicationConnection {
    endpoint: PlayReaderEndpoint,
    pending: VecDeque<PlayOutput>,
    pending_limit: usize,
    next_teleport_id: i32,
    entities: BTreeMap<PlayerUuid, PlayerEntitySnapshot>,
}

impl ReplicationConnection {
    fn new(endpoint: PlayReaderEndpoint, pending_limit: usize) -> Self {
        Self {
            endpoint,
            pending: VecDeque::new(),
            pending_limit,
            next_teleport_id: 2,
            entities: BTreeMap::new(),
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
        if process_commands(&runtime, &commands, &mut connections, &config, &mut exit)? {
            flush_connections(&mut connections, &mut exit);
            exit.deferred_outputs = connections
                .values()
                .map(|connection| connection.pending.len() as u64)
                .sum();
            return Ok(exit);
        }

        match subscription.recv_timeout(config.poll_interval) {
            Ok(event) => {
                dispatch_event(event, &runtime, &config, &mut connections, &mut exit)?;
                for _ in 1..MAX_EVENTS_PER_POLL {
                    match subscription.try_recv() {
                        Ok(event) => {
                            dispatch_event(event, &runtime, &config, &mut connections, &mut exit)?
                        }
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
    config: &GameReplicationConfig,
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
                if connections.contains_key(&uuid) {
                    let _ = reply.send(Err(format!(
                        "player {uuid:?} is already registered for replication"
                    )));
                    continue;
                }

                let mut connection =
                    ReplicationConnection::new(endpoint, config.pending_output_limit.get());
                if entity_replication_enabled(&config.entity_protocol_ids) {
                    let initialization = online_player_snapshots(runtime).and_then(|snapshots| {
                        for snapshot in snapshots {
                            if snapshot.uuid != uuid {
                                queue_player_spawn(&mut connection, snapshot, config, exit)?;
                            }
                        }
                        Ok(())
                    });
                    if let Err(error) = initialization {
                        let _ = reply.send(Err(error.to_string()));
                        return Err(error);
                    }
                }
                connections.insert(uuid, connection);
                let _ = reply.send(Ok(()));
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
    runtime: &SharedGameRuntime,
    config: &GameReplicationConfig,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    exit.events = exit.events.saturating_add(1);
    match event {
        GameEvent::PlayerConnected { uuid, name, .. } => {
            if let Some(vitals) = runtime
                .with_state(|state| state.player(uuid).map(|player| player.vitals))
                .context("cannot read connected player vitals")?
            {
                if let Some(connection) = connections.get_mut(&uuid) {
                    queue_set_health(connection, vitals, exit)?;
                }
            }
            if entity_replication_enabled(&config.entity_protocol_ids) {
                let snapshot = player_snapshot(runtime, uuid)?.with_context(|| {
                    format!("connected player {uuid:?} is missing from authoritative state")
                })?;
                if let Some(connection) = connections.get_mut(&uuid) {
                    queue_player_info_update(connection, &snapshot, exit)?;
                }
                for (target, connection) in connections.iter_mut() {
                    if *target != uuid {
                        queue_player_spawn(connection, snapshot.clone(), config, exit)?;
                    }
                }
            }
            broadcast_except(
                connections,
                uuid,
                PlayOutput::SystemChat {
                    message: format!("{name} joined the game"),
                    overlay: false,
                },
                exit,
            );
        }
        GameEvent::PlayerDisconnected {
            uuid,
            name,
            entity_id,
        } => {
            if entity_replication_enabled(&config.entity_protocol_ids) {
                for (target, connection) in connections.iter_mut() {
                    if *target != uuid {
                        queue_player_remove(connection, uuid, entity_id, exit)?;
                    }
                }
            } else {
                for connection in connections.values_mut() {
                    connection.entities.remove(&uuid);
                }
            }
            broadcast_except(
                connections,
                uuid,
                PlayOutput::SystemChat {
                    message: format!("{name} left the game"),
                    overlay: false,
                },
                exit,
            );
        }
        GameEvent::Broadcast { message } => broadcast(
            connections,
            PlayOutput::SystemChat {
                message,
                overlay: false,
            },
            exit,
        ),
        GameEvent::PlayerMoved {
            uuid,
            entity_id,
            transform,
        } => {
            if entity_replication_enabled(&config.entity_protocol_ids) {
                let snapshot = player_snapshot(runtime, uuid)?;
                for (target, connection) in connections.iter_mut() {
                    if *target == uuid {
                        continue;
                    }
                    match connection.entities.get(&uuid).cloned() {
                        Some(tracked) if tracked.entity_id == entity_id => {
                            queue_player_movement(connection, &tracked, transform, exit)?;
                        }
                        Some(tracked) => {
                            queue_player_remove(connection, uuid, Some(tracked.entity_id), exit)?;
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(connection, snapshot, config, exit)?;
                            }
                        }
                        None => {
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(connection, snapshot, config, exit)?;
                            }
                        }
                    }
                }
            }
        }
        GameEvent::PlayerTeleported {
            uuid,
            entity_id,
            transform,
        } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue_teleport(transform, exit);
            }
            if entity_replication_enabled(&config.entity_protocol_ids) {
                let snapshot = player_snapshot(runtime, uuid)?;
                for (target, connection) in connections.iter_mut() {
                    if *target == uuid {
                        continue;
                    }
                    match connection.entities.get(&uuid).cloned() {
                        Some(tracked) if tracked.entity_id == entity_id => {
                            queue_player_absolute_teleport(connection, &tracked, transform, exit)?;
                        }
                        Some(tracked) => {
                            queue_player_remove(connection, uuid, Some(tracked.entity_id), exit)?;
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(connection, snapshot, config, exit)?;
                            }
                        }
                        None => {
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(connection, snapshot, config, exit)?;
                            }
                        }
                    }
                }
            }
        }
        GameEvent::PlayerGameModeChanged { uuid, current, .. } => {
            target_chat(
                connections,
                uuid,
                format!("Game mode changed to {current:?}"),
                true,
                exit,
            );
            if entity_replication_enabled(&config.entity_protocol_ids) {
                if let Some(snapshot) = player_snapshot(runtime, uuid)? {
                    for connection in connections.values_mut() {
                        queue_player_info_update(connection, &snapshot, exit)?;
                        if let Some(tracked) = connection.entities.get_mut(&uuid) {
                            tracked.game_mode = current;
                        }
                    }
                }
            }
        }
        GameEvent::InventoryChanged {
            uuid,
            inserted,
            item,
        } => target_chat(connections, uuid, format!("+{inserted} {item}"), true, exit),
        GameEvent::InventorySlotChanged { uuid, slot, stack } => {
            let equipment_update = if entity_replication_enabled(&config.entity_protocol_ids) {
                player_snapshot(runtime, uuid)?.and_then(|snapshot| {
                    equipment_slot_for_inventory_index(slot, snapshot.selected_hotbar).map(
                        |equipment_slot| {
                            (
                                snapshot.entity_id,
                                EquipmentEntry::new(equipment_slot, stack.clone()),
                            )
                        },
                    )
                })
            } else {
                None
            };
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(
                    PlayOutput::SetPlayerInventory {
                        slot,
                        stack: stack.clone(),
                    },
                    exit,
                );
            }
            if let Some((entity_id, entry)) = equipment_update {
                queue_equipment_except(connections, uuid, entity_id, &[entry], config, exit)?;
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
        GameEvent::PlayerDamaged { .. } => {}
        GameEvent::PlayerVitalsChanged { uuid, vitals } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                queue_set_health(connection, vitals, exit)?;
            }
        }
        GameEvent::PlayerKilled { uuid } => {
            target_chat(connections, uuid, "You died".to_owned(), false, exit)
        }
        GameEvent::SelectedHotbarChanged { uuid, current, .. } => {
            if entity_replication_enabled(&config.entity_protocol_ids) {
                if let Some(snapshot) = player_snapshot(runtime, uuid)? {
                    let entry = snapshot
                        .equipment
                        .iter()
                        .find(|entry| entry.slot == EquipmentSlot::MainHand)
                        .cloned()
                        .unwrap_or_else(|| EquipmentEntry::new(EquipmentSlot::MainHand, None));
                    debug_assert_eq!(snapshot.selected_hotbar, current);
                    queue_equipment_except(
                        connections,
                        uuid,
                        snapshot.entity_id,
                        &[entry],
                        config,
                        exit,
                    )?;
                }
            }
        }
        GameEvent::TimeChanged { .. } | GameEvent::SaveRequested | GameEvent::ShutdownRequested => {
        }
    }
    Ok(())
}

fn entity_replication_enabled(registry: &EntityProtocolRegistry) -> bool {
    registry.protocol_id("minecraft:player").is_some()
}

fn player_snapshot(
    runtime: &SharedGameRuntime,
    uuid: PlayerUuid,
) -> Result<Option<PlayerEntitySnapshot>> {
    runtime
        .with_state(|state| player_snapshot_from_state(state, uuid))
        .context("cannot read authoritative player snapshot")?
}

fn online_player_snapshots(runtime: &SharedGameRuntime) -> Result<Vec<PlayerEntitySnapshot>> {
    runtime
        .with_state(|state| {
            let mut snapshots = Vec::new();
            for player in state.players().values().filter(|player| player.connected) {
                let snapshot =
                    player_snapshot_from_state(state, player.uuid)?.with_context(|| {
                        format!("online player {:?} has no entity snapshot", player.uuid)
                    })?;
                snapshots.push(snapshot);
            }
            Ok(snapshots)
        })
        .context("cannot read authoritative online-player snapshots")?
}

fn player_snapshot_from_state(
    state: &GameState,
    uuid: PlayerUuid,
) -> Result<Option<PlayerEntitySnapshot>> {
    let Some(player) = state.player(uuid) else {
        return Ok(None);
    };
    if !player.connected {
        return Ok(None);
    }
    let entity_id = player
        .entity_id
        .with_context(|| format!("connected player {uuid:?} has no entity id"))?;
    let entity = state
        .entities()
        .get(entity_id)
        .with_context(|| format!("player {uuid:?} entity {entity_id:?} is missing"))?;
    Ok(Some(PlayerEntitySnapshot {
        uuid,
        name: player.name.clone(),
        entity_id,
        game_mode: player.game_mode,
        transform: entity.transform,
        velocity: entity.velocity,
        equipment: player_equipment(player),
        selected_hotbar: player.inventory.selected_hotbar(),
    }))
}

fn queue_player_info_update(
    connection: &mut ReplicationConnection,
    snapshot: &PlayerEntitySnapshot,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    let payload = encode_player_info_update(&[PlayerInfoEntry::new(
        snapshot.uuid,
        snapshot.name.clone(),
        snapshot.game_mode,
    )])
    .context("cannot encode player info update")?;
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::PlayerInfoUpdate,
            payload,
        },
        exit,
    );
    Ok(())
}

fn queue_player_spawn(
    connection: &mut ReplicationConnection,
    snapshot: PlayerEntitySnapshot,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !entity_replication_enabled(&config.entity_protocol_ids) {
        return Ok(());
    }
    queue_player_info_update(connection, &snapshot, exit)?;
    let payload = encode_add_entity(
        snapshot.entity_id,
        snapshot.uuid,
        "minecraft:player",
        snapshot.transform,
        snapshot.velocity,
        &config.entity_protocol_ids,
    )
    .context("cannot encode player add-entity packet")?
    .context("player entity protocol id is unavailable")?;
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::AddEntity,
            payload,
        },
        exit,
    );
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetEntityData,
            payload: encode_empty_entity_data(snapshot.entity_id)
                .context("cannot encode empty player entity data")?,
        },
        exit,
    );
    if snapshot.equipment.iter().any(|entry| entry.stack.is_some()) {
        queue_player_equipment(
            connection,
            snapshot.entity_id,
            &snapshot.equipment,
            config,
            exit,
        )?;
    }
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::RotateHead,
            payload: encode_rotate_head(snapshot.entity_id, snapshot.transform.yaw)
                .context("cannot encode initial player head rotation")?,
        },
        exit,
    );
    connection.entities.insert(snapshot.uuid, snapshot);
    Ok(())
}

fn player_equipment(player: &PlayerState) -> Vec<EquipmentEntry> {
    [
        EquipmentSlot::MainHand,
        EquipmentSlot::OffHand,
        EquipmentSlot::Feet,
        EquipmentSlot::Legs,
        EquipmentSlot::Chest,
        EquipmentSlot::Head,
    ]
    .into_iter()
    .map(|slot| EquipmentEntry::new(slot, player.inventory.equipment(slot).cloned()))
    .collect()
}

fn equipment_slot_for_inventory_index(
    inventory_index: usize,
    selected_hotbar: u8,
) -> Option<EquipmentSlot> {
    [
        EquipmentSlot::MainHand,
        EquipmentSlot::OffHand,
        EquipmentSlot::Feet,
        EquipmentSlot::Legs,
        EquipmentSlot::Chest,
        EquipmentSlot::Head,
    ]
    .into_iter()
    .find(|slot| slot.inventory_index(selected_hotbar) == inventory_index)
}

fn queue_player_equipment(
    connection: &mut ReplicationConnection,
    entity_id: EntityId,
    entries: &[EquipmentEntry],
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    let Some(payload) = encode_set_equipment(
        entity_id,
        entries,
        &config.item_protocol_ids,
        &config.data_component_protocol_ids,
    )
    .context("cannot encode player equipment")?
    else {
        return Ok(());
    };
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetEquipment,
            payload,
        },
        exit,
    );
    Ok(())
}

fn queue_equipment_except(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    excluded: PlayerUuid,
    entity_id: EntityId,
    entries: &[EquipmentEntry],
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    for (uuid, connection) in connections {
        if *uuid != excluded && connection.entities.contains_key(&excluded) {
            queue_player_equipment(connection, entity_id, entries, config, exit)?;
        }
    }
    Ok(())
}

fn queue_player_remove(
    connection: &mut ReplicationConnection,
    uuid: PlayerUuid,
    fallback_entity_id: Option<EntityId>,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    let tracked = connection.entities.remove(&uuid);
    if let Some(entity_id) = tracked
        .as_ref()
        .map(|snapshot| snapshot.entity_id)
        .or(fallback_entity_id)
    {
        connection.queue(
            PlayOutput::ProtocolPacket {
                kind: PacketKind::RemoveEntities,
                payload: encode_remove_entities(&[entity_id])
                    .context("cannot encode remove-player entity packet")?,
            },
            exit,
        );
    }
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::PlayerInfoRemove,
            payload: encode_player_info_remove(&[uuid])
                .context("cannot encode player info removal")?,
        },
        exit,
    );
    Ok(())
}

fn queue_player_movement(
    connection: &mut ReplicationConnection,
    tracked: &PlayerEntitySnapshot,
    transform: Transform,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if let Some(movement) = encode_entity_movement(tracked.entity_id, tracked.transform, transform)
        .context("cannot encode player entity movement")?
    {
        queue_encoded_movement(connection, movement, exit);
    }
    if tracked.transform.yaw.to_bits() != transform.yaw.to_bits() {
        connection.queue(
            PlayOutput::ProtocolPacket {
                kind: PacketKind::RotateHead,
                payload: encode_rotate_head(tracked.entity_id, transform.yaw)
                    .context("cannot encode player head rotation")?,
            },
            exit,
        );
    }
    if let Some(snapshot) = connection.entities.get_mut(&tracked.uuid) {
        snapshot.transform = transform;
    }
    Ok(())
}

fn queue_encoded_movement(
    connection: &mut ReplicationConnection,
    movement: EncodedEntityMovement,
    exit: &mut GameReplicationExit,
) {
    let kind = match movement.kind {
        EntityMovementKind::Position => PacketKind::MoveEntityPosition,
        EntityMovementKind::PositionRotation => PacketKind::MoveEntityPositionRotation,
        EntityMovementKind::Rotation => PacketKind::MoveEntityRotation,
        EntityMovementKind::Teleport => PacketKind::TeleportEntity,
    };
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind,
            payload: movement.payload,
        },
        exit,
    );
}

fn queue_player_absolute_teleport(
    connection: &mut ReplicationConnection,
    tracked: &PlayerEntitySnapshot,
    transform: Transform,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::TeleportEntity,
            payload: encode_teleport_entity(tracked.entity_id, transform, tracked.velocity)
                .context("cannot encode absolute player entity teleport")?,
        },
        exit,
    );
    if tracked.transform.yaw.to_bits() != transform.yaw.to_bits() {
        connection.queue(
            PlayOutput::ProtocolPacket {
                kind: PacketKind::RotateHead,
                payload: encode_rotate_head(tracked.entity_id, transform.yaw)
                    .context("cannot encode teleported player head rotation")?,
            },
            exit,
        );
    }
    if let Some(snapshot) = connection.entities.get_mut(&tracked.uuid) {
        snapshot.transform = transform;
    }
    Ok(())
}

fn queue_set_health(
    connection: &mut ReplicationConnection,
    vitals: Vitals,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetHealth,
            payload: encode_set_health(vitals).context("cannot encode player health")?,
        },
        exit,
    );
    Ok(())
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
    use ferrum_game::{CommandSource, HOTBAR_START, ItemStack, Transform};
    use ferrum_runtime::{BoundedInputQueue, ConnectionId, worker_channel};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    fn entity_config() -> GameReplicationConfig {
        GameReplicationConfig {
            entity_protocol_ids: EntityProtocolRegistry::new([("minecraft:player", 148)]).unwrap(),
            item_protocol_ids: ItemProtocolRegistry::new([("minecraft:stone", 1)]).unwrap(),
            ..GameReplicationConfig::default()
        }
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

    fn recv_raw_output(
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

    fn recv_output(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) -> PlayOutput {
        loop {
            let output = recv_raw_output(writer, workers, inputs);
            if matches!(
                output,
                PlayOutput::ProtocolPacket {
                    kind: PacketKind::SetHealth,
                    ..
                }
            ) {
                continue;
            }
            return output;
        }
    }

    fn recv_protocol(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
        expected: PacketKind,
    ) -> Vec<u8> {
        match recv_output(writer, workers, inputs) {
            PlayOutput::ProtocolPacket { kind, payload } => {
                assert_eq!(kind, expected);
                payload
            }
            output => panic!("expected {expected:?} protocol packet, got {output:?}"),
        }
    }

    fn read_varint(bytes: &[u8]) -> (i32, usize) {
        let mut value = 0_i32;
        for (index, byte) in bytes.iter().copied().enumerate().take(5) {
            value |= i32::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                return (value, index + 1);
            }
        }
        panic!("invalid VarInt payload")
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

    #[test]
    fn synchronizes_player_entity_lifecycle_in_protocol_order() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let steve = PlayerUuid::new(101);
        let alex = PlayerUuid::new(102);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(256).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(101),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let (alex_reader, alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(102),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(256).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );
        let steve_entity_id = game
            .with_state(|state| state.player(steve).and_then(|player| player.entity_id))
            .unwrap()
            .unwrap();

        service.control().register(alex, alex_reader).unwrap();
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );
        let add_payload = recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::AddEntity,
        );
        assert_eq!(read_varint(&add_payload).0, steve_entity_id.get() as i32);
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RotateHead,
        );

        game.connect_player(alex, "Alex", spawn()).unwrap();
        let alex_entity_id = game
            .with_state(|state| state.player(alex).and_then(|player| player.entity_id))
            .unwrap()
            .unwrap();
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );
        let add_payload = recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::AddEntity,
        );
        assert_eq!(read_varint(&add_payload).0, alex_entity_id.get() as i32);
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RotateHead,
        );
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, overlay: false } if message == "Alex joined the game"
        ));
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );

        service.control().unregister(alex).unwrap();
        game.disconnect_player(alex).unwrap();
        let remove_payload = recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );
        let (count, consumed) = read_varint(&remove_payload);
        assert_eq!(count, 1);
        assert_eq!(
            read_varint(&remove_payload[consumed..]).0,
            alex_entity_id.get() as i32
        );
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoRemove,
        );
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, overlay: false } if message == "Alex left the game"
        ));
        service.shutdown().unwrap();
    }

    #[test]
    fn replicates_relative_rotation_and_large_distance_player_movement() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let steve = PlayerUuid::new(201);
        let alex = PlayerUuid::new(202);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(256).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(201),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let (alex_reader, alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(202),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(256).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );
        let steve_entity_id = game
            .with_state(|state| state.player(steve).and_then(|player| player.entity_id))
            .unwrap()
            .unwrap();
        service.control().register(alex, alex_reader).unwrap();
        for kind in [
            PacketKind::PlayerInfoUpdate,
            PacketKind::AddEntity,
            PacketKind::SetEntityData,
            PacketKind::RotateHead,
        ] {
            recv_protocol(&alex_writer, &mut workers, &mut inputs, kind);
        }
        game.connect_player(alex, "Alex", spawn()).unwrap();
        for kind in [
            PacketKind::PlayerInfoUpdate,
            PacketKind::AddEntity,
            PacketKind::SetEntityData,
            PacketKind::RotateHead,
        ] {
            recv_protocol(&steve_writer, &mut workers, &mut inputs, kind);
        }
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { .. }
        ));
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );

        game.move_player(
            steve,
            Transform::new([1.0, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
        )
        .unwrap();
        let payload = recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::MoveEntityPosition,
        );
        assert_eq!(read_varint(&payload).0, steve_entity_id.get() as i32);
        assert!(steve_writer.try_recv_output().is_err());

        game.move_player(
            steve,
            Transform::new([1.0, 65.0, 0.5], 90.0, 0.0, true).unwrap(),
        )
        .unwrap();
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::MoveEntityRotation,
        );
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RotateHead,
        );
        assert!(steve_writer.try_recv_output().is_err());

        game.move_player(
            steve,
            Transform::new([100.0, 65.0, 0.5], 90.0, 0.0, true).unwrap(),
        )
        .unwrap();
        let payload = recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::TeleportEntity,
        );
        assert_eq!(read_varint(&payload).0, steve_entity_id.get() as i32);
        assert!(steve_writer.try_recv_output().is_err());
        service.shutdown().unwrap();
    }

    #[test]
    fn synchronizes_initial_equipment_and_selected_hotbar_changes() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let steve = PlayerUuid::new(301);
        let alex = PlayerUuid::new(302);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(256).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(301),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let (alex_reader, alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(302),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(256).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );
        let steve_entity_id = game
            .with_state(|state| state.player(steve).and_then(|player| player.entity_id))
            .unwrap()
            .unwrap();
        let stone = ItemStack::new("minecraft:stone", 1).unwrap();
        game.with_state_mut(|state| {
            state
                .player_mut(steve)
                .unwrap()
                .inventory
                .set_slot(HOTBAR_START, Some(stone.clone()))
                .unwrap();
            Ok(())
        })
        .unwrap();

        service.control().register(alex, alex_reader).unwrap();
        for kind in [
            PacketKind::PlayerInfoUpdate,
            PacketKind::AddEntity,
            PacketKind::SetEntityData,
        ] {
            recv_protocol(&alex_writer, &mut workers, &mut inputs, kind);
        }
        let equipment = recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEquipment,
        );
        let (entity_id, entity_bytes) = read_varint(&equipment);
        assert_eq!(entity_id, steve_entity_id.get() as i32);
        assert_eq!(equipment[entity_bytes] & 0x7f, 0);
        let (count, count_bytes) = read_varint(&equipment[entity_bytes + 1..]);
        assert_eq!(count, 1);
        let item_offset = entity_bytes + 1 + count_bytes;
        assert_eq!(read_varint(&equipment[item_offset..]).0, 1);
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RotateHead,
        );

        game.connect_player(alex, "Alex", spawn()).unwrap();
        for kind in [
            PacketKind::PlayerInfoUpdate,
            PacketKind::AddEntity,
            PacketKind::SetEntityData,
            PacketKind::RotateHead,
        ] {
            recv_protocol(&steve_writer, &mut workers, &mut inputs, kind);
        }
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { .. }
        ));
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );

        game.with_state_mut(|state| {
            state
                .player_mut(steve)
                .unwrap()
                .inventory
                .set_slot(HOTBAR_START + 1, Some(stone.clone()))
                .unwrap();
            Ok(())
        })
        .unwrap();
        game.publish(&[GameEvent::InventorySlotChanged {
            uuid: steve,
            slot: HOTBAR_START + 1,
            stack: Some(stone),
        }])
        .unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SetPlayerInventory { slot, .. } if slot == HOTBAR_START + 1
        ));
        assert!(alex_writer.try_recv_output().is_err());

        game.select_hotbar(steve, 1).unwrap();
        let equipment = recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEquipment,
        );
        let (entity_id, entity_bytes) = read_varint(&equipment);
        assert_eq!(entity_id, steve_entity_id.get() as i32);
        assert_eq!(equipment[entity_bytes] & 0x7f, 0);
        assert_eq!(read_varint(&equipment[entity_bytes + 1..]).0, 1);
        assert!(steve_writer.try_recv_output().is_err());
        service.shutdown().unwrap();
    }

    #[test]
    fn synchronizes_initial_and_changed_health_only_to_the_subject() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, GameReplicationConfig::default()).unwrap();
        let steve = PlayerUuid::new(401);
        let alex = PlayerUuid::new(402);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(128).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(401),
            NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
        let (alex_reader, alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(402),
            NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(128).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        assert_eq!(
            recv_raw_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload: vec![0x41, 0xa0, 0, 0, 0x14, 0x40, 0xa0, 0, 0],
            }
        );

        service.control().register(alex, alex_reader).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        assert!(matches!(
            recv_raw_output(&alex_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex joined the game"
        ));

        game.damage_player(steve, 4.0).unwrap();
        match recv_raw_output(&steve_writer, &mut workers, &mut inputs) {
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload,
            } => {
                assert_eq!(f32::from_be_bytes(payload[0..4].try_into().unwrap()), 16.0);
                assert_eq!(payload[4], 20);
                assert_eq!(f32::from_be_bytes(payload[5..9].try_into().unwrap()), 5.0);
            }
            output => panic!("expected health packet, got {output:?}"),
        }
        assert!(alex_writer.try_recv_output().is_err());

        game.damage_player(steve, 100.0).unwrap();
        assert!(matches!(
            recv_raw_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload,
            } if f32::from_be_bytes(payload[0..4].try_into().unwrap()) == 0.0
        ));
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, overlay: false } if message == "You died"
        ));
        service.shutdown().unwrap();
    }
}
