from pathlib import Path

PATH = Path("crates/ferrum-server/src/game_replication.rs")
text = PATH.read_text()


def replace_once(old: str, new: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one anchor, found {count}: {old[:120]!r}")
    text = text.replace(old, new, 1)


replace_once(
    "use ferrum_game::{GameEvent, PLAYER_INVENTORY_SLOTS, PlayerUuid};\nuse ferrum_play::EntityProtocolRegistry;\n",
    """use ferrum_game::{
    EntityId, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerUuid, Transform,
    Velocity,
};
use ferrum_play::{
    EncodedEntityMovement, EntityMovementKind, EntityProtocolRegistry, PlayerInfoEntry,
    encode_add_entity, encode_empty_entity_data, encode_entity_movement,
    encode_player_info_remove, encode_player_info_update, encode_remove_entities,
    encode_rotate_head, encode_teleport_entity,
};
use ferrum_protocol::PacketKind;
""",
)

replace_once(
    "#[derive(Debug)]\nstruct ReplicationConnection {",
    """#[derive(Debug, Clone)]
struct PlayerEntitySnapshot {
    uuid: PlayerUuid,
    name: String,
    entity_id: EntityId,
    game_mode: GameMode,
    transform: Transform,
    velocity: Velocity,
}

#[derive(Debug)]
struct ReplicationConnection {""",
)

replace_once(
    "    next_teleport_id: i32,\n}",
    "    next_teleport_id: i32,\n    entities: BTreeMap<PlayerUuid, PlayerEntitySnapshot>,\n}",
)
replace_once(
    "            next_teleport_id: 2,\n        }",
    "            next_teleport_id: 2,\n            entities: BTreeMap::new(),\n        }",
)

replace_once(
    """            &mut connections,
            config.pending_output_limit.get(),
            &mut exit,
""",
    """            &mut connections,
            config.pending_output_limit.get(),
            &config.entity_protocol_ids,
            &mut exit,
""",
)

old_dispatch = "dispatch_event(event, &mut connections, &mut exit);"
if text.count(old_dispatch) != 2:
    raise SystemExit(f"expected two dispatch anchors, found {text.count(old_dispatch)}")
text = text.replace(
    old_dispatch,
    "dispatch_event(\n                    event,\n                    &runtime,\n                    &config.entity_protocol_ids,\n                    &mut connections,\n                    &mut exit,\n                )?;",
)

replace_once(
    """    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    pending_limit: usize,
    exit: &mut GameReplicationExit,
""",
    """    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    pending_limit: usize,
    entity_protocol_ids: &EntityProtocolRegistry,
    exit: &mut GameReplicationExit,
""",
)

replace_once(
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
                if connections.contains_key(&uuid) {
                    let _ = reply.send(Err(format!(
                        "player {uuid:?} is already registered for replication"
                    )));
                    continue;
                }

                let mut connection = ReplicationConnection::new(endpoint, pending_limit);
                if entity_replication_enabled(entity_protocol_ids) {
                    let initialization = online_player_snapshots(runtime).and_then(|snapshots| {
                        for snapshot in snapshots {
                            if snapshot.uuid != uuid {
                                queue_player_spawn(
                                    &mut connection,
                                    snapshot,
                                    entity_protocol_ids,
                                    exit,
                                )?;
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
""",
)

start = text.index("fn dispatch_event(")
end = text.index("fn target_chat(", start)
new_dispatch = r'''fn dispatch_event(
    event: GameEvent,
    runtime: &SharedGameRuntime,
    entity_protocol_ids: &EntityProtocolRegistry,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    exit.events = exit.events.saturating_add(1);
    match event {
        GameEvent::PlayerConnected { uuid, name, .. } => {
            if entity_replication_enabled(entity_protocol_ids) {
                let snapshot = player_snapshot(runtime, uuid)?.with_context(|| {
                    format!("connected player {uuid:?} is missing from authoritative state")
                })?;
                if let Some(connection) = connections.get_mut(&uuid) {
                    queue_player_info_update(connection, &snapshot, exit)?;
                }
                for (target, connection) in connections.iter_mut() {
                    if *target != uuid {
                        queue_player_spawn(
                            connection,
                            snapshot.clone(),
                            entity_protocol_ids,
                            exit,
                        )?;
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
            if entity_replication_enabled(entity_protocol_ids) {
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
            if entity_replication_enabled(entity_protocol_ids) {
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
                            queue_player_remove(
                                connection,
                                uuid,
                                Some(tracked.entity_id),
                                exit,
                            )?;
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(
                                    connection,
                                    snapshot,
                                    entity_protocol_ids,
                                    exit,
                                )?;
                            }
                        }
                        None => {
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(
                                    connection,
                                    snapshot,
                                    entity_protocol_ids,
                                    exit,
                                )?;
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
            if entity_replication_enabled(entity_protocol_ids) {
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
                            queue_player_remove(
                                connection,
                                uuid,
                                Some(tracked.entity_id),
                                exit,
                            )?;
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(
                                    connection,
                                    snapshot,
                                    entity_protocol_ids,
                                    exit,
                                )?;
                            }
                        }
                        None => {
                            if let Some(snapshot) = snapshot.clone() {
                                queue_player_spawn(
                                    connection,
                                    snapshot,
                                    entity_protocol_ids,
                                    exit,
                                )?;
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
            if entity_replication_enabled(entity_protocol_ids) {
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
                let snapshot = player_snapshot_from_state(state, player.uuid)?.with_context(|| {
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
    registry: &EntityProtocolRegistry,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !entity_replication_enabled(registry) {
        return Ok(());
    }
    queue_player_info_update(connection, &snapshot, exit)?;
    let payload = encode_add_entity(
        snapshot.entity_id,
        snapshot.uuid,
        "minecraft:player",
        snapshot.transform,
        snapshot.velocity,
        registry,
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

'''
text = text[:start] + new_dispatch + text[end:]

replace_once(
    """    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }
""",
    """    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    fn entity_config() -> GameReplicationConfig {
        GameReplicationConfig {
            entity_protocol_ids: EntityProtocolRegistry::new([("minecraft:player", 148)])
                .unwrap(),
            ..GameReplicationConfig::default()
        }
    }
""",
)

replace_once(
    """    fn recv_output(
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
""",
    """    fn recv_output(
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
""",
)

insert = r'''

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
'''
closing = text.rfind("\n}")
if closing == -1:
    raise SystemExit("cannot find test module closing brace")
text = text[:closing] + insert + text[closing:]

PATH.write_text(text)
