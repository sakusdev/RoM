from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


# Add a protocol-aware semantic output that the live writer resolves through the
# active generated packet table.
path = Path("crates/ferrum-server/src/authoritative_runtime.rs")
text = path.read_text()
text = replace_once(
    text,
    "use ferrum_play::PlayerMovement;",
    "use ferrum_play::PlayerMovement;\nuse ferrum_protocol::PacketKind;",
    "authoritative packet kind import",
)
text = replace_once(
    text,
    """    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
""",
    """    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
    /// A version-neutral packet payload resolved by the active protocol table.
    ProtocolPacket { kind: PacketKind, payload: Vec<u8> },
""",
    "protocol output variant",
)
path.write_text(text)


# Extend gameplay replication with tab-list and player entity lifecycle/movement.
path = Path("crates/ferrum-server/src/game_replication.rs")
text = path.read_text()
text = replace_once(
    text,
    "use ferrum_game::{GameEvent, PLAYER_INVENTORY_SLOTS, PlayerUuid};",
    """use ferrum_game::{
    EntityId, GameEvent, GameMode, PLAYER_INVENTORY_SLOTS, PlayerUuid, Transform, Velocity,
};
use ferrum_play::{
    EntityMovementKind, EntityProtocolRegistry, PlayerInfoEntry, encode_add_entity,
    encode_entity_movement, encode_player_info_remove, encode_player_info_update,
    encode_remove_entities, encode_rotate_head, encode_teleport_entity,
};
use ferrum_protocol::PacketKind;""",
    "replication imports",
)
text = replace_once(
    text,
    """pub struct GameReplicationConfig {
    pub event_capacity: NonZeroUsize,
    pub command_capacity: NonZeroUsize,
    pub pending_output_limit: NonZeroUsize,
    pub poll_interval: Duration,
}
""",
    """pub struct GameReplicationConfig {
    pub event_capacity: NonZeroUsize,
    pub command_capacity: NonZeroUsize,
    pub pending_output_limit: NonZeroUsize,
    pub poll_interval: Duration,
    pub entity_protocol_ids: EntityProtocolRegistry,
}
""",
    "replication config field",
)
text = replace_once(
    text,
    """            poll_interval: DEFAULT_POLL_INTERVAL,
        }
""",
    """            poll_interval: DEFAULT_POLL_INTERVAL,
            entity_protocol_ids: EntityProtocolRegistry::default(),
        }
""",
    "replication config default",
)
text = replace_once(
    text,
    """struct ReplicationConnection {
""",
    """#[derive(Debug, Clone)]
struct ReplicatedPlayer {
    uuid: PlayerUuid,
    name: String,
    entity_id: EntityId,
    game_mode: GameMode,
    transform: Transform,
    velocity: Velocity,
}

struct ReplicationConnection {
""",
    "replicated player state",
)
text = replace_once(
    text,
    """    let mut connections = BTreeMap::new();
    let mut exit = GameReplicationExit::default();
""",
    """    let mut connections = BTreeMap::new();
    let mut players = BTreeMap::new();
    let mut exit = GameReplicationExit::default();
""",
    "replication player map",
)
text = replace_once(
    text,
    """            config.pending_output_limit.get(),
            &mut exit,
""",
    """            config.pending_output_limit.get(),
            &config.entity_protocol_ids,
            &mut exit,
""",
    "command entity registry",
)
text = replace_once(
    text,
    """                dispatch_event(event, &mut connections, &mut exit);
                for _ in 1..MAX_EVENTS_PER_POLL {
                    match subscription.try_recv() {
                        Ok(event) => dispatch_event(event, &mut connections, &mut exit),
""",
    """                dispatch_event(
                    &runtime,
                    event,
                    &mut players,
                    &config.entity_protocol_ids,
                    &mut connections,
                    &mut exit,
                )?;
                for _ in 1..MAX_EVENTS_PER_POLL {
                    match subscription.try_recv() {
                        Ok(event) => dispatch_event(
                            &runtime,
                            event,
                            &mut players,
                            &config.entity_protocol_ids,
                            &mut connections,
                            &mut exit,
                        )?,
""",
    "event dispatch calls",
)
text = replace_once(
    text,
    """    pending_limit: usize,
    exit: &mut GameReplicationExit,
""",
    """    pending_limit: usize,
    entity_protocol_ids: &EntityProtocolRegistry,
    exit: &mut GameReplicationExit,
""",
    "process command signature",
)
text = replace_once(
    text,
    """            } => {
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
""",
    """            } => {
                let result = if connections.contains_key(&uuid) {
                    Err(format!(
                        "player {uuid:?} is already registered for replication"
                    ))
                } else {
                    connections.insert(uuid, ReplicationConnection::new(endpoint, pending_limit));
                    let sync_result = sync_existing_players(
                        runtime,
                        uuid,
                        connections
                            .get_mut(&uuid)
                            .expect("newly inserted replication connection exists"),
                        entity_protocol_ids,
                        exit,
                    )
                    .map_err(|error| error.to_string());
                    if sync_result.is_err() {
                        connections.remove(&uuid);
                    }
                    sync_result
                };
                let _ = reply.send(result);
            }
""",
    "registration entity sync",
)
old_dispatch_start = text.index("fn dispatch_event(")
old_dispatch_end = text.index("\nfn target_chat(", old_dispatch_start)
new_dispatch = r'''fn entity_replication_enabled(registry: &EntityProtocolRegistry) -> bool {
    registry.protocol_id("minecraft:player").is_some()
}

fn snapshot_connected_players(runtime: &SharedGameRuntime) -> Result<Vec<ReplicatedPlayer>> {
    runtime.with_state(|state| {
        state
            .players()
            .values()
            .filter(|player| player.connected)
            .filter_map(|player| {
                let entity_id = player.entity_id?;
                let entity = state.entities().get(entity_id)?;
                Some(ReplicatedPlayer {
                    uuid: player.uuid,
                    name: player.name.clone(),
                    entity_id,
                    game_mode: player.game_mode,
                    transform: entity.transform,
                    velocity: entity.velocity,
                })
            })
            .collect()
    })
}

fn snapshot_player(
    runtime: &SharedGameRuntime,
    uuid: PlayerUuid,
) -> Result<Option<ReplicatedPlayer>> {
    runtime.with_state(|state| {
        let player = state.player(uuid)?;
        if !player.connected {
            return None;
        }
        let entity_id = player.entity_id?;
        let entity = state.entities().get(entity_id)?;
        Some(ReplicatedPlayer {
            uuid,
            name: player.name.clone(),
            entity_id,
            game_mode: player.game_mode,
            transform: entity.transform,
            velocity: entity.velocity,
        })
    })
}

fn player_info_output(player: &ReplicatedPlayer) -> Result<PlayOutput> {
    let payload = encode_player_info_update(&[PlayerInfoEntry::new(
        player.uuid,
        player.name.clone(),
        player.game_mode,
    )])?;
    Ok(PlayOutput::ProtocolPacket {
        kind: PacketKind::PlayerInfoUpdate,
        payload,
    })
}

fn add_entity_output(
    player: &ReplicatedPlayer,
    registry: &EntityProtocolRegistry,
) -> Result<Option<PlayOutput>> {
    Ok(encode_add_entity(
        player.entity_id,
        player.uuid,
        "minecraft:player",
        player.transform,
        player.velocity,
        registry,
    )?
    .map(|payload| PlayOutput::ProtocolPacket {
        kind: PacketKind::AddEntity,
        payload,
    }))
}

fn sync_existing_players(
    runtime: &SharedGameRuntime,
    joining: PlayerUuid,
    connection: &mut ReplicationConnection,
    registry: &EntityProtocolRegistry,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !entity_replication_enabled(registry) {
        return Ok(());
    }
    for player in snapshot_connected_players(runtime)? {
        if player.uuid == joining {
            continue;
        }
        connection.queue(player_info_output(&player)?, exit);
        if let Some(output) = add_entity_output(&player, registry)? {
            connection.queue(output, exit);
        }
    }
    Ok(())
}

fn movement_packet_kind(kind: EntityMovementKind) -> PacketKind {
    match kind {
        EntityMovementKind::Position => PacketKind::MoveEntityPosition,
        EntityMovementKind::PositionRotation => PacketKind::MoveEntityPositionRotation,
        EntityMovementKind::Rotation => PacketKind::MoveEntityRotation,
        EntityMovementKind::Teleport => PacketKind::TeleportEntity,
    }
}

fn dispatch_event(
    runtime: &SharedGameRuntime,
    event: GameEvent,
    players: &mut BTreeMap<PlayerUuid, ReplicatedPlayer>,
    entity_protocol_ids: &EntityProtocolRegistry,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    exit.events = exit.events.saturating_add(1);
    match event {
        GameEvent::PlayerConnected { uuid, name, .. } => {
            broadcast_except(
                connections,
                uuid,
                PlayOutput::SystemChat {
                    message: format!("{name} joined the game"),
                    overlay: false,
                },
                exit,
            );
            if entity_replication_enabled(entity_protocol_ids)
                && let Some(player) = snapshot_player(runtime, uuid)?
            {
                players.insert(uuid, player.clone());
                broadcast(connections, player_info_output(&player)?, exit);
                if let Some(output) = add_entity_output(&player, entity_protocol_ids)? {
                    broadcast_except(connections, uuid, output, exit);
                }
            }
        }
        GameEvent::PlayerDisconnected {
            uuid,
            name,
            entity_id,
        } => {
            broadcast_except(
                connections,
                uuid,
                PlayOutput::SystemChat {
                    message: format!("{name} left the game"),
                    overlay: false,
                },
                exit,
            );
            players.remove(&uuid);
            if entity_replication_enabled(entity_protocol_ids) {
                if let Some(entity_id) = entity_id {
                    broadcast(
                        connections,
                        PlayOutput::ProtocolPacket {
                            kind: PacketKind::RemoveEntities,
                            payload: encode_remove_entities(&[entity_id])?,
                        },
                        exit,
                    );
                }
                broadcast(
                    connections,
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::PlayerInfoRemove,
                        payload: encode_player_info_remove(&[uuid])?,
                    },
                    exit,
                );
            }
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
            if entity_replication_enabled(entity_protocol_ids) {
                if let Some(player) = players.get_mut(&uuid) {
                    let previous = player.transform;
                    player.transform = transform;
                    player.entity_id = entity_id;
                    if let Some(movement) =
                        encode_entity_movement(entity_id, previous, transform)?
                    {
                        broadcast_except(
                            connections,
                            uuid,
                            PlayOutput::ProtocolPacket {
                                kind: movement_packet_kind(movement.kind),
                                payload: movement.payload,
                            },
                            exit,
                        );
                    }
                    if previous.yaw != transform.yaw {
                        broadcast_except(
                            connections,
                            uuid,
                            PlayOutput::ProtocolPacket {
                                kind: PacketKind::RotateHead,
                                payload: encode_rotate_head(entity_id, transform.yaw)?,
                            },
                            exit,
                        );
                    }
                } else if let Some(player) = snapshot_player(runtime, uuid)? {
                    players.insert(uuid, player);
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
            if entity_replication_enabled(entity_protocol_ids) {
                if let Some(player) = players.get_mut(&uuid) {
                    player.entity_id = entity_id;
                    player.transform = transform;
                    broadcast_except(
                        connections,
                        uuid,
                        PlayOutput::ProtocolPacket {
                            kind: PacketKind::TeleportEntity,
                            payload: encode_teleport_entity(entity_id, transform, player.velocity)?,
                        },
                        exit,
                    );
                    broadcast_except(
                        connections,
                        uuid,
                        PlayOutput::ProtocolPacket {
                            kind: PacketKind::RotateHead,
                            payload: encode_rotate_head(entity_id, transform.yaw)?,
                        },
                        exit,
                    );
                } else if let Some(player) = snapshot_player(runtime, uuid)? {
                    players.insert(uuid, player);
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
            if entity_replication_enabled(entity_protocol_ids)
                && let Some(player) = players.get_mut(&uuid)
            {
                player.game_mode = current;
                broadcast(connections, player_info_output(player)?, exit);
            }
        }
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
        GameEvent::SelectedHotbarChanged { .. }
        | GameEvent::TimeChanged { .. }
        | GameEvent::SaveRequested
        | GameEvent::ShutdownRequested => {}
    }
    Ok(())
}
'''
text = text[:old_dispatch_start] + new_dispatch + text[old_dispatch_end:]

# Add a focused test without changing the legacy default-config expectations.
test_anchor = """    #[test]
    fn synchronizes_full_inventory_and_give_slot_changes() {
"""
entity_test = r'''    #[test]
    fn replicates_player_info_entity_lifecycle_and_movement_when_enabled() {
        let game = SharedGameRuntime::vanilla_overworld();
        let config = GameReplicationConfig {
            entity_protocol_ids: EntityProtocolRegistry::new([("minecraft:player", 1)]).unwrap(),
            ..GameReplicationConfig::default()
        };
        let service = spawn_game_replication(&game, config).unwrap();
        let steve = PlayerUuid::new(10);
        let alex = PlayerUuid::new(11);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(128).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(10),
            NonZeroUsize::new(64).unwrap(),
        )
        .unwrap();
        let (alex_reader, _alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(11),
            NonZeroUsize::new(64).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(128).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().register(alex, alex_reader).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();

        let mut kinds = Vec::new();
        for _ in 0..3 {
            let output = recv_output(&steve_writer, &mut workers, &mut inputs);
            if let PlayOutput::ProtocolPacket { kind, .. } = output {
                kinds.push(kind);
            }
        }
        assert!(kinds.contains(&PacketKind::PlayerInfoUpdate));
        assert!(kinds.contains(&PacketKind::AddEntity));

        game.move_player(
            alex,
            Transform::new([1.5, 65.0, 0.5], 30.0, 0.0, true).unwrap(),
        )
        .unwrap();
        let mut movement_kinds = Vec::new();
        for _ in 0..2 {
            if let PlayOutput::ProtocolPacket { kind, .. } =
                recv_output(&steve_writer, &mut workers, &mut inputs)
            {
                movement_kinds.push(kind);
            }
        }
        assert!(movement_kinds.contains(&PacketKind::MoveEntityPositionRotation));
        assert!(movement_kinds.contains(&PacketKind::RotateHead));

        service.control().unregister(alex).unwrap();
        game.disconnect_player(alex).unwrap();
        let mut leave_kinds = Vec::new();
        for _ in 0..3 {
            if let PlayOutput::ProtocolPacket { kind, .. } =
                recv_output(&steve_writer, &mut workers, &mut inputs)
            {
                leave_kinds.push(kind);
            }
        }
        assert!(leave_kinds.contains(&PacketKind::RemoveEntities));
        assert!(leave_kinds.contains(&PacketKind::PlayerInfoRemove));
        service.shutdown().unwrap();
    }

'''
text = replace_once(text, test_anchor, entity_test + test_anchor, "entity replication test")
path.write_text(text)


# Resolve semantic protocol packets and build the entity type palette from the
# generated version pack / built-in manifest.
path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text()
text = replace_once(
    text,
    """    BlockPosition, CommonPlayerSpawnInfo, DataComponentProtocolRegistry, DefaultSpawnPosition,
    GlobalPosition, ItemProtocolRegistry, JoinGame, PlayerPosition, PositionMoveRotation,
""",
    """    BlockPosition, CommonPlayerSpawnInfo, DataComponentProtocolRegistry, DefaultSpawnPosition,
    EntityProtocolRegistry, GlobalPosition, ItemProtocolRegistry, JoinGame, PlayerPosition,
    PositionMoveRotation,
""",
    "main entity registry import",
)
text = replace_once(
    text,
    """    item_protocol_ids: ItemProtocolRegistry,
    data_component_protocol_ids: DataComponentProtocolRegistry,
""",
    """    entity_protocol_ids: EntityProtocolRegistry,
    item_protocol_ids: ItemProtocolRegistry,
    data_component_protocol_ids: DataComponentProtocolRegistry,
""",
    "server config entity registry",
)
text = replace_once(
    text,
    """        registry_payloads,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    """        registry_payloads,
        entity_protocol_ids,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    "runtime load tuple declaration",
)
text = replace_once(
    text,
    """            loaded.registry_payloads,
            loaded.item_protocol_ids,
            loaded.data_component_protocol_ids,
""",
    """            loaded.registry_payloads,
            loaded.entity_protocol_ids,
            loaded.item_protocol_ids,
            loaded.data_component_protocol_ids,
""",
    "loaded tuple values",
)
text = replace_once(
    text,
    """        (
            config
                .protocol_profile()
                .context("cannot build configured protocol profile")?,
            play_runtime::builtin_world_profile(),
            registry_payloads,
            ItemProtocolRegistry::default(),
            DataComponentProtocolRegistry::default(),
        )
""",
    """        let entity_protocol_ids =
            if config.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME) {
                builtin_entity_protocol_ids()?
            } else {
                EntityProtocolRegistry::default()
            };
        (
            config
                .protocol_profile()
                .context("cannot build configured protocol profile")?,
            play_runtime::builtin_world_profile(),
            registry_payloads,
            entity_protocol_ids,
            ItemProtocolRegistry::default(),
            DataComponentProtocolRegistry::default(),
        )
""",
    "builtin entity tuple",
)
text = replace_once(
    text,
    """    config.runtime_profile = Some(runtime_profile);
    config.item_protocol_ids = item_protocol_ids;
""",
    """    config.runtime_profile = Some(runtime_profile);
    config.entity_protocol_ids = entity_protocol_ids;
    config.item_protocol_ids = item_protocol_ids;
""",
    "assign entity registry",
)
text = replace_once(
    text,
    """        registry_payloads,
        config.play_policy.clone(),
""",
    """        registry_payloads,
        config.entity_protocol_ids.clone(),
        config.play_policy.clone(),
""",
    "production state entity registry",
)
text = replace_once(
    text,
    """            registry_payloads,
            config.play_policy.clone(),
""",
    """            registry_payloads,
            config.entity_protocol_ids.clone(),
            config.play_policy.clone(),
""",
    "test state entity registry",
)
text = replace_once(
    text,
    """        registry_payloads: Vec<Vec<u8>>,
        play_policy: PlayPolicy,
""",
    """        registry_payloads: Vec<Vec<u8>>,
        entity_protocol_ids: EntityProtocolRegistry,
        play_policy: PlayPolicy,
""",
    "state runtime signature",
)
text = replace_once(
    text,
    """        let game_replication =
            spawn_game_replication(&game_runtime, GameReplicationConfig::default())?;
""",
    """        let game_replication = spawn_game_replication(
            &game_runtime,
            GameReplicationConfig {
                entity_protocol_ids,
                ..GameReplicationConfig::default()
            },
        )?;
""",
    "spawn entity replication",
)
text = replace_once(
    text,
    """            registry_payloads,
            play_policy,
            Some(store),
""",
    """            registry_payloads,
            EntityProtocolRegistry::default(),
            play_policy,
            Some(store),
""",
    "loaded world test registry",
)
text = replace_once(
    text,
    """            data_component_protocol_ids: DataComponentProtocolRegistry::default(),
""",
    """            entity_protocol_ids: EntityProtocolRegistry::default(),
            data_component_protocol_ids: DataComponentProtocolRegistry::default(),
""",
    "config default entity registry",
)
text = replace_once(
    text,
    """    registry_payloads: Vec<Vec<u8>>,
    item_protocol_ids: ItemProtocolRegistry,
""",
    """    registry_payloads: Vec<Vec<u8>>,
    entity_protocol_ids: EntityProtocolRegistry,
    item_protocol_ids: ItemProtocolRegistry,
""",
    "loaded pack entity field",
)
text = replace_once(
    text,
    """    let item_protocol_ids = ItemProtocolRegistry::new(
""",
    """    let entity_protocol_ids = entity_protocol_registry_from_pack(&pack.registries)?;
    let item_protocol_ids = ItemProtocolRegistry::new(
""",
    "load entity registry",
)
text = replace_once(
    text,
    """        registry_payloads,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    """        registry_payloads,
        entity_protocol_ids,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    "return entity registry",
)
helper_anchor = """fn builtin_26_1_2_registry_payloads() -> Result<Vec<Vec<u8>>> {
    encode_registry_payloads(version_26_1_2::configuration_registries())
}

"""
helper = r'''fn entity_protocol_registry_from_names<I, S>(names: I) -> Result<EntityProtocolRegistry>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut entries = Vec::new();
    for (protocol_id, name) in names.into_iter().enumerate() {
        entries.push((
            name.into(),
            i32::try_from(protocol_id).context("entity protocol ID exceeds i32")?,
        ));
    }
    EntityProtocolRegistry::new(entries).context("cannot build entity protocol registry")
}

fn entity_protocol_registry_from_pack(
    registries: &[RomPackRegistry],
) -> Result<EntityProtocolRegistry> {
    let registry = registries
        .iter()
        .find(|registry| registry.id == "minecraft:entity_type")
        .context("version pack is missing minecraft:entity_type registry")?;
    entity_protocol_registry_from_names(registry.entries.iter().cloned())
}

fn builtin_entity_protocol_ids() -> Result<EntityProtocolRegistry> {
    let registry = version_26_1_2::SYNCHRONIZED_REGISTRIES
        .iter()
        .find(|registry| registry.id == "minecraft:entity_type")
        .context("built-in profile is missing minecraft:entity_type registry")?;
    entity_protocol_registry_from_names(registry.entries.iter().copied())
}

'''
text = replace_once(text, helper_anchor, helper_anchor + helper, "entity registry helpers")
text = replace_once(
    text,
    """    let data_component_protocol_ids = data_component_protocol_ids.clone();
    spawn_play_writer(
""",
    """    let data_component_protocol_ids = data_component_protocol_ids.clone();
    let protocol_profile = profile.clone();
    spawn_play_writer(
""",
    "writer protocol profile",
)
text = replace_once(
    text,
    """        move |writer, output| match output {
            PlayOutput::SetPlayerInventory { slot, stack } => {
""",
    """        move |writer, output| match output {
            PlayOutput::ProtocolPacket { kind, payload } => {
                if let Some(packet_id) = protocol_profile.packets().id(kind) {
                    write_packet(
                        writer,
                        &build_packet(packet_id, |body| {
                            body.extend_from_slice(&payload);
                            Ok(())
                        })?,
                    )?;
                    writer.flush()?;
                }
                Ok(PlayWriterDirective::Continue)
            }
            PlayOutput::SetPlayerInventory { slot, stack } => {
""",
    "semantic packet writer",
)
text = replace_once(
    text,
    """        PlayOutput::SetPlayerInventory { .. }
        | PlayOutput::SetContainerContent { .. }
""",
    """        PlayOutput::ProtocolPacket { .. }
        | PlayOutput::SetPlayerInventory { .. }
        | PlayOutput::SetContainerContent { .. }
""",
    "semantic writer fallback",
)
path.write_text(text)


# Keep writer unit-test matches exhaustive.
path = Path("crates/ferrum-server/src/play_writer.rs")
text = path.read_text()
text = text.replace(
    "PlayOutput::Packet(bytes) => writer.write_all(&bytes)?,",
    "PlayOutput::Packet(bytes) => writer.write_all(&bytes)?,\n                    PlayOutput::ProtocolPacket { .. } => {}",
)
text = text.replace(
    "PlayOutput::Packet(_)\n                | PlayOutput::KeepAliveRequest(_)",
    "PlayOutput::Packet(_)\n                | PlayOutput::ProtocolPacket { .. }\n                | PlayOutput::KeepAliveRequest(_)",
)
path.write_text(text)


# Document the newly connected live path.
path = Path("README.md")
text = path.read_text()
text = replace_once(
    text,
    "- Offline-mode player chat accepted from the generated `chat` packet and replicated through authoritative gameplay events",
    "- Offline-mode player chat accepted from the generated `chat` packet and replicated through authoritative gameplay events\n- Player tab-list, spawn, movement/rotation, teleport, and removal replication through bounded per-connection outputs",
    "README entity replication",
)
path.write_text(text)
