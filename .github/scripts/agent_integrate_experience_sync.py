from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise RuntimeError(f"marker not found in {path}: {old[:120]!r}")
    if text.count(old) != 1:
        raise RuntimeError(f"marker not unique in {path}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


# Domain event: experience mutations must be observable by replication.
state = ROOT / "crates/rom-game/src/state.rs"
replace_once(
    state,
    "    EntityId, EntityStore, EntityType, EntityUuid, GameMode, InventoryError, ItemStack,\n    PlayerError, PlayerState, PlayerUuid, Transform, Vitals,",
    "    EntityId, EntityStore, EntityType, EntityUuid, Experience, GameMode, InventoryError,\n    ItemStack, PlayerError, PlayerState, PlayerUuid, Transform, Vitals,",
)
replace_once(
    state,
    "    PlayerVitalsChanged {\n        uuid: PlayerUuid,\n        vitals: Vitals,\n    },",
    "    PlayerVitalsChanged {\n        uuid: PlayerUuid,\n        vitals: Vitals,\n    },\n    PlayerExperienceChanged {\n        uuid: PlayerUuid,\n        experience: Experience,\n    },",
)

gameplay_tick = ROOT / "crates/rom-game/src/gameplay_tick.rs"
replace_once(
    gameplay_tick,
    "            player.experience.total = total;\n            player.experience.level = level;\n            player.experience.progress = progress;\n            outcome.stats.experience_pickups = outcome.stats.experience_pickups.saturating_add(1);",
    "            player.experience.total = total;\n            player.experience.level = level;\n            player.experience.progress = progress;\n            outcome.events.push(GameEvent::PlayerExperienceChanged {\n                uuid,\n                experience: player.experience,\n            });\n            outcome.stats.experience_pickups = outcome.stats.experience_pickups.saturating_add(1);",
)

# Version-neutral payload encoder is exposed by rom-play.
play_lib = ROOT / "crates/rom-play/src/lib.rs"
replace_once(play_lib, "mod entity_data;\nmod generic_entity;", "mod entity_data;\nmod experience;\nmod generic_entity;")
replace_once(
    play_lib,
    "pub use entity_data::{\n    ENTITY_DATA_TERMINATOR, EntityDataEncodeError, EntityDataEntry, MAX_ENTITY_DATA_ENTRIES,\n    MAX_ENTITY_DATA_VALUE_BYTES, encode_entity_data, encode_entity_data_varint_value,\n};\npub use generic_entity::{GenericEntityEncodeError, encode_add_world_entity};",
    "pub use entity_data::{\n    ENTITY_DATA_TERMINATOR, EntityDataEncodeError, EntityDataEntry, MAX_ENTITY_DATA_ENTRIES,\n    MAX_ENTITY_DATA_VALUE_BYTES, encode_entity_data, encode_entity_data_varint_value,\n};\npub use experience::{ExperienceEncodeError, encode_set_experience};\npub use generic_entity::{GenericEntityEncodeError, encode_add_world_entity};",
)

# Typed protocol kind and catalog mapping for minecraft:set_experience.
protocol = ROOT / "crates/rom-protocol/src/lib.rs"
replace_once(protocol, "    SetEquipment,\n    SetHealth,", "    SetEquipment,\n    SetExperience,\n    SetHealth,")
replace_once(protocol, "        Self::SetEquipment,\n        Self::SetHealth,", "        Self::SetEquipment,\n        Self::SetExperience,\n        Self::SetHealth,")
replace_once(protocol, "            | Self::SetEquipment\n            | Self::SetHealth", "            | Self::SetEquipment\n            | Self::SetExperience\n            | Self::SetHealth")

catalog = ROOT / "crates/rom-protocol/src/packet_catalog.rs"
replace_once(
    catalog,
    "        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_equipment\") => {\n            Some(PacketKind::SetEquipment)\n        }\n        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_health\") => {",
    "        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_equipment\") => {\n            Some(PacketKind::SetEquipment)\n        }\n        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_experience\") => {\n            Some(PacketKind::SetExperience)\n        }\n        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_health\") => {",
)
replace_once(
    catalog,
    "        PacketKind::SetEquipment => \"minecraft:set_equipment\",\n        PacketKind::SetHealth => \"minecraft:set_health\",",
    "        PacketKind::SetEquipment => \"minecraft:set_equipment\",\n        PacketKind::SetExperience => \"minecraft:set_experience\",\n        PacketKind::SetHealth => \"minecraft:set_health\",",
)
replace_once(
    catalog,
    "    #[test]\n    fn recognizes_set_health_as_optional_typed_packet() {",
    "    #[test]\n    fn recognizes_set_experience_as_optional_typed_packet() {\n        assert_eq!(\n            known_packet_kind(\n                ProtocolPhase::Play,\n                PacketDirection::Clientbound,\n                \"set_experience\",\n            ),\n            Some(PacketKind::SetExperience)\n        );\n        assert_eq!(\n            canonical_packet_name(PacketKind::SetExperience),\n            \"minecraft:set_experience\"\n        );\n    }\n\n    #[test]\n    fn recognizes_set_health_as_optional_typed_packet() {",
)

# Replication: send initial state, pickup changes, and post-respawn state.
replication = ROOT / "crates/rom-server/src/game_replication.rs"
replace_once(
    replication,
    "    EntityId, EquipmentSlot, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerState,\n    PlayerUuid, Transform, Velocity, Vitals, experience_orb::experience_orb_data,",
    "    EntityId, EquipmentSlot, Experience, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS,\n    PlayerState, PlayerUuid, Transform, Velocity, Vitals, experience_orb::experience_orb_data,",
)
replace_once(
    replication,
    "    encode_entity_movement, encode_hurt_animation, encode_item_stack, encode_player_combat_kill,\n    encode_player_info_remove, encode_player_info_update, encode_remove_entities, encode_respawn,\n    encode_rotate_head, encode_set_equipment, encode_set_health, encode_teleport_entity,",
    "    encode_entity_movement, encode_hurt_animation, encode_item_stack, encode_player_combat_kill,\n    encode_player_info_remove, encode_player_info_update, encode_remove_entities, encode_respawn,\n    encode_rotate_head, encode_set_equipment, encode_set_experience, encode_set_health,\n    encode_teleport_entity,",
)
replace_once(
    replication,
    "                                Some(player) if player.connected => Some((\n                                    player.vitals,\n                                    player_snapshot_from_state(state, uuid)?.with_context(|| {",
    "                                Some(player) if player.connected => Some((\n                                    player.vitals,\n                                    player.experience,\n                                    player_snapshot_from_state(state, uuid)?.with_context(|| {",
)
replace_once(
    replication,
    "                    if let Some((vitals, snapshot)) = self_state {\n                        queue_set_health(connection, vitals, exit)?;\n                        queue_player_info_update(connection, &snapshot, exit)?;",
    "                    if let Some((vitals, experience, snapshot)) = self_state {\n                        queue_set_health(connection, vitals, exit)?;\n                        queue_set_experience(connection, experience, exit)?;\n                        queue_player_info_update(connection, &snapshot, exit)?;",
)
replace_once(
    replication,
    "            let vitals = runtime\n                .with_state(|state| state.player(uuid).map(|player| player.vitals))\n                .context(\"cannot read connected player vitals\")?;",
    "            let player_state = runtime\n                .with_state(|state| {\n                    state\n                        .player(uuid)\n                        .map(|player| (player.vitals, player.experience))\n                })\n                .context(\"cannot read connected player vitals and experience\")?;",
)
replace_once(
    replication,
    "                if let Some(vitals) = vitals {\n                    queue_set_health(connection, vitals, exit)?;\n                }",
    "                if let Some((vitals, experience)) = player_state {\n                    queue_set_health(connection, vitals, exit)?;\n                    queue_set_experience(connection, experience, exit)?;\n                }",
)
replace_once(
    replication,
    "        GameEvent::PlayerVitalsChanged { uuid, vitals } => {\n            if let Some(connection) = connections.get_mut(&uuid) {\n                queue_set_health(connection, vitals, exit)?;\n            }\n        }",
    "        GameEvent::PlayerVitalsChanged { uuid, vitals } => {\n            if let Some(connection) = connections.get_mut(&uuid) {\n                queue_set_health(connection, vitals, exit)?;\n            }\n        }\n        GameEvent::PlayerExperienceChanged { uuid, experience } => {\n            if let Some(connection) = connections.get_mut(&uuid) {\n                queue_set_experience(connection, experience, exit)?;\n            }\n        }",
)
replace_once(
    replication,
    "                connection.queue_teleport(transform, exit);\n            }\n            if entity_replication_enabled(&config.entity_protocol_ids)",
    "                connection.queue_teleport(transform, exit);\n                if let Some(experience) = runtime\n                    .with_state(|state| state.player(uuid).map(|player| player.experience))\n                    .context(\"cannot read respawned player experience\")?\n                {\n                    queue_set_experience(connection, experience, exit)?;\n                }\n            }\n            if entity_replication_enabled(&config.entity_protocol_ids)",
)
replace_once(
    replication,
    "fn queue_set_health(\n    connection: &mut ReplicationConnection,\n    vitals: Vitals,\n    exit: &mut GameReplicationExit,\n) -> Result<()> {",
    "fn queue_set_experience(\n    connection: &mut ReplicationConnection,\n    experience: Experience,\n    exit: &mut GameReplicationExit,\n) -> Result<()> {\n    connection.queue(\n        PlayOutput::ProtocolPacket {\n            kind: PacketKind::SetExperience,\n            payload: encode_set_experience(experience)\n                .context(\"cannot encode player experience\")?,\n        },\n        exit,\n    );\n    Ok(())\n}\n\nfn queue_set_health(\n    connection: &mut ReplicationConnection,\n    vitals: Vitals,\n    exit: &mut GameReplicationExit,\n) -> Result<()> {",
)

print("Integrated authoritative player experience synchronization.")
