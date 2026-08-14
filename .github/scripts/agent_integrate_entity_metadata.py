from pathlib import Path

# Expose version-specific metadata layout.
path = Path("crates/ferrum-version-26-1-2/src/lib.rs")
text = path.read_text()
marker = "mod registries;\nmod tags;\n"
replacement = "pub mod entity_metadata;\nmod registries;\nmod tags;\n"
if marker in text:
    text = text.replace(marker, replacement, 1)
elif "pub mod entity_metadata;" not in text:
    raise SystemExit("version metadata module marker not found")
path.write_text(text)

# Export the generic metadata VarInt value helper.
path = Path("crates/ferrum-play/src/lib.rs")
text = path.read_text()
old = "    MAX_ENTITY_DATA_VALUE_BYTES, encode_entity_data,\n};"
new = "    MAX_ENTITY_DATA_VALUE_BYTES, encode_entity_data, encode_entity_data_varint_value,\n};"
if old in text:
    text = text.replace(old, new, 1)
elif "encode_entity_data_varint_value" not in text:
    raise SystemExit("entity data export marker not found")
path.write_text(text)

# Integrate typed item/XP metadata into authoritative replication snapshots.
path = Path("crates/ferrum-server/src/game_replication.rs")
text = path.read_text()

old_game = '''use ferrum_game::{
    EntityId, EquipmentSlot, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerState,
    PlayerUuid, Transform, Velocity, Vitals,
};'''
new_game = '''use ferrum_game::{
    EntityId, EquipmentSlot, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerState,
    PlayerUuid, Transform, Velocity, Vitals, experience_orb_data, item_entity_data,
};'''
if old_game in text:
    text = text.replace(old_game, new_game, 1)
elif "experience_orb_data" not in text or "item_entity_data" not in text:
    raise SystemExit("ferrum_game replication import marker not found")

old_play = '''    CommonPlayerSpawnInfo, DataComponentProtocolRegistry, EncodedEntityMovement,
    EntityMovementKind, EntityProtocolRegistry, EquipmentEntry, ItemProtocolRegistry,
    PlayerInfoEntry, Respawn, RespawnDataToKeep, encode_add_entity, encode_add_world_entity,
    encode_empty_entity_data, encode_entity_movement, encode_hurt_animation,
    encode_player_combat_kill, encode_player_info_remove, encode_player_info_update,
    encode_remove_entities, encode_respawn, encode_rotate_head, encode_set_equipment,
    encode_set_health, encode_teleport_entity,
};'''
new_play = '''    CommonPlayerSpawnInfo, DataComponentProtocolRegistry, EncodedEntityMovement,
    EntityDataEntry, EntityMovementKind, EntityProtocolRegistry, EquipmentEntry, ItemProtocolRegistry,
    PlayerInfoEntry, Respawn, RespawnDataToKeep, encode_add_entity, encode_add_world_entity,
    encode_empty_entity_data, encode_entity_data, encode_entity_data_varint_value,
    encode_entity_movement, encode_hurt_animation, encode_item_stack, encode_player_combat_kill,
    encode_player_info_remove, encode_player_info_update, encode_remove_entities, encode_respawn,
    encode_rotate_head, encode_set_equipment, encode_set_health, encode_teleport_entity,
};'''
if old_play in text:
    text = text.replace(old_play, new_play, 1)
elif "EntityDataEntry" not in text:
    raise SystemExit("ferrum_play replication import marker not found")

version_import = '''use ferrum_version_26_1_2::entity_metadata::{
    EXPERIENCE_ORB_VALUE_INDEX, INT_SERIALIZER_ID, ITEM_ENTITY_STACK_INDEX,
    ITEM_STACK_SERIALIZER_ID,
};
'''
rompack_import = "use ferrum_rompack::RomPackWorld;\n"
if version_import not in text:
    if rompack_import not in text:
        raise SystemExit("rompack import marker not found")
    text = text.replace(rompack_import, rompack_import + version_import, 1)

old_snapshot = '''struct WorldEntitySnapshot {
    entity_id: EntityId,
    uuid: ferrum_game::EntityUuid,
    entity_type: String,
    transform: Transform,
    velocity: Velocity,
}'''
new_snapshot = '''struct WorldEntitySnapshot {
    entity_id: EntityId,
    uuid: ferrum_game::EntityUuid,
    entity_type: String,
    transform: Transform,
    velocity: Velocity,
    entity_data_payload: Vec<u8>,
}'''
if old_snapshot in text:
    text = text.replace(old_snapshot, new_snapshot, 1)
elif "entity_data_payload" not in text:
    raise SystemExit("world snapshot marker not found")

old_call = "    let current = authoritative_world_entities(runtime, &config.entity_protocol_ids)?;"
new_call = "    let current = authoritative_world_entities(runtime, config)?;"
if old_call in text:
    text = text.replace(old_call, new_call, 1)
elif new_call not in text:
    raise SystemExit("world entity snapshot call marker not found")

start = text.index("fn authoritative_world_entities(")
end = text.index("fn sync_world_entities(", start)
new_authoritative = '''fn authoritative_world_entities(
    runtime: &SharedGameRuntime,
    config: &GameReplicationConfig,
) -> Result<BTreeMap<EntityId, WorldEntitySnapshot>> {
    let snapshots = runtime
        .with_state(|state| -> Result<BTreeMap<EntityId, WorldEntitySnapshot>> {
            let mut snapshots = BTreeMap::new();
            for (entity_id, entity) in state.entities().iter() {
                if entity.is_player()
                    || config
                        .entity_protocol_ids
                        .protocol_id(entity.entity_type.as_str())
                        .is_none()
                {
                    continue;
                }
                let entity_data_payload =
                    encode_world_entity_data(state, *entity_id, entity.entity_type.as_str(), config)?;
                snapshots.insert(
                    *entity_id,
                    WorldEntitySnapshot {
                        entity_id: *entity_id,
                        uuid: entity.uuid,
                        entity_type: entity.entity_type.as_str().to_owned(),
                        transform: entity.transform,
                        velocity: entity.velocity,
                        entity_data_payload,
                    },
                );
            }
            Ok(snapshots)
        })
        .context("cannot read authoritative world entities")??;
    Ok(snapshots)
}

fn encode_world_entity_data(
    state: &GameState,
    entity_id: EntityId,
    entity_type: &str,
    config: &GameReplicationConfig,
) -> Result<Vec<u8>> {
    match entity_type {
        "minecraft:item" => {
            let data = item_entity_data(state.entities(), entity_id)
                .context("cannot read item entity data")?
                .context("item entity has no typed item data")?;
            let stack = encode_item_stack(
                Some(&data.stack),
                &config.item_protocol_ids,
                &config.data_component_protocol_ids,
            )
            .context("cannot encode item entity stack")?
            .context("item entity stack uses unavailable protocol metadata")?;
            encode_entity_data(
                entity_id,
                &[EntityDataEntry::new(
                    ITEM_ENTITY_STACK_INDEX,
                    ITEM_STACK_SERIALIZER_ID,
                    &stack,
                )],
            )
            .context("cannot encode item entity metadata")
        }
        "minecraft:experience_orb" => {
            let data = experience_orb_data(state.entities(), entity_id)
                .context("cannot read experience orb data")?
                .context("experience orb has no typed value data")?;
            let value = i32::try_from(data.value)
                .context("experience orb value exceeds metadata VarInt range")?;
            let value = encode_entity_data_varint_value(value);
            encode_entity_data(
                entity_id,
                &[EntityDataEntry::new(
                    EXPERIENCE_ORB_VALUE_INDEX,
                    INT_SERIALIZER_ID,
                    &value,
                )],
            )
            .context("cannot encode experience orb metadata")
        }
        _ => encode_empty_entity_data(entity_id).context("cannot encode empty world entity data"),
    }
}

'''
text = text[:start] + new_authoritative + text[end:]

old_spawn_data = '''                        connection.queue(
                            PlayOutput::ProtocolPacket {
                                kind: PacketKind::SetEntityData,
                                payload: encode_empty_entity_data(snapshot.entity_id)
                                    .context("cannot encode initial world entity data")?,
                            },
                            exit,
                        );'''
new_spawn_data = '''                        connection.queue(
                            PlayOutput::ProtocolPacket {
                                kind: PacketKind::SetEntityData,
                                payload: snapshot.entity_data_payload.clone(),
                            },
                            exit,
                        );'''
if old_spawn_data in text:
    text = text.replace(old_spawn_data, new_spawn_data, 1)
elif "payload: snapshot.entity_data_payload.clone()" not in text:
    raise SystemExit("initial world metadata marker not found")

old_previous = '''                Some(previous) => {
                    if let Some(movement) = encode_entity_movement(
                        snapshot.entity_id,
                        previous.transform,
                        snapshot.transform,
                    )
                    .context("cannot encode world entity movement")?
                    {
                        queue_encoded_movement(connection, movement, exit);
                    }
                }'''
new_previous = '''                Some(previous) => {
                    if let Some(movement) = encode_entity_movement(
                        snapshot.entity_id,
                        previous.transform,
                        snapshot.transform,
                    )
                    .context("cannot encode world entity movement")?
                    {
                        queue_encoded_movement(connection, movement, exit);
                    }
                    if previous.entity_data_payload != snapshot.entity_data_payload {
                        connection.queue(
                            PlayOutput::ProtocolPacket {
                                kind: PacketKind::SetEntityData,
                                payload: snapshot.entity_data_payload.clone(),
                            },
                            exit,
                        );
                    }
                }'''
if old_previous in text:
    text = text.replace(old_previous, new_previous, 1)
elif "previous.entity_data_payload != snapshot.entity_data_payload" not in text:
    raise SystemExit("world metadata diff marker not found")

path.write_text(text)
