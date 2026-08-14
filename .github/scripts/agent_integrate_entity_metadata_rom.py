from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise RuntimeError(f"marker not found in {path}: {old[:100]!r}")
    if text.count(old) != 1:
        raise RuntimeError(f"marker is not unique in {path}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


play_lib = ROOT / "crates/rom-play/src/lib.rs"
replace_once(play_lib, "mod entity;\nmod generic_entity;", "mod entity;\nmod entity_data;\nmod generic_entity;")
replace_once(
    play_lib,
    "pub use entity::{\n    EncodedEntityMovement, EntityEncodeError, EntityMovementKind, EntityProtocolRegistry,\n    PlayerInfoEntry, encode_add_entity, encode_empty_entity_data, encode_entity_movement,\n    encode_player_info_remove, encode_player_info_update, encode_remove_entities,\n    encode_rotate_head, encode_teleport_entity,\n};\n",
    "pub use entity::{\n    EncodedEntityMovement, EntityEncodeError, EntityMovementKind, EntityProtocolRegistry,\n    PlayerInfoEntry, encode_add_entity, encode_empty_entity_data, encode_entity_movement,\n    encode_player_info_remove, encode_player_info_update, encode_remove_entities,\n    encode_rotate_head, encode_teleport_entity,\n};\npub use entity_data::{\n    ENTITY_DATA_TERMINATOR, EntityDataEncodeError, EntityDataEntry, MAX_ENTITY_DATA_ENTRIES,\n    MAX_ENTITY_DATA_VALUE_BYTES, encode_entity_data, encode_entity_data_varint_value,\n};\n",
)

version_lib = ROOT / "crates/rom-version-26-1-2/src/lib.rs"
replace_once(version_lib, "mod registries;\nmod tags;", "pub mod entity_metadata;\nmod registries;\nmod tags;")

replication = ROOT / "crates/rom-server/src/game_replication.rs"
replace_once(
    replication,
    "use rom_game::{\n    EntityId, EquipmentSlot, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerState,\n    PlayerUuid, Transform, Velocity, Vitals,\n};",
    "use rom_game::{\n    EntityId, EquipmentSlot, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerState,\n    PlayerUuid, Transform, Velocity, Vitals, experience_orb::experience_orb_data,\n    item_entity::item_entity_data,\n};",
)
replace_once(
    replication,
    "    PlayerInfoEntry, Respawn, RespawnDataToKeep, encode_add_entity, encode_add_world_entity,\n    encode_empty_entity_data, encode_entity_movement, encode_hurt_animation,\n    encode_player_combat_kill, encode_player_info_remove, encode_player_info_update,\n    encode_remove_entities, encode_respawn, encode_rotate_head, encode_set_equipment,\n    encode_set_health, encode_teleport_entity,\n};",
    "    EntityDataEntry, PlayerInfoEntry, Respawn, RespawnDataToKeep, encode_add_entity,\n    encode_add_world_entity, encode_empty_entity_data, encode_entity_data,\n    encode_entity_data_varint_value, encode_entity_movement, encode_hurt_animation,\n    encode_item_stack, encode_player_combat_kill, encode_player_info_remove,\n    encode_player_info_update, encode_remove_entities, encode_respawn, encode_rotate_head,\n    encode_set_equipment, encode_set_health, encode_teleport_entity,\n};",
)
replace_once(
    replication,
    "use rom_protocol::PacketKind;",
    "use rom_protocol::PacketKind;\nuse rom_version_26_1_2::entity_metadata::{\n    EXPERIENCE_ORB_VALUE_INDEX, INT_SERIALIZER_ID, ITEM_ENTITY_STACK_INDEX,\n    ITEM_STACK_SERIALIZER_ID,\n};",
)
replace_once(
    replication,
    "struct WorldEntitySnapshot {\n    entity_id: EntityId,\n    uuid: rom_game::EntityUuid,\n    entity_type: String,\n    transform: Transform,\n    velocity: Velocity,\n}",
    "struct WorldEntitySnapshot {\n    entity_id: EntityId,\n    uuid: rom_game::EntityUuid,\n    entity_type: String,\n    transform: Transform,\n    velocity: Velocity,\n    entity_data_payload: Vec<u8>,\n}",
)
old = '''fn authoritative_world_entities(\n    runtime: &SharedGameRuntime,\n    registry: &EntityProtocolRegistry,\n) -> Result<BTreeMap<EntityId, WorldEntitySnapshot>> {\n    runtime\n        .with_state(|state| {\n            state\n                .entities()\n                .iter()\n                .filter_map(|(entity_id, entity)| {\n                    if entity.is_player()\n                        || registry.protocol_id(entity.entity_type.as_str()).is_none()\n                    {\n                        return None;\n                    }\n                    Some((\n                        *entity_id,\n                        WorldEntitySnapshot {\n                            entity_id: *entity_id,\n                            uuid: entity.uuid,\n                            entity_type: entity.entity_type.as_str().to_owned(),\n                            transform: entity.transform,\n                            velocity: entity.velocity,\n                        },\n                    ))\n                })\n                .collect()\n        })\n        .context("cannot read authoritative world entities")\n}\n'''
new = '''fn authoritative_world_entities(\n    runtime: &SharedGameRuntime,\n    config: &GameReplicationConfig,\n) -> Result<BTreeMap<EntityId, WorldEntitySnapshot>> {\n    runtime\n        .with_state(|state| -> Result<_> {\n            let mut snapshots = BTreeMap::new();\n            for (entity_id, entity) in state.entities().iter() {\n                if entity.is_player()\n                    || config\n                        .entity_protocol_ids\n                        .protocol_id(entity.entity_type.as_str())\n                        .is_none()\n                {\n                    continue;\n                }\n                let entity_data_payload = encode_world_entity_data(\n                    state,\n                    *entity_id,\n                    entity.entity_type.as_str(),\n                    config,\n                )?;\n                snapshots.insert(\n                    *entity_id,\n                    WorldEntitySnapshot {\n                        entity_id: *entity_id,\n                        uuid: entity.uuid,\n                        entity_type: entity.entity_type.as_str().to_owned(),\n                        transform: entity.transform,\n                        velocity: entity.velocity,\n                        entity_data_payload,\n                    },\n                );\n            }\n            Ok(snapshots)\n        })\n        .context("cannot read authoritative world entities")?\n}\n\nfn encode_world_entity_data(\n    state: &GameState,\n    entity_id: EntityId,\n    entity_type: &str,\n    config: &GameReplicationConfig,\n) -> Result<Vec<u8>> {\n    match entity_type {\n        "minecraft:item" => {\n            let data = item_entity_data(state.entities(), entity_id)\n                .context("cannot read authoritative item entity data")?\n                .context("item entity is missing its authoritative stack data")?;\n            let stack = encode_item_stack(\n                Some(&data.stack),\n                &config.item_protocol_ids,\n                &config.data_component_protocol_ids,\n            )\n            .context("cannot encode item entity stack")?\n            .context("item entity stack references unavailable protocol registry data")?;\n            encode_entity_data(\n                entity_id,\n                &[EntityDataEntry::new(\n                    ITEM_ENTITY_STACK_INDEX,\n                    ITEM_STACK_SERIALIZER_ID,\n                    &stack,\n                )],\n            )\n            .context("cannot encode item entity metadata")\n        }\n        "minecraft:experience_orb" => {\n            let data = experience_orb_data(state.entities(), entity_id)\n                .context("cannot read authoritative experience orb data")?\n                .context("experience orb is missing its authoritative value")?;\n            let value = i32::try_from(data.value)\n                .context("experience orb value exceeds protocol VarInt range")?;\n            let value = encode_entity_data_varint_value(value);\n            encode_entity_data(\n                entity_id,\n                &[EntityDataEntry::new(\n                    EXPERIENCE_ORB_VALUE_INDEX,\n                    INT_SERIALIZER_ID,\n                    &value,\n                )],\n            )\n            .context("cannot encode experience orb metadata")\n        }\n        _ => encode_empty_entity_data(entity_id).context("cannot encode empty world entity data"),\n    }\n}\n'''
replace_once(replication, old, new)
replace_once(
    replication,
    "    let current = authoritative_world_entities(runtime, &config.entity_protocol_ids)?;",
    "    let current = authoritative_world_entities(runtime, config)?;",
)
replace_once(
    replication,
    "                                payload: encode_empty_entity_data(snapshot.entity_id)\n                                    .context(\"cannot encode initial world entity data\")?,",
    "                                payload: snapshot.entity_data_payload.clone(),",
)
replace_once(
    replication,
    "                Some(previous) => {\n                    if let Some(movement) = encode_entity_movement(\n                        snapshot.entity_id,\n                        previous.transform,\n                        snapshot.transform,\n                    )\n                    .context(\"cannot encode world entity movement\")?\n                    {\n                        queue_encoded_movement(connection, movement, exit);\n                    }\n                }",
    "                Some(previous) => {\n                    if let Some(movement) = encode_entity_movement(\n                        snapshot.entity_id,\n                        previous.transform,\n                        snapshot.transform,\n                    )\n                    .context(\"cannot encode world entity movement\")?\n                    {\n                        queue_encoded_movement(connection, movement, exit);\n                    }\n                    if previous.entity_data_payload != snapshot.entity_data_payload {\n                        connection.queue(\n                            PlayOutput::ProtocolPacket {\n                                kind: PacketKind::SetEntityData,\n                                payload: snapshot.entity_data_payload.clone(),\n                            },\n                            exit,\n                        );\n                    }\n                }",
)

print("Integrated authoritative item/XP entity metadata replication.")
