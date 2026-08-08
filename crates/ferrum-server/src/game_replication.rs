use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
    sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use ferrum_game::{
    AttributeSet, Entity, EntityId, EntityPayload, EntityUuid, EquipmentSlot, Experience,
    GameEvent, GameMode, GameState, ItemStack, PLAYER_INVENTORY_SLOTS, PlayerState, PlayerUuid,
    StatusEffectSet, Transform, Velocity, Vitals,
};
use ferrum_play::{
    CommonPlayerSpawnInfo, DataComponentProtocolRegistry, EncodedEntityMovement,
    EntityMovementKind, EntityProtocolRegistry, EquipmentEntry, ExperienceOrbMetadataProtocol,
    ItemEntityMetadataProtocol, ItemProtocolRegistry, PlayerInfoEntry, ProtocolIdRegistry, Respawn,
    RespawnDataToKeep, encode_add_entity, encode_add_entity_with_uuid, encode_damage_event,
    encode_empty_entity_data, encode_entity_movement, encode_experience_orb_data,
    encode_hurt_animation, encode_item_entity_data, encode_player_combat_kill,
    encode_player_info_remove, encode_player_info_update, encode_remove_entities,
    encode_remove_mob_effect, encode_respawn, encode_rotate_head, encode_set_entity_motion,
    encode_set_equipment, encode_set_experience, encode_set_health, encode_take_item_entity,
    encode_teleport_entity, encode_update_attribute, encode_update_attributes,
    encode_update_mob_effect,
};
use ferrum_protocol::PacketKind;
use ferrum_rompack::RomPackWorld;

use crate::{
    authoritative_runtime::PlayOutput,
    game_runtime::{GameEventSubscription, SharedGameRuntime},
    play_connection::{PlayOutputSubmitError, PlayReaderEndpoint},
};

const DEFAULT_PENDING_OUTPUT_LIMIT: usize = 256;
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(5);
const DEFAULT_ENTITY_TRACKING_RANGE_BLOCKS: u32 = 128;
const MAX_ENTITY_TRACKING_RANGE_BLOCKS: u32 = 1_024;
const MAX_COMMANDS_PER_POLL: usize = 256;
const MAX_EVENTS_PER_POLL: usize = 1_024;

#[derive(Debug, Clone)]
pub struct GameReplicationConfig {
    pub event_capacity: NonZeroUsize,
    pub command_capacity: NonZeroUsize,
    pub pending_output_limit: NonZeroUsize,
    pub poll_interval: Duration,
    pub entity_tracking_range_blocks: u32,
    pub entity_protocol_ids: EntityProtocolRegistry,
    pub item_protocol_ids: ItemProtocolRegistry,
    pub data_component_protocol_ids: DataComponentProtocolRegistry,
    pub attribute_protocol_ids: ProtocolIdRegistry,
    pub mob_effect_protocol_ids: ProtocolIdRegistry,
    pub damage_type_protocol_ids: ProtocolIdRegistry,
    pub item_entity_metadata: Option<ItemEntityMetadataProtocol>,
    pub experience_orb_metadata: Option<ExperienceOrbMetadataProtocol>,
    pub world: Option<RomPackWorld>,
}

impl Default for GameReplicationConfig {
    fn default() -> Self {
        Self {
            event_capacity: NonZeroUsize::new(4_096).expect("4096 is non-zero"),
            command_capacity: NonZeroUsize::new(1_024).expect("1024 is non-zero"),
            pending_output_limit: NonZeroUsize::new(DEFAULT_PENDING_OUTPUT_LIMIT)
                .expect("pending output limit is non-zero"),
            poll_interval: DEFAULT_POLL_INTERVAL,
            entity_tracking_range_blocks: DEFAULT_ENTITY_TRACKING_RANGE_BLOCKS,
            entity_protocol_ids: EntityProtocolRegistry::default(),
            item_protocol_ids: ItemProtocolRegistry::default(),
            data_component_protocol_ids: DataComponentProtocolRegistry::default(),
            attribute_protocol_ids: ProtocolIdRegistry::default(),
            mob_effect_protocol_ids: ProtocolIdRegistry::default(),
            damage_type_protocol_ids: ProtocolIdRegistry::default(),
            item_entity_metadata: None,
            experience_orb_metadata: None,
            world: None,
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
    attributes: AttributeSet,
    status_effects: StatusEffectSet,
}

#[derive(Debug, Clone)]
struct ItemEntitySnapshot {
    entity_id: EntityId,
    uuid: EntityUuid,
    transform: Transform,
    velocity: Velocity,
    stack: ItemStack,
}

#[derive(Debug, Clone)]
struct NonPlayerEntitySnapshot {
    entity_id: EntityId,
    uuid: EntityUuid,
    entity_type: String,
    transform: Transform,
    velocity: Velocity,
    metadata: NonPlayerEntityMetadata,
    attributes: Option<AttributeSet>,
    status_effects: Option<StatusEffectSet>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NonPlayerEntityMetadata {
    Empty,
    ExperienceOrb { value: u32 },
}

#[derive(Debug)]
struct ReplicationConnection {
    endpoint: PlayReaderEndpoint,
    pending: VecDeque<PlayOutput>,
    pending_limit: usize,
    next_teleport_id: i32,
    entities: BTreeMap<PlayerUuid, PlayerEntitySnapshot>,
    item_entities: BTreeMap<EntityId, ItemEntitySnapshot>,
    non_player_entities: BTreeMap<EntityId, NonPlayerEntitySnapshot>,
    viewer_transform: Option<Transform>,
    active: bool,
    healthy: bool,
    self_initialized: bool,
}

impl ReplicationConnection {
    fn new(endpoint: PlayReaderEndpoint, pending_limit: usize) -> Self {
        Self {
            endpoint,
            pending: VecDeque::new(),
            pending_limit,
            next_teleport_id: 2,
            entities: BTreeMap::new(),
            item_entities: BTreeMap::new(),
            non_player_entities: BTreeMap::new(),
            viewer_transform: None,
            active: false,
            healthy: true,
            self_initialized: false,
        }
    }

    fn activate(&mut self) -> Result<()> {
        if self.active {
            bail!("replication connection is already active");
        }
        if !self.healthy {
            bail!("replication connection is not healthy");
        }
        self.active = true;
        Ok(())
    }

    fn queue(&mut self, output: PlayOutput, exit: &mut GameReplicationExit) -> bool {
        if !self.active || !self.healthy {
            return false;
        }
        if self.pending.len() >= self.pending_limit {
            self.pending.clear();
            self.entities.clear();
            self.item_entities.clear();
            self.non_player_entities.clear();
            self.healthy = false;
            exit.dropped_outputs = exit.dropped_outputs.saturating_add(1);
            let _ = self.endpoint.try_disconnect();
            return false;
        }
        self.pending.push_back(output);
        exit.produced_outputs = exit.produced_outputs.saturating_add(1);
        true
    }

    fn queue_teleport(
        &mut self,
        transform: ferrum_game::Transform,
        exit: &mut GameReplicationExit,
    ) -> bool {
        let teleport_id = self.next_teleport_id;
        self.next_teleport_id = self.next_teleport_id.saturating_add(1);
        self.queue(
            PlayOutput::PlayerTeleport {
                teleport_id,
                transform,
            },
            exit,
        )
    }

    fn flush(&mut self, exit: &mut GameReplicationExit) -> bool {
        if !self.healthy {
            return false;
        }
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
    Activate {
        uuid: PlayerUuid,
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

    pub fn activate(&self, uuid: PlayerUuid) -> Result<()> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(ReplicationCommand::Activate { uuid, reply })
            .context("game replication service is disconnected")?;
        response
            .recv()
            .context("game replication service dropped activation response")?
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
    if !(1..=MAX_ENTITY_TRACKING_RANGE_BLOCKS).contains(&config.entity_tracking_range_blocks) {
        bail!(
            "game replication entity tracking range must be between 1 and {MAX_ENTITY_TRACKING_RANGE_BLOCKS} blocks"
        );
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
                let result = match connections.entry(uuid) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(ReplicationConnection::new(
                            endpoint,
                            config.pending_output_limit.get(),
                        ));
                        Ok(())
                    }
                    std::collections::btree_map::Entry::Occupied(_) => Err(format!(
                        "player {uuid:?} is already registered for replication"
                    )),
                };
                let _ = reply.send(result);
            }
            ReplicationCommand::Activate { uuid, reply } => {
                let result = (|| -> Result<()> {
                    let (self_state, snapshots, item_snapshots, non_player_snapshots) = runtime
                        .with_state(|state| -> Result<_> {
                            let self_state = match state.player(uuid) {
                                Some(player) if player.connected => Some((
                                    player.vitals,
                                    player.experience,
                                    player_snapshot_from_state(state, uuid)?.with_context(|| {
                                        format!(
                                            "active player {uuid:?} has no authoritative entity snapshot"
                                        )
                                    })?,
                                )),
                                _ => None,
                            };
                            let mut snapshots = Vec::new();
                            for player in state
                                .players()
                                .values()
                                .filter(|player| player.connected && player.uuid != uuid)
                            {
                                snapshots.push(
                                    player_snapshot_from_state(state, player.uuid)?.with_context(
                                        || {
                                            format!(
                                                "online player {:?} has no entity snapshot",
                                                player.uuid
                                            )
                                        },
                                    )?,
                                );
                            }
                            let item_snapshots = state
                                .entities()
                                .iter()
                                .filter_map(|(_, entity)| item_snapshot_from_entity(entity))
                                .collect::<Vec<_>>();
                            let non_player_snapshots = state
                                .entities()
                                .iter()
                                .filter_map(|(_, entity)| {
                                    non_player_snapshot_from_entity(entity)
                                })
                                .collect::<Vec<_>>();
                            Ok((
                                self_state,
                                snapshots,
                                item_snapshots,
                                non_player_snapshots,
                            ))
                        })
                        .context("cannot read activation snapshot")??;
                    let connection = connections.get_mut(&uuid).with_context(|| {
                        format!("player {uuid:?} is not registered for replication")
                    })?;
                    connection.activate()?;
                    if let Some((vitals, experience, snapshot)) = self_state {
                        connection.viewer_transform = Some(snapshot.transform);
                        queue_set_health(connection, vitals, exit)?;
                        queue_set_experience(connection, experience, exit)?;
                        if entity_replication_enabled(&config.entity_protocol_ids) {
                            queue_player_info_update(connection, &snapshot, exit)?;
                        }
                        queue_player_state_sync(connection, &snapshot, config, exit)?;
                        connection.self_initialized = true;
                    }
                    if entity_replication_enabled(&config.entity_protocol_ids) {
                        for snapshot in snapshots {
                            queue_player_spawn(connection, snapshot, config, exit)?;
                        }
                    }
                    if item_entity_replication_enabled(config) {
                        for snapshot in item_snapshots {
                            queue_item_spawn(connection, snapshot, config, exit)?;
                        }
                    }
                    for snapshot in non_player_snapshots {
                        queue_non_player_spawn(connection, snapshot, config, exit)?;
                    }
                    if !connection.healthy {
                        bail!("initial replication snapshot exceeded the bounded output queue");
                    }
                    Ok(())
                })()
                .map_err(|error| error.to_string());
                if result.is_err() {
                    connections.remove(&uuid);
                }
                let _ = reply.send(result);
            }
            ReplicationCommand::SyncInventory { uuid, reply } => {
                let result = if let Some(connection) = connections.get_mut(&uuid) {
                    if !connection.active {
                        Err(format!("player {uuid:?} replication is not active"))
                    } else {
                        runtime
                            .with_state(|state| {
                                state
                                    .player(uuid)
                                    .map(|player| player.inventory.slots().to_vec())
                            })
                            .map_err(|error| error.to_string())
                            .and_then(|slots| {
                                slots.ok_or_else(|| {
                                    format!(
                                        "player {uuid:?} is missing from authoritative state"
                                    )
                                })
                            })
                            .and_then(|slots| {
                                if slots.len() != PLAYER_INVENTORY_SLOTS {
                                    return Err(format!(
                                        "player inventory has {} slots; expected {PLAYER_INVENTORY_SLOTS}",
                                        slots.len()
                                    ));
                                }
                                if !connection.queue(
                                    PlayOutput::SetContainerContent {
                                        container_id: ferrum_game::PLAYER_CONTAINER_ID,
                                        state_id: 0,
                                        slots,
                                        carried: None,
                                    },
                                    exit,
                                ) {
                                    return Err(
                                        "cannot queue inventory snapshot on an unhealthy replication connection"
                                            .to_owned(),
                                    );
                                }
                                exit.inventory_snapshots =
                                    exit.inventory_snapshots.saturating_add(1);
                                Ok(())
                            })
                    }
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
            let snapshot = player_snapshot(runtime, uuid)?.with_context(|| {
                format!("connected player {uuid:?} is missing from authoritative state")
            })?;
            let player_state = runtime
                .with_state(|state| {
                    state
                        .player(uuid)
                        .map(|player| (player.vitals, player.experience))
                })
                .context("cannot read connected player vitals and experience")?;
            if let Some(connection) = connections.get_mut(&uuid)
                && connection.active
                && !connection.self_initialized
            {
                if let Some((vitals, experience)) = player_state {
                    queue_set_health(connection, vitals, exit)?;
                    queue_set_experience(connection, experience, exit)?;
                }
                connection.viewer_transform = Some(snapshot.transform);
                if entity_replication_enabled(&config.entity_protocol_ids) {
                    queue_player_info_update(connection, &snapshot, exit)?;
                }
                queue_player_state_sync(connection, &snapshot, config, exit)?;
                connection.self_initialized = true;
                reconcile_connection_visibility(connection, uuid, runtime, config, exit)?;
            }
            if entity_replication_enabled(&config.entity_protocol_ids) {
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
            entity_id: _,
        } => {
            if entity_replication_enabled(&config.entity_protocol_ids) {
                for (target, connection) in connections.iter_mut() {
                    if *target != uuid {
                        queue_player_remove(connection, uuid, exit)?;
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
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.viewer_transform = Some(transform);
            }
            if entity_replication_enabled(&config.entity_protocol_ids) {
                let snapshot = player_snapshot(runtime, uuid)?;
                for (target, connection) in connections.iter_mut() {
                    if *target == uuid {
                        continue;
                    }
                    if !connection_tracks_position(connection, transform, config) {
                        queue_player_entity_remove(connection, uuid, exit)?;
                        continue;
                    }
                    match connection.entities.get(&uuid).cloned() {
                        Some(tracked) if tracked.entity_id == entity_id => {
                            queue_player_movement(connection, &tracked, transform, exit)?;
                        }
                        Some(_) => {
                            queue_player_entity_remove(connection, uuid, exit)?;
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
            if let Some(connection) = connections.get_mut(&uuid) {
                reconcile_connection_visibility(connection, uuid, runtime, config, exit)?;
            }
        }
        GameEvent::PlayerTeleported {
            uuid,
            entity_id,
            transform,
        } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.viewer_transform = Some(transform);
                connection.queue_teleport(transform, exit);
            }
            if entity_replication_enabled(&config.entity_protocol_ids) {
                let snapshot = player_snapshot(runtime, uuid)?;
                for (target, connection) in connections.iter_mut() {
                    if *target == uuid {
                        continue;
                    }
                    if !connection_tracks_position(connection, transform, config) {
                        queue_player_entity_remove(connection, uuid, exit)?;
                        continue;
                    }
                    match connection.entities.get(&uuid).cloned() {
                        Some(tracked) if tracked.entity_id == entity_id => {
                            queue_player_absolute_teleport(connection, &tracked, transform, exit)?;
                        }
                        Some(_) => {
                            queue_player_entity_remove(connection, uuid, exit)?;
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
            if let Some(connection) = connections.get_mut(&uuid) {
                reconcile_connection_visibility(connection, uuid, runtime, config, exit)?;
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
        GameEvent::PlayerDamaged {
            uuid,
            entity_id,
            source,
            ..
        } => {
            let damage_payload =
                encode_damage_event(entity_id, source, &config.damage_type_protocol_ids)
                    .context("cannot encode player damage source")?;
            let payload = encode_hurt_animation(entity_id, 0.0)
                .context("cannot encode player hurt animation")?;
            for (target, connection) in connections.iter_mut() {
                if *target == uuid || connection.entities.contains_key(&uuid) {
                    if let Some(payload) = damage_payload.clone() {
                        connection.queue(
                            PlayOutput::ProtocolPacket {
                                kind: PacketKind::DamageEvent,
                                payload,
                            },
                            exit,
                        );
                    }
                    connection.queue(
                        PlayOutput::ProtocolPacket {
                            kind: PacketKind::HurtAnimation,
                            payload: payload.clone(),
                        },
                        exit,
                    );
                }
            }
        }
        GameEvent::PlayerVelocityChanged {
            uuid,
            entity_id,
            velocity,
        } => {
            let payload = encode_set_entity_motion(entity_id, velocity)
                .context("cannot encode player entity motion")?;
            for (target, connection) in connections.iter_mut() {
                if *target == uuid || connection.entities.contains_key(&uuid) {
                    connection.queue(
                        PlayOutput::ProtocolPacket {
                            kind: PacketKind::SetEntityMotion,
                            payload: payload.clone(),
                        },
                        exit,
                    );
                    if let Some(tracked) = connection.entities.get_mut(&uuid) {
                        tracked.velocity = velocity;
                    }
                }
            }
        }
        GameEvent::PlayerAttributeChanged {
            uuid, attribute, ..
        } => {
            let Some(snapshot) = player_snapshot(runtime, uuid)? else {
                return Ok(());
            };
            let instance = snapshot.attributes.get(&attribute).with_context(|| {
                format!("attribute update references missing attribute {attribute}")
            })?;
            let payload = encode_update_attribute(
                snapshot.entity_id,
                &attribute,
                instance,
                &config.attribute_protocol_ids,
            )
            .context("cannot encode player attribute update")?;
            if let Some(payload) = payload {
                for (target, connection) in connections.iter_mut() {
                    if *target == uuid || connection.entities.contains_key(&uuid) {
                        connection.queue(
                            PlayOutput::ProtocolPacket {
                                kind: PacketKind::UpdateAttributes,
                                payload: payload.clone(),
                            },
                            exit,
                        );
                        if let Some(tracked) = connection.entities.get_mut(&uuid)
                            && let Some(tracked_instance) = tracked.attributes.get_mut(&attribute)
                        {
                            *tracked_instance = instance.clone();
                        }
                    }
                }
            }
        }
        GameEvent::PlayerStatusEffectChanged {
            uuid,
            effect,
            active,
        } => {
            let Some(snapshot) = player_snapshot(runtime, uuid)? else {
                return Ok(());
            };
            let (kind, payload) = if active {
                let instance = snapshot.status_effects.get(&effect).with_context(|| {
                    format!("active status-effect update is missing effect {effect}")
                })?;
                (
                    PacketKind::UpdateMobEffect,
                    encode_update_mob_effect(
                        snapshot.entity_id,
                        instance,
                        &config.mob_effect_protocol_ids,
                    )
                    .context("cannot encode player status-effect update")?,
                )
            } else {
                (
                    PacketKind::RemoveMobEffect,
                    encode_remove_mob_effect(
                        snapshot.entity_id,
                        &effect,
                        &config.mob_effect_protocol_ids,
                    )
                    .context("cannot encode player status-effect removal")?,
                )
            };
            if let Some(payload) = payload {
                for (target, connection) in connections.iter_mut() {
                    if *target == uuid || connection.entities.contains_key(&uuid) {
                        connection.queue(
                            PlayOutput::ProtocolPacket {
                                kind,
                                payload: payload.clone(),
                            },
                            exit,
                        );
                    }
                }
            }
        }
        GameEvent::PlayerVitalsChanged { uuid, vitals } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                queue_set_health(connection, vitals, exit)?;
            }
        }
        GameEvent::PlayerKilled {
            uuid,
            entity_id,
            name,
        } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::PlayerCombatKill,
                        payload: encode_player_combat_kill(entity_id, &format!("{name} died"))
                            .context("cannot encode player combat death")?,
                    },
                    exit,
                );
            }
            if entity_replication_enabled(&config.entity_protocol_ids) {
                for (target, connection) in connections.iter_mut() {
                    if *target != uuid {
                        queue_player_entity_remove(connection, uuid, exit)?;
                    }
                }
            }
        }
        GameEvent::PlayerRespawned {
            uuid,
            entity_id: _,
            transform,
            game_mode,
            previous_game_mode,
        } => {
            let world = config
                .world
                .as_ref()
                .context("player respawn requires a replication world profile")?;
            let respawn = Respawn {
                spawn_info: CommonPlayerSpawnInfo {
                    dimension_type_id: world.dimension_type_id,
                    dimension: world.dimension.clone(),
                    seed: 0,
                    game_mode: protocol_game_mode(game_mode),
                    previous_game_mode: protocol_previous_game_mode(previous_game_mode),
                    is_debug: false,
                    is_flat: true,
                    last_death_location: None,
                    portal_cooldown: 0,
                    sea_level: world.sea_level,
                },
                data_to_keep: RespawnDataToKeep::Attributes,
            };
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.viewer_transform = Some(transform);
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::Respawn,
                        payload: encode_respawn(&respawn)
                            .context("cannot encode player respawn")?,
                    },
                    exit,
                );
                connection.queue_teleport(transform, exit);
            }
            if entity_replication_enabled(&config.entity_protocol_ids)
                && let Some(snapshot) = player_snapshot(runtime, uuid)?
            {
                for (target, connection) in connections.iter_mut() {
                    if *target != uuid {
                        queue_player_spawn(connection, snapshot.clone(), config, exit)?;
                    }
                }
            }
            if let Some(connection) = connections.get_mut(&uuid) {
                reconcile_connection_visibility(connection, uuid, runtime, config, exit)?;
            }
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
        GameEvent::LivingEntityDamaged {
            entity_id, source, ..
        } => {
            let damage_payload =
                encode_damage_event(entity_id, source, &config.damage_type_protocol_ids)
                    .context("cannot encode non-player damage source")?;
            for connection in connections.values_mut() {
                let Some(yaw) = connection
                    .non_player_entities
                    .get(&entity_id)
                    .map(|snapshot| snapshot.transform.yaw)
                else {
                    continue;
                };
                if let Some(payload) = damage_payload.clone() {
                    connection.queue(
                        PlayOutput::ProtocolPacket {
                            kind: PacketKind::DamageEvent,
                            payload,
                        },
                        exit,
                    );
                }
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::HurtAnimation,
                        payload: encode_hurt_animation(entity_id, yaw)
                            .context("cannot encode non-player hurt animation")?,
                    },
                    exit,
                );
            }
        }
        GameEvent::LivingEntityKilled { .. } => {}
        GameEvent::EntityMoved {
            entity_id,
            transform,
            velocity,
        } => {
            let entity = runtime
                .with_state(|state| state.entities().get(entity_id).cloned())
                .context("cannot read moving entity snapshot")?;
            if let Some(snapshot) = entity.as_ref().and_then(item_snapshot_from_entity) {
                debug_assert_eq!(snapshot.transform, transform);
                debug_assert_eq!(snapshot.velocity, velocity);
                for connection in connections.values_mut() {
                    queue_item_spawn(connection, snapshot.clone(), config, exit)?;
                }
            } else if let Some(snapshot) = entity.as_ref().and_then(non_player_snapshot_from_entity)
            {
                debug_assert_eq!(snapshot.transform, transform);
                debug_assert_eq!(snapshot.velocity, velocity);
                for connection in connections.values_mut() {
                    queue_non_player_spawn(connection, snapshot.clone(), config, exit)?;
                }
            }
        }
        GameEvent::EntityRemoved { entity_id } => {
            queue_item_remove(connections, entity_id, exit)?;
            queue_non_player_remove(connections, entity_id, exit)?;
        }
        GameEvent::ItemEntityChanged { entity_id, stack } => {
            queue_item_stack_update(connections, entity_id, &stack, config, exit)?;
        }
        GameEvent::ItemPickedUp {
            uuid,
            entity_id,
            inserted,
            item,
        } => {
            if let Some(collector) = player_snapshot(runtime, uuid)? {
                queue_item_pickup(connections, entity_id, collector.entity_id, inserted, exit)?;
            }
            target_chat(
                connections,
                uuid,
                format!("Picked up {inserted} {item}"),
                true,
                exit,
            );
        }
        GameEvent::ExperienceOrbPickedUp {
            uuid,
            entity_id,
            value: _,
        } => {
            if let Some(collector) = player_snapshot(runtime, uuid)? {
                queue_experience_orb_pickup(connections, entity_id, collector.entity_id, exit)?;
            }
        }
        GameEvent::PlayerExperienceChanged { uuid, experience } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                queue_set_experience(connection, experience, exit)?;
            }
        }
        GameEvent::EntitySpawned { entity } => {
            if item_entity_replication_enabled(config)
                && let Some(snapshot) = item_snapshot_from_entity(&entity)
            {
                for connection in connections.values_mut() {
                    queue_item_spawn(connection, snapshot.clone(), config, exit)?;
                }
            } else if let Some(snapshot) = non_player_snapshot_from_entity(&entity) {
                for connection in connections.values_mut() {
                    queue_non_player_spawn(connection, snapshot.clone(), config, exit)?;
                }
            }
        }
        GameEvent::TimeChanged { .. } | GameEvent::SaveRequested | GameEvent::ShutdownRequested => {
        }
    }
    Ok(())
}

fn item_entity_replication_enabled(config: &GameReplicationConfig) -> bool {
    config.item_entity_metadata.is_some()
        && config
            .entity_protocol_ids
            .protocol_id("minecraft:item")
            .is_some()
}

fn item_snapshot_from_entity(entity: &Entity) -> Option<ItemEntitySnapshot> {
    let EntityPayload::Item(item) = &entity.payload else {
        return None;
    };
    Some(ItemEntitySnapshot {
        entity_id: entity.id,
        uuid: entity.uuid,
        transform: entity.transform,
        velocity: entity.velocity,
        stack: item.stack.clone(),
    })
}

fn non_player_snapshot_from_entity(entity: &Entity) -> Option<NonPlayerEntitySnapshot> {
    if entity.is_player()
        || entity.entity_type.as_str() == "minecraft:item"
        || matches!(&entity.payload, EntityPayload::Item(_))
    {
        return None;
    }
    let (metadata, attributes, status_effects) = match &entity.payload {
        EntityPayload::Living(living) => (
            NonPlayerEntityMetadata::Empty,
            Some(living.attributes.clone()),
            Some(living.status_effects.clone()),
        ),
        EntityPayload::ExperienceOrb { value } => (
            NonPlayerEntityMetadata::ExperienceOrb { value: *value },
            None,
            None,
        ),
        _ => (NonPlayerEntityMetadata::Empty, None, None),
    };
    Some(NonPlayerEntitySnapshot {
        entity_id: entity.id,
        uuid: entity.uuid,
        entity_type: entity.entity_type.as_str().to_owned(),
        transform: entity.transform,
        velocity: entity.velocity,
        metadata,
        attributes,
        status_effects,
    })
}

fn queue_item_spawn(
    connection: &mut ReplicationConnection,
    snapshot: ItemEntitySnapshot,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !item_entity_replication_enabled(config) || !connection.active || !connection.healthy {
        return Ok(());
    }
    if !connection_tracks_position(connection, snapshot.transform, config) {
        queue_item_remove_for_connection(connection, snapshot.entity_id, exit)?;
        return Ok(());
    }
    if let Some(tracked) = connection.item_entities.get(&snapshot.entity_id).cloned() {
        if tracked.uuid == snapshot.uuid {
            let stack_changed = tracked.stack != snapshot.stack;
            if stack_changed
                && !queue_item_stack_update_for_connection(
                    connection,
                    snapshot.entity_id,
                    &snapshot.stack,
                    config,
                    exit,
                )?
            {
                queue_item_remove_for_connection(connection, snapshot.entity_id, exit)?;
                return Ok(());
            }
            if tracked.transform != snapshot.transform
                && let Some(movement) = encode_entity_movement(
                    snapshot.entity_id,
                    tracked.transform,
                    snapshot.transform,
                )
                .context("cannot encode item entity movement")?
            {
                queue_encoded_movement(connection, movement, exit);
            }
            if tracked.velocity != snapshot.velocity {
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::SetEntityMotion,
                        payload: encode_set_entity_motion(snapshot.entity_id, snapshot.velocity)
                            .context("cannot encode item entity velocity")?,
                    },
                    exit,
                );
            }
            if connection.healthy {
                connection
                    .item_entities
                    .insert(snapshot.entity_id, snapshot);
            }
            return Ok(());
        }
        queue_item_remove_for_connection(connection, snapshot.entity_id, exit)?;
    }
    let Some(add_payload) = encode_add_entity_with_uuid(
        snapshot.entity_id,
        snapshot.uuid,
        "minecraft:item",
        snapshot.transform,
        snapshot.velocity,
        &config.entity_protocol_ids,
    )
    .context("cannot encode item add-entity packet")?
    else {
        return Ok(());
    };
    let metadata = config
        .item_entity_metadata
        .context("item entity metadata protocol is unavailable")?;
    let Some(data_payload) = encode_item_entity_data(
        snapshot.entity_id,
        &snapshot.stack,
        &config.item_protocol_ids,
        &config.data_component_protocol_ids,
        metadata,
    )
    .context("cannot encode item entity stack metadata")?
    else {
        return Ok(());
    };
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::AddEntity,
            payload: add_payload,
        },
        exit,
    );
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetEntityData,
            payload: data_payload,
        },
        exit,
    );
    if connection.healthy {
        connection
            .item_entities
            .insert(snapshot.entity_id, snapshot);
    }
    Ok(())
}

fn queue_non_player_spawn(
    connection: &mut ReplicationConnection,
    snapshot: NonPlayerEntitySnapshot,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !connection.active || !connection.healthy {
        return Ok(());
    }
    if config
        .entity_protocol_ids
        .protocol_id(&snapshot.entity_type)
        .is_none()
        || !connection_tracks_position(connection, snapshot.transform, config)
    {
        queue_non_player_remove_for_connection(connection, snapshot.entity_id, exit)?;
        return Ok(());
    }
    let Some(data_payload) = encode_non_player_entity_data(&snapshot, config)? else {
        queue_non_player_remove_for_connection(connection, snapshot.entity_id, exit)?;
        return Ok(());
    };
    if let Some(tracked) = connection
        .non_player_entities
        .get(&snapshot.entity_id)
        .cloned()
    {
        if tracked.uuid == snapshot.uuid && tracked.entity_type == snapshot.entity_type {
            if tracked.metadata != snapshot.metadata {
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::SetEntityData,
                        payload: data_payload.clone(),
                    },
                    exit,
                );
            }
            if tracked.transform != snapshot.transform
                && let Some(mut movement) = encode_entity_movement(
                    snapshot.entity_id,
                    tracked.transform,
                    snapshot.transform,
                )
                .context("cannot encode non-player entity movement")?
            {
                if movement.kind == EntityMovementKind::Teleport {
                    movement.payload = encode_teleport_entity(
                        snapshot.entity_id,
                        snapshot.transform,
                        snapshot.velocity,
                    )
                    .context("cannot encode non-player entity teleport")?;
                }
                queue_encoded_movement(connection, movement, exit);
            }
            if tracked.transform.yaw.to_bits() != snapshot.transform.yaw.to_bits() {
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::RotateHead,
                        payload: encode_rotate_head(snapshot.entity_id, snapshot.transform.yaw)
                            .context("cannot encode non-player entity head rotation")?,
                    },
                    exit,
                );
            }
            if tracked.velocity != snapshot.velocity {
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::SetEntityMotion,
                        payload: encode_set_entity_motion(snapshot.entity_id, snapshot.velocity)
                            .context("cannot encode non-player entity velocity")?,
                    },
                    exit,
                );
            }
            if connection.healthy {
                connection
                    .non_player_entities
                    .insert(snapshot.entity_id, snapshot);
            }
            return Ok(());
        }
        queue_non_player_remove_for_connection(connection, snapshot.entity_id, exit)?;
    }
    let Some(payload) = encode_add_entity_with_uuid(
        snapshot.entity_id,
        snapshot.uuid,
        &snapshot.entity_type,
        snapshot.transform,
        snapshot.velocity,
        &config.entity_protocol_ids,
    )
    .context("cannot encode non-player add-entity packet")?
    else {
        return Ok(());
    };
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
            payload: data_payload,
        },
        exit,
    );
    queue_non_player_state_sync(connection, &snapshot, config, exit)?;
    if connection.healthy {
        connection
            .non_player_entities
            .insert(snapshot.entity_id, snapshot);
    }
    Ok(())
}

fn encode_non_player_entity_data(
    snapshot: &NonPlayerEntitySnapshot,
    config: &GameReplicationConfig,
) -> Result<Option<Vec<u8>>> {
    match snapshot.metadata {
        NonPlayerEntityMetadata::Empty => Ok(Some(
            encode_empty_entity_data(snapshot.entity_id)
                .context("cannot encode empty non-player entity data")?,
        )),
        NonPlayerEntityMetadata::ExperienceOrb { value } => {
            let Some(metadata) = config.experience_orb_metadata else {
                return Ok(None);
            };
            Ok(Some(
                encode_experience_orb_data(snapshot.entity_id, value, metadata)
                    .context("cannot encode experience orb entity data")?,
            ))
        }
    }
}

fn queue_non_player_state_sync(
    connection: &mut ReplicationConnection,
    snapshot: &NonPlayerEntitySnapshot,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if let Some(attributes) = &snapshot.attributes
        && let Some(payload) = encode_update_attributes(
            snapshot.entity_id,
            attributes,
            &config.attribute_protocol_ids,
        )
        .context("cannot encode non-player attribute snapshot")?
    {
        connection.queue(
            PlayOutput::ProtocolPacket {
                kind: PacketKind::UpdateAttributes,
                payload,
            },
            exit,
        );
    }
    if let Some(status_effects) = &snapshot.status_effects {
        for effect in status_effects.iter().map(|(_, effect)| effect) {
            if let Some(payload) = encode_update_mob_effect(
                snapshot.entity_id,
                effect,
                &config.mob_effect_protocol_ids,
            )
            .context("cannot encode non-player status-effect snapshot")?
            {
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::UpdateMobEffect,
                        payload,
                    },
                    exit,
                );
            }
        }
    }
    Ok(())
}

fn queue_item_stack_update(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    entity_id: EntityId,
    stack: &ItemStack,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    for connection in connections.values_mut() {
        if !connection.item_entities.contains_key(&entity_id) {
            continue;
        }
        if queue_item_stack_update_for_connection(connection, entity_id, stack, config, exit)? {
            if let Some(tracked) = connection.item_entities.get_mut(&entity_id) {
                tracked.stack = stack.clone();
            }
        } else {
            queue_item_remove_for_connection(connection, entity_id, exit)?;
        }
    }
    Ok(())
}

fn queue_item_stack_update_for_connection(
    connection: &mut ReplicationConnection,
    entity_id: EntityId,
    stack: &ItemStack,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<bool> {
    let metadata = config
        .item_entity_metadata
        .context("item entity metadata protocol is unavailable")?;
    let Some(payload) = encode_item_entity_data(
        entity_id,
        stack,
        &config.item_protocol_ids,
        &config.data_component_protocol_ids,
        metadata,
    )
    .context("cannot encode item entity stack update")?
    else {
        return Ok(false);
    };
    Ok(connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetEntityData,
            payload,
        },
        exit,
    ))
}

fn queue_item_pickup(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    item_entity_id: EntityId,
    collector_entity_id: EntityId,
    amount: u32,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    let payload = encode_take_item_entity(item_entity_id, collector_entity_id, amount)
        .context("cannot encode item pickup animation")?;
    for connection in connections.values_mut() {
        if connection.item_entities.contains_key(&item_entity_id) {
            connection.queue(
                PlayOutput::ProtocolPacket {
                    kind: PacketKind::TakeItemEntity,
                    payload: payload.clone(),
                },
                exit,
            );
        }
    }
    Ok(())
}

fn queue_experience_orb_pickup(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    orb_entity_id: EntityId,
    collector_entity_id: EntityId,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    let payload = encode_take_item_entity(orb_entity_id, collector_entity_id, 1)
        .context("cannot encode experience orb pickup animation")?;
    for connection in connections.values_mut() {
        if connection.non_player_entities.contains_key(&orb_entity_id) {
            connection.queue(
                PlayOutput::ProtocolPacket {
                    kind: PacketKind::TakeItemEntity,
                    payload: payload.clone(),
                },
                exit,
            );
        }
    }
    Ok(())
}

fn queue_item_remove_for_connection(
    connection: &mut ReplicationConnection,
    entity_id: EntityId,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if connection.item_entities.remove(&entity_id).is_none() {
        return Ok(());
    }
    let payload =
        encode_remove_entities(&[entity_id]).context("cannot encode non-player entity removal")?;
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::RemoveEntities,
            payload,
        },
        exit,
    );
    Ok(())
}

fn queue_item_remove(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    entity_id: EntityId,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    for connection in connections.values_mut() {
        queue_item_remove_for_connection(connection, entity_id, exit)?;
    }
    Ok(())
}

fn queue_non_player_remove_for_connection(
    connection: &mut ReplicationConnection,
    entity_id: EntityId,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if connection.non_player_entities.remove(&entity_id).is_none() {
        return Ok(());
    }
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::RemoveEntities,
            payload: encode_remove_entities(&[entity_id])
                .context("cannot encode non-player entity removal")?,
        },
        exit,
    );
    Ok(())
}

fn queue_non_player_remove(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    entity_id: EntityId,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    for connection in connections.values_mut() {
        queue_non_player_remove_for_connection(connection, entity_id, exit)?;
    }
    Ok(())
}

const fn protocol_game_mode(game_mode: GameMode) -> i8 {
    match game_mode {
        GameMode::Survival => 0,
        GameMode::Creative => 1,
        GameMode::Adventure => 2,
        GameMode::Spectator => 3,
    }
}

const fn protocol_previous_game_mode(game_mode: Option<GameMode>) -> i8 {
    match game_mode {
        Some(game_mode) => protocol_game_mode(game_mode),
        None => -1,
    }
}

fn entity_replication_enabled(registry: &EntityProtocolRegistry) -> bool {
    registry.protocol_id("minecraft:player").is_some()
}

fn connection_tracks_position(
    connection: &ReplicationConnection,
    transform: Transform,
    config: &GameReplicationConfig,
) -> bool {
    let Some(viewer) = connection.viewer_transform else {
        return true;
    };
    let distance_squared = viewer
        .position
        .into_iter()
        .zip(transform.position)
        .map(|(origin, target)| (target - origin).powi(2))
        .sum::<f64>();
    let range = f64::from(config.entity_tracking_range_blocks);
    distance_squared <= range * range
}

fn player_snapshot(
    runtime: &SharedGameRuntime,
    uuid: PlayerUuid,
) -> Result<Option<PlayerEntitySnapshot>> {
    runtime
        .with_state(|state| player_snapshot_from_state(state, uuid))
        .context("cannot read authoritative player snapshot")?
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
        attributes: player.attributes.clone(),
        status_effects: player.status_effects.clone(),
    }))
}

fn reconcile_connection_visibility(
    connection: &mut ReplicationConnection,
    viewer_uuid: PlayerUuid,
    runtime: &SharedGameRuntime,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    let (players, items, non_players) = runtime
        .with_state(|state| -> Result<_> {
            let mut players = Vec::new();
            for player in state
                .players()
                .values()
                .filter(|player| player.connected && player.uuid != viewer_uuid)
            {
                players.push(
                    player_snapshot_from_state(state, player.uuid)?.with_context(|| {
                        format!("online player {:?} has no entity snapshot", player.uuid)
                    })?,
                );
            }
            let items = state
                .entities()
                .iter()
                .filter_map(|(_, entity)| item_snapshot_from_entity(entity))
                .collect::<Vec<_>>();
            let non_players = state
                .entities()
                .iter()
                .filter_map(|(_, entity)| non_player_snapshot_from_entity(entity))
                .collect::<Vec<_>>();
            Ok((players, items, non_players))
        })
        .context("cannot read entity visibility snapshot")??;
    if entity_replication_enabled(&config.entity_protocol_ids) {
        for snapshot in players {
            queue_player_spawn(connection, snapshot, config, exit)?;
        }
    }
    if item_entity_replication_enabled(config) {
        for snapshot in items {
            queue_item_spawn(connection, snapshot, config, exit)?;
        }
    }
    for snapshot in non_players {
        queue_non_player_spawn(connection, snapshot, config, exit)?;
    }
    Ok(())
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
    if !entity_replication_enabled(&config.entity_protocol_ids)
        || !connection.active
        || !connection.healthy
    {
        return Ok(());
    }
    if !connection_tracks_position(connection, snapshot.transform, config) {
        queue_player_entity_remove(connection, snapshot.uuid, exit)?;
        return Ok(());
    }
    if let Some(tracked) = connection.entities.get(&snapshot.uuid) {
        if tracked.entity_id == snapshot.entity_id {
            connection.entities.insert(snapshot.uuid, snapshot);
            return Ok(());
        }
    }
    queue_player_entity_remove(connection, snapshot.uuid, exit)?;
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
    queue_player_state_sync(connection, &snapshot, config, exit)?;
    if connection.healthy {
        connection.entities.insert(snapshot.uuid, snapshot);
    }
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

fn queue_player_entity_remove(
    connection: &mut ReplicationConnection,
    uuid: PlayerUuid,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if let Some(entity_id) = connection
        .entities
        .remove(&uuid)
        .map(|snapshot| snapshot.entity_id)
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
    Ok(())
}

fn queue_player_remove(
    connection: &mut ReplicationConnection,
    uuid: PlayerUuid,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    queue_player_entity_remove(connection, uuid, exit)?;
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

fn queue_set_experience(
    connection: &mut ReplicationConnection,
    experience: Experience,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetExperience,
            payload: encode_set_experience(experience)
                .context("cannot encode player experience")?,
        },
        exit,
    );
    Ok(())
}

fn queue_player_state_sync(
    connection: &mut ReplicationConnection,
    snapshot: &PlayerEntitySnapshot,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if let Some(payload) = encode_update_attributes(
        snapshot.entity_id,
        &snapshot.attributes,
        &config.attribute_protocol_ids,
    )
    .context("cannot encode player attribute snapshot")?
    {
        connection.queue(
            PlayOutput::ProtocolPacket {
                kind: PacketKind::UpdateAttributes,
                payload,
            },
            exit,
        );
    }
    for effect in snapshot.status_effects.iter().map(|(_, effect)| effect) {
        if let Some(payload) =
            encode_update_mob_effect(snapshot.entity_id, effect, &config.mob_effect_protocol_ids)
                .context("cannot encode player status-effect snapshot")?
        {
            connection.queue(
                PlayOutput::ProtocolPacket {
                    kind: PacketKind::UpdateMobEffect,
                    payload,
                },
                exit,
            );
        }
    }
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
    use ferrum_game::{
        CommandSource, DamageKind, DamageSource, HOTBAR_END, HOTBAR_START, ItemStack,
        MAIN_INVENTORY_START, StatusEffectId, StatusEffectInstance, Transform,
    };
    use ferrum_runtime::{BoundedInputQueue, ConnectionId, worker_channel};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    fn entity_config() -> GameReplicationConfig {
        GameReplicationConfig {
            entity_protocol_ids: EntityProtocolRegistry::new([
                ("minecraft:experience_orb", 49),
                ("minecraft:item", 71),
                ("minecraft:player", 148),
                ("minecraft:zombie", 153),
            ])
            .unwrap(),
            item_protocol_ids: ItemProtocolRegistry::new([("minecraft:stone", 1)]).unwrap(),
            item_entity_metadata: Some(
                ItemEntityMetadataProtocol::new(
                    ferrum_version_26_1_2::ITEM_ENTITY_STACK_METADATA_INDEX,
                    ferrum_version_26_1_2::ITEM_STACK_ENTITY_DATA_SERIALIZER_ID,
                )
                .unwrap(),
            ),
            experience_orb_metadata: Some(
                ExperienceOrbMetadataProtocol::new(
                    ferrum_version_26_1_2::EXPERIENCE_ORB_VALUE_METADATA_INDEX,
                    ferrum_version_26_1_2::INT_ENTITY_DATA_SERIALIZER_ID,
                )
                .unwrap(),
            ),
            world: Some(RomPackWorld {
                data_version: ferrum_version_26_1_2::WORLD_VERSION,
                overworld_min_section_y: ferrum_version_26_1_2::OVERWORLD_MIN_SECTION_Y,
                overworld_section_count: ferrum_version_26_1_2::OVERWORLD_SECTION_COUNT,
                dimension: ferrum_version_26_1_2::OVERWORLD_DIMENSION.to_owned(),
                dimension_type_id: ferrum_version_26_1_2::OVERWORLD_DIMENSION_TYPE_ID,
                sea_level: ferrum_version_26_1_2::OVERWORLD_SEA_LEVEL,
                floor_y: ferrum_version_26_1_2::FLAT_WORLD_FLOOR_Y,
                spawn_x: ferrum_version_26_1_2::FLAT_WORLD_SPAWN_X,
                spawn_z: ferrum_version_26_1_2::FLAT_WORLD_SPAWN_Z,
                block_states: ferrum_rompack::RomPackBlockStates {
                    air: ferrum_version_26_1_2::AIR_BLOCK_STATE_ID,
                    stone: ferrum_version_26_1_2::STONE_BLOCK_STATE_ID,
                    grass: ferrum_version_26_1_2::GRASS_BLOCK_STATE_ID,
                    dirt: ferrum_version_26_1_2::DIRT_BLOCK_STATE_ID,
                    bedrock: ferrum_version_26_1_2::BEDROCK_BLOCK_STATE_ID,
                },
                biomes: ferrum_rompack::RomPackBiomes {
                    plains: ferrum_version_26_1_2::PLAINS_BIOME_ID,
                },
            }),
            ..GameReplicationConfig::default()
        }
    }

    fn player_state_sync_config() -> GameReplicationConfig {
        let mut config = entity_config();
        config.attribute_protocol_ids = ProtocolIdRegistry::new([
            ("minecraft:armor", 0),
            ("minecraft:armor_toughness", 1),
            ("minecraft:attack_damage", 2),
            ("minecraft:attack_speed", 4),
            ("minecraft:block_interaction_range", 6),
            ("minecraft:entity_interaction_range", 10),
            ("minecraft:gravity", 14),
            ("minecraft:knockback_resistance", 16),
            ("minecraft:max_health", 19),
            ("minecraft:movement_speed", 22),
            ("minecraft:safe_fall_distance", 24),
            ("minecraft:step_height", 28),
        ])
        .unwrap();
        config.mob_effect_protocol_ids = ProtocolIdRegistry::new([("minecraft:haste", 2)]).unwrap();
        config.damage_type_protocol_ids =
            ProtocolIdRegistry::new([("minecraft:player_attack", 34)]).unwrap();
        config
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
                    kind: PacketKind::SetHealth | PacketKind::SetExperience,
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

    fn recv_protocol_until(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
        expected: PacketKind,
    ) -> Vec<u8> {
        loop {
            if let PlayOutput::ProtocolPacket { kind, payload } =
                recv_raw_output(writer, workers, inputs)
                && kind == expected
            {
                return payload;
            }
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
        service.control().activate(steve).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().register(alex, alex_reader).unwrap();
        service.control().activate(alex).unwrap();
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
        service.control().activate(steve).unwrap();
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
        service.control().activate(steve).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().register(alex, alex_reader).unwrap();
        service.control().activate(alex).unwrap();
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

        service.control().activate(steve).unwrap();
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

        service.control().activate(alex).unwrap();
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

        service.control().activate(steve).unwrap();
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
        service.control().activate(alex).unwrap();
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
    fn tracks_players_and_moving_items_within_the_bounded_range() {
        let game = SharedGameRuntime::vanilla_overworld();
        let mut config = entity_config();
        config.entity_tracking_range_blocks = 8;
        let service = spawn_game_replication(&game, config).unwrap();
        let steve = PlayerUuid::new(251);
        let alex = PlayerUuid::new(252);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(256).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(251),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let (alex_reader, _alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(252),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(256).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        service.control().activate(steve).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        recv_protocol_until(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );

        service.control().register(alex, alex_reader).unwrap();
        service.control().activate(alex).unwrap();
        game.connect_player(
            alex,
            "Alex",
            Transform::new([32.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex joined the game"
        ));
        assert!(steve_writer.try_recv_output().is_err());

        game.move_player(
            alex,
            Transform::new([4.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
        )
        .unwrap();
        recv_protocol_until(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::AddEntity,
        );
        game.move_player(
            alex,
            Transform::new([32.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
        )
        .unwrap();
        recv_protocol_until(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );

        game.spawn_item_entity(
            Transform::new([3.5, 70.0, 0.5], 0.0, 0.0, false).unwrap(),
            ItemStack::new("minecraft:stone", 1).unwrap(),
            Velocity::new([0.1, 0.0, 0.0]).unwrap(),
            None,
        )
        .unwrap();
        recv_protocol_until(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::AddEntity,
        );
        game.tick().unwrap();
        recv_protocol_until(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::MoveEntityPosition,
        );

        game.move_player(
            steve,
            Transform::new([64.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
        )
        .unwrap();
        recv_protocol_until(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );
        service.shutdown().unwrap();
    }

    #[test]
    fn replicates_non_player_entity_spawn_motion_teleport_and_remove() {
        let game = SharedGameRuntime::vanilla_overworld();
        let mut config = entity_config();
        config.entity_tracking_range_blocks = 8;
        let service = spawn_game_replication(&game, config).unwrap();
        let player = PlayerUuid::new(253);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(128).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(253),
            NonZeroUsize::new(64).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(128).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(player, reader).unwrap();
        service.control().activate(player).unwrap();
        game.connect_player(player, "Steve", spawn()).unwrap();
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );

        let spawned = game
            .spawn_entity(
                ferrum_game::EntityType::new("minecraft:zombie").unwrap(),
                Transform::new([2.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
                Velocity::default(),
                EntityPayload::Living(ferrum_game::LivingEntityData::new(20.0).unwrap()),
            )
            .unwrap();
        let entity_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        let add = recv_protocol_until(&writer, &mut workers, &mut inputs, PacketKind::AddEntity);
        let (encoded_entity_id, entity_id_bytes) = read_varint(&add);
        assert_eq!(encoded_entity_id, i32::try_from(entity_id.get()).unwrap());
        assert_eq!(read_varint(&add[entity_id_bytes + 16..]).0, 153);
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );

        game.move_entity(
            entity_id,
            Transform::new([3.5, 65.0, 0.5], 45.0, 0.0, true).unwrap(),
        )
        .unwrap();
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::MoveEntityPositionRotation,
        );
        recv_protocol_until(&writer, &mut workers, &mut inputs, PacketKind::RotateHead);

        game.move_entity(
            entity_id,
            Transform::new([-7.0, 65.0, 0.5], 45.0, 0.0, true).unwrap(),
        )
        .unwrap();
        let teleport = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::TeleportEntity,
        );
        assert_eq!(
            read_varint(&teleport).0,
            i32::try_from(entity_id.get()).unwrap()
        );

        game.set_entity_velocity(entity_id, Velocity::new([0.2, 0.0, -0.1]).unwrap())
            .unwrap();
        let motion = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityMotion,
        );
        assert_eq!(
            read_varint(&motion).0,
            i32::try_from(entity_id.get()).unwrap()
        );
        game.damage_entity(entity_id, 1.0).unwrap();
        let hurt = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::HurtAnimation,
        );
        assert_eq!(
            read_varint(&hurt).0,
            i32::try_from(entity_id.get()).unwrap()
        );

        game.move_entity(
            entity_id,
            Transform::new([32.5, 65.0, 0.5], 45.0, 0.0, true).unwrap(),
        )
        .unwrap();
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );
        game.move_entity(
            entity_id,
            Transform::new([4.5, 65.0, 0.5], 45.0, 0.0, true).unwrap(),
        )
        .unwrap();
        recv_protocol_until(&writer, &mut workers, &mut inputs, PacketKind::AddEntity);
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );
        game.damage_entity(entity_id, 100.0).unwrap();
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::HurtAnimation,
        );
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );
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

        service.control().activate(steve).unwrap();
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

        service.control().activate(alex).unwrap();
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

        service.control().activate(steve).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        assert_eq!(
            recv_raw_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload: vec![0x41, 0xa0, 0, 0, 0x14, 0x40, 0xa0, 0, 0],
            }
        );
        assert!(matches!(
            recv_raw_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetExperience,
                ..
            }
        ));

        service.control().register(alex, alex_reader).unwrap();

        service.control().activate(alex).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        assert!(matches!(
            recv_raw_output(&alex_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        assert!(matches!(
            recv_raw_output(&alex_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetExperience,
                ..
            }
        ));
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex joined the game"
        ));

        game.damage_player(steve, 4.0).unwrap();
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::HurtAnimation,
        );
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
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::HurtAnimation,
        );
        assert!(matches!(
            recv_raw_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload,
            } if f32::from_be_bytes(payload[0..4].try_into().unwrap()) == 0.0
        ));
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerCombatKill,
        );
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SetContainerContent {
                container_id: 0,
                ..
            }
        ));
        service.shutdown().unwrap();
    }

    #[test]
    fn replicates_attributes_effects_damage_sources_and_velocity_to_watchers() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, player_state_sync_config()).unwrap();
        let steve = PlayerUuid::new(451);
        let observer = PlayerUuid::new(452);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(256).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(451),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let (observer_reader, observer_writer) = register_play_connection(
            &connector,
            ConnectionId::new(452),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(256).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        service.control().activate(steve).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        let initial_attributes = recv_protocol_until(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::UpdateAttributes,
        );
        let (steve_entity_id, entity_id_bytes) = read_varint(&initial_attributes);
        assert_eq!(read_varint(&initial_attributes[entity_id_bytes..]).0, 12);

        service
            .control()
            .register(observer, observer_reader)
            .unwrap();
        service.control().activate(observer).unwrap();
        let observer_snapshot = recv_protocol_until(
            &observer_writer,
            &mut workers,
            &mut inputs,
            PacketKind::UpdateAttributes,
        );
        assert_eq!(read_varint(&observer_snapshot).0, steve_entity_id);

        game.set_player_attribute_base(steve, "minecraft:movement_speed", 0.2)
            .unwrap();
        for writer in [&steve_writer, &observer_writer] {
            let payload = recv_protocol_until(
                writer,
                &mut workers,
                &mut inputs,
                PacketKind::UpdateAttributes,
            );
            let (entity_id, entity_id_bytes) = read_varint(&payload);
            let (count, count_bytes) = read_varint(&payload[entity_id_bytes..]);
            assert_eq!(entity_id, steve_entity_id);
            assert_eq!(count, 1);
            assert_eq!(read_varint(&payload[entity_id_bytes + count_bytes..]).0, 22);
        }

        game.add_status_effect(
            steve,
            StatusEffectInstance::new(StatusEffectId::new("minecraft:haste").unwrap(), 1, 200)
                .unwrap(),
        )
        .unwrap();
        for writer in [&steve_writer, &observer_writer] {
            let payload = recv_protocol_until(
                writer,
                &mut workers,
                &mut inputs,
                PacketKind::UpdateMobEffect,
            );
            let (entity_id, entity_id_bytes) = read_varint(&payload);
            assert_eq!(entity_id, steve_entity_id);
            assert_eq!(read_varint(&payload[entity_id_bytes..]).0, 2);
        }

        game.apply_knockback(steve, [1.0, 0.0], 0.4).unwrap();
        for writer in [&steve_writer, &observer_writer] {
            let payload = recv_protocol_until(
                writer,
                &mut workers,
                &mut inputs,
                PacketKind::SetEntityMotion,
            );
            assert_eq!(read_varint(&payload).0, steve_entity_id);
        }

        let source = DamageSource {
            kind: DamageKind::PlayerAttack,
            attacker: None,
            direct_entity: None,
            bypasses_armor: false,
            bypasses_invulnerability: false,
        };
        game.damage_player_with_source(steve, 1.0, source).unwrap();
        for writer in [&steve_writer, &observer_writer] {
            let payload =
                recv_protocol_until(writer, &mut workers, &mut inputs, PacketKind::DamageEvent);
            let (entity_id, entity_id_bytes) = read_varint(&payload);
            assert_eq!(entity_id, steve_entity_id);
            assert_eq!(read_varint(&payload[entity_id_bytes..]).0, 34);
        }

        game.remove_status_effect(steve, "minecraft:haste").unwrap();
        for writer in [&steve_writer, &observer_writer] {
            let payload = recv_protocol_until(
                writer,
                &mut workers,
                &mut inputs,
                PacketKind::RemoveMobEffect,
            );
            let (entity_id, entity_id_bytes) = read_varint(&payload);
            assert_eq!(entity_id, steve_entity_id);
            assert_eq!(read_varint(&payload[entity_id_bytes..]).0, 2);
        }
        service.shutdown().unwrap();
    }

    #[test]
    fn sends_hurt_death_respawn_teleport_and_health_in_order() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let steve = PlayerUuid::new(501);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(128).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(501),
            NonZeroUsize::new(64).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(128).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, reader).unwrap();
        service.control().activate(steve).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        assert!(matches!(
            recv_raw_output(&writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );

        game.damage_player(steve, 20.0).unwrap();
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::HurtAnimation,
        );
        assert!(matches!(
            recv_raw_output(&writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerCombatKill,
        );
        assert!(matches!(
            recv_output(&writer, &mut workers, &mut inputs),
            PlayOutput::SetContainerContent {
                container_id: 0,
                ..
            }
        ));

        let respawn = Transform::new([0.5, 64.0, 0.5], 0.0, 0.0, false).unwrap();
        game.respawn_player(steve, respawn).unwrap();
        recv_protocol(&writer, &mut workers, &mut inputs, PacketKind::Respawn);
        assert!(matches!(
            recv_output(&writer, &mut workers, &mut inputs),
            PlayOutput::PlayerTeleport { transform, .. } if transform == respawn
        ));
        assert!(matches!(
            recv_raw_output(&writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        service.shutdown().unwrap();
    }

    #[test]
    fn activation_snapshots_preexisting_item_entities() {
        let game = SharedGameRuntime::vanilla_overworld();
        let spawned = game
            .spawn_item_entity(
                spawn(),
                ItemStack::new("minecraft:stone", 3).unwrap(),
                Velocity::default(),
                None,
            )
            .unwrap();
        let item_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let player = PlayerUuid::new(51);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(51),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(player, reader).unwrap();
        service.control().activate(player).unwrap();

        let add = recv_protocol_until(&writer, &mut workers, &mut inputs, PacketKind::AddEntity);
        let (encoded_entity_id, entity_id_bytes) = read_varint(&add);
        assert_eq!(encoded_entity_id, i32::try_from(item_id.get()).unwrap());
        let type_offset = entity_id_bytes + 16;
        assert_eq!(read_varint(&add[type_offset..]).0, 71);

        let metadata = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );
        let (encoded_entity_id, entity_id_bytes) = read_varint(&metadata);
        assert_eq!(encoded_entity_id, i32::try_from(item_id.get()).unwrap());
        assert_eq!(
            &metadata[entity_id_bytes..],
            &[
                ferrum_version_26_1_2::ITEM_ENTITY_STACK_METADATA_INDEX,
                ferrum_version_26_1_2::ITEM_STACK_ENTITY_DATA_SERIALIZER_ID as u8,
                3,
                1,
                0,
                0,
                0xff,
            ]
        );
        service.shutdown().unwrap();
    }

    #[test]
    fn activation_snapshots_and_removes_preexisting_experience_orbs() {
        let game = SharedGameRuntime::vanilla_overworld();
        let spawned = game
            .spawn_entity(
                ferrum_game::EntityType::new("minecraft:experience_orb").unwrap(),
                spawn(),
                Velocity::default(),
                EntityPayload::ExperienceOrb { value: 300 },
            )
            .unwrap();
        let orb_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let player = PlayerUuid::new(54);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(54),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(player, reader).unwrap();
        service.control().activate(player).unwrap();

        let add = recv_protocol_until(&writer, &mut workers, &mut inputs, PacketKind::AddEntity);
        let (encoded_entity_id, entity_id_bytes) = read_varint(&add);
        assert_eq!(encoded_entity_id, i32::try_from(orb_id.get()).unwrap());
        assert_eq!(read_varint(&add[entity_id_bytes + 16..]).0, 49);

        let metadata = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );
        let (encoded_entity_id, entity_id_bytes) = read_varint(&metadata);
        assert_eq!(encoded_entity_id, i32::try_from(orb_id.get()).unwrap());
        assert_eq!(
            &metadata[entity_id_bytes..],
            &[
                ferrum_version_26_1_2::EXPERIENCE_ORB_VALUE_METADATA_INDEX,
                ferrum_version_26_1_2::INT_ENTITY_DATA_SERIALIZER_ID as u8,
                0xac,
                0x02,
                0xff,
            ]
        );

        game.despawn_entity(orb_id).unwrap();
        let remove = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );
        let (count, count_bytes) = read_varint(&remove);
        assert_eq!(count, 1);
        assert_eq!(
            read_varint(&remove[count_bytes..]).0,
            i32::try_from(orb_id.get()).unwrap()
        );
        service.shutdown().unwrap();
    }

    #[test]
    fn experience_orb_pickup_animates_updates_experience_and_removes_the_orb() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let player = PlayerUuid::new(55);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(64).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(55),
            NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(64).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(player, reader).unwrap();
        service.control().activate(player).unwrap();
        game.connect_player(player, "Steve", spawn()).unwrap();

        let initial_experience = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetExperience,
        );
        assert_eq!(
            initial_experience,
            encode_set_experience(Experience::default()).unwrap()
        );
        let collector_entity_id = game
            .with_state(|state| state.player(player).unwrap().entity_id.unwrap())
            .unwrap();
        let spawned = game
            .spawn_entity(
                ferrum_game::EntityType::new("minecraft:experience_orb").unwrap(),
                spawn(),
                Velocity::default(),
                EntityPayload::ExperienceOrb { value: 10 },
            )
            .unwrap();
        let orb_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        recv_protocol_until(&writer, &mut workers, &mut inputs, PacketKind::AddEntity);
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );

        game.tick().unwrap();
        let take = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::TakeItemEntity,
        );
        let (taken_id, taken_bytes) = read_varint(&take);
        let (collector_id, collector_bytes) = read_varint(&take[taken_bytes..]);
        let (amount, _) = read_varint(&take[taken_bytes + collector_bytes..]);
        assert_eq!(taken_id, i32::try_from(orb_id.get()).unwrap());
        assert_eq!(
            collector_id,
            i32::try_from(collector_entity_id.get()).unwrap()
        );
        assert_eq!(amount, 1);

        let experience = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetExperience,
        );
        let authoritative_experience = game
            .with_state(|state| state.player(player).unwrap().experience)
            .unwrap();
        assert_eq!(authoritative_experience.level, 1);
        assert_eq!(authoritative_experience.total, 10);
        assert_eq!(
            experience,
            encode_set_experience(authoritative_experience).unwrap()
        );
        let remove = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );
        let (count, count_bytes) = read_varint(&remove);
        assert_eq!(count, 1);
        assert_eq!(
            read_varint(&remove[count_bytes..]).0,
            i32::try_from(orb_id.get()).unwrap()
        );
        service.shutdown().unwrap();
    }

    #[test]
    fn partial_and_full_item_pickups_replicate_animation_metadata_and_removal() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let player = PlayerUuid::new(52);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(64).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(52),
            NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(64).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(player, reader).unwrap();
        service.control().activate(player).unwrap();
        game.connect_player(player, "Alex", spawn()).unwrap();
        let collector_entity_id = game
            .with_state(|state| state.player(player).unwrap().entity_id.unwrap())
            .unwrap();
        game.with_state_mut(|state| {
            let inventory = &mut state.player_mut(player).unwrap().inventory;
            for slot in MAIN_INVENTORY_START..=HOTBAR_END {
                inventory
                    .set_slot(slot, Some(ItemStack::new("minecraft:dirt", 64).unwrap()))
                    .unwrap();
            }
            inventory
                .set_slot(
                    MAIN_INVENTORY_START,
                    Some(ItemStack::new("minecraft:stone", 63).unwrap()),
                )
                .unwrap();
            Ok(())
        })
        .unwrap();

        let spawned = game
            .spawn_item_entity(
                spawn(),
                ItemStack::new("minecraft:stone", 3).unwrap(),
                Velocity::default(),
                None,
            )
            .unwrap();
        let item_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        recv_protocol_until(&writer, &mut workers, &mut inputs, PacketKind::AddEntity);
        recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );
        game.with_state_mut(|state| {
            state
                .entities_mut()
                .get_mut(item_id)
                .unwrap()
                .item_mut()
                .unwrap()
                .pickup_delay_ticks = 0;
            Ok(())
        })
        .unwrap();

        game.pickup_nearby_items(player, 2.0).unwrap();
        let take = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::TakeItemEntity,
        );
        let (taken_entity_id, taken_bytes) = read_varint(&take);
        let (collector_id, collector_bytes) = read_varint(&take[taken_bytes..]);
        let (amount, _) = read_varint(&take[taken_bytes + collector_bytes..]);
        assert_eq!(taken_entity_id, i32::try_from(item_id.get()).unwrap());
        assert_eq!(
            collector_id,
            i32::try_from(collector_entity_id.get()).unwrap()
        );
        assert_eq!(amount, 1);
        let update = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEntityData,
        );
        let (_, update_entity_id_bytes) = read_varint(&update);
        assert_eq!(
            &update[update_entity_id_bytes..],
            &[
                ferrum_version_26_1_2::ITEM_ENTITY_STACK_METADATA_INDEX,
                ferrum_version_26_1_2::ITEM_STACK_ENTITY_DATA_SERIALIZER_ID as u8,
                2,
                1,
                0,
                0,
                0xff,
            ]
        );

        game.with_state_mut(|state| {
            state.player_mut(player).unwrap().inventory.clear();
            Ok(())
        })
        .unwrap();
        game.pickup_nearby_items(player, 2.0).unwrap();
        let take = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::TakeItemEntity,
        );
        let (_, entity_id_bytes) = read_varint(&take);
        let (_, collector_id_bytes) = read_varint(&take[entity_id_bytes..]);
        assert_eq!(
            read_varint(&take[entity_id_bytes + collector_id_bytes..]).0,
            2
        );
        let remove = recv_protocol_until(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::RemoveEntities,
        );
        let (removed_count, count_bytes) = read_varint(&remove);
        assert_eq!(removed_count, 1);
        assert_eq!(
            read_varint(&remove[count_bytes..]).0,
            i32::try_from(item_id.get()).unwrap()
        );
        service.shutdown().unwrap();
    }

    #[test]
    fn registration_is_silent_until_explicit_activation() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, GameReplicationConfig::default()).unwrap();
        let steve = PlayerUuid::new(601);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(601),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        thread::sleep(Duration::from_millis(10));
        ingest(&mut workers, &mut inputs);
        assert!(writer.try_recv_output().is_err());
        service.control().activate(steve).unwrap();
        assert!(matches!(
            recv_raw_output(&writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        service.shutdown().unwrap();
    }

    #[test]
    fn repeated_spawn_snapshot_is_idempotent() {
        let game = SharedGameRuntime::vanilla_overworld();
        let steve = PlayerUuid::new(602);
        game.connect_player(steve, "Steve", spawn()).unwrap();
        let snapshot = player_snapshot(&game, steve).unwrap().unwrap();
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());
        let (reader, _writer) = register_play_connection(
            &connector,
            ConnectionId::new(602),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        let mut connection = ReplicationConnection::new(reader, 16);
        connection.activate().unwrap();
        let mut exit = GameReplicationExit::default();
        queue_player_spawn(
            &mut connection,
            snapshot.clone(),
            &entity_config(),
            &mut exit,
        )
        .unwrap();
        let pending = connection.pending.len();
        queue_player_spawn(&mut connection, snapshot, &entity_config(), &mut exit).unwrap();
        assert_eq!(connection.pending.len(), pending);
        assert_eq!(connection.entities.len(), 1);
    }

    #[test]
    fn output_overflow_disconnects_instead_of_corrupting_tracking_state() {
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(8).unwrap());
        let (reader, _writer) = register_play_connection(
            &connector,
            ConnectionId::new(603),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(8).unwrap();
        ingest(&mut workers, &mut inputs);
        let mut connection = ReplicationConnection::new(reader, 1);
        connection.activate().unwrap();
        let mut exit = GameReplicationExit::default();
        assert!(connection.queue(
            PlayOutput::SystemChat {
                message: "first".to_owned(),
                overlay: false,
            },
            &mut exit
        ));
        assert!(!connection.queue(
            PlayOutput::SystemChat {
                message: "second".to_owned(),
                overlay: false,
            },
            &mut exit
        ));
        assert!(!connection.healthy);
        assert!(connection.pending.is_empty());
        assert!(connection.entities.is_empty());
        assert!(connection.item_entities.is_empty());
        assert!(connection.non_player_entities.is_empty());
        assert_eq!(exit.dropped_outputs, 1);
    }
}
