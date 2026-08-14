from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected one marker in {path}, found {count}: {old[:160]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


state = ROOT / "crates/rom-game/src/state.rs"
replace_once(
    state,
    "    PlayerError, PlayerState, PlayerUuid, Transform, Vitals,\n",
    "    PlayerError, PlayerState, PlayerUuid, Transform, Velocity, Vitals,\n",
)
replace_once(
    state,
    "    PlayerTeleported {\n        uuid: PlayerUuid,\n        entity_id: EntityId,\n        transform: Transform,\n    },\n",
    "    PlayerTeleported {\n        uuid: PlayerUuid,\n        entity_id: EntityId,\n        transform: Transform,\n    },\n    PlayerVelocityChanged {\n        uuid: PlayerUuid,\n        entity_id: EntityId,\n        velocity: Velocity,\n    },\n",
)

runtime = ROOT / "crates/rom-server/src/game_runtime.rs"
replace_once(
    runtime,
    "    GameState, GameStateError, GameplayTickError, ItemStack, PersistenceError, PlayerUuid,\n    Transform, execute_command,\n",
    "    GameState, GameStateError, GameplayTickError, ItemStack, KnockbackOutcome, PersistenceError,\n    PlayerUuid, Transform, execute_command,\n",
)
replace_once(
    runtime,
    "    pub fn damage_player(\n        &self,\n        uuid: PlayerUuid,\n        amount: f32,\n    ) -> Result<Vec<GameEvent>, GameRuntimeError> {\n        let events = self.write()?.damage_player(uuid, amount)?;\n        self.finalize_events(&events)?;\n        Ok(events)\n    }\n\n",
    "    pub fn damage_player(\n        &self,\n        uuid: PlayerUuid,\n        amount: f32,\n    ) -> Result<Vec<GameEvent>, GameRuntimeError> {\n        let events = self.write()?.damage_player(uuid, amount)?;\n        self.finalize_events(&events)?;\n        Ok(events)\n    }\n\n    pub fn knockback_player(\n        &self,\n        uuid: PlayerUuid,\n        attacker_position: [f64; 3],\n        horizontal_strength: f64,\n        vertical_strength: f64,\n    ) -> Result<KnockbackOutcome, GameRuntimeError> {\n        let outcome = self.write()?.knockback_player(\n            uuid,\n            attacker_position,\n            horizontal_strength,\n            vertical_strength,\n        )?;\n        self.finalize_events(&[GameEvent::PlayerVelocityChanged {\n            uuid,\n            entity_id: outcome.entity_id,\n            velocity: outcome.current_velocity,\n        }])?;\n        Ok(outcome)\n    }\n\n",
)
replace_once(
    runtime,
    "    #[test]\n    fn death_drops_are_materialized_as_world_entities() {",
    "    #[test]\n    fn knockback_publishes_authoritative_velocity_change() {\n        let runtime = SharedGameRuntime::vanilla_overworld();\n        let subscription = runtime.subscribe(NonZeroUsize::new(4).unwrap()).unwrap();\n        let uuid = PlayerUuid::new(0x51);\n        runtime.connect_player(uuid, \"Steve\", spawn()).unwrap();\n        assert!(matches!(subscription.try_recv().unwrap(), GameEvent::PlayerConnected { .. }));\n\n        let outcome = runtime\n            .knockback_player(uuid, [-2.0, 65.0, 0.5], 0.4, 0.4)\n            .unwrap();\n        assert!(matches!(\n            subscription.try_recv().unwrap(),\n            GameEvent::PlayerVelocityChanged {\n                uuid: event_uuid,\n                entity_id,\n                velocity,\n            } if event_uuid == uuid\n                && entity_id == outcome.entity_id\n                && velocity == outcome.current_velocity\n        ));\n    }\n\n    #[test]\n    fn death_drops_are_materialized_as_world_entities() {",
)

entity = ROOT / "crates/rom-play/src/entity.rs"
replace_once(
    entity,
    "pub fn encode_empty_entity_data(entity_id: EntityId) -> Result<Vec<u8>, EntityEncodeError> {\n    let mut output = Vec::new();\n    write_entity_id(&mut output, entity_id)?;\n    output.push(ENTITY_DATA_TERMINATOR);\n    Ok(output)\n}\n\n",
    "pub fn encode_empty_entity_data(entity_id: EntityId) -> Result<Vec<u8>, EntityEncodeError> {\n    let mut output = Vec::new();\n    write_entity_id(&mut output, entity_id)?;\n    output.push(ENTITY_DATA_TERMINATOR);\n    Ok(output)\n}\n\npub fn encode_set_entity_motion(\n    entity_id: EntityId,\n    velocity: Velocity,\n) -> Result<Vec<u8>, EntityEncodeError> {\n    let mut output = Vec::with_capacity(11);\n    write_entity_id(&mut output, entity_id)?;\n    for component in velocity.0 {\n        output.extend_from_slice(&encode_velocity_component(component)?.to_be_bytes());\n    }\n    Ok(output)\n}\n\n",
)
replace_once(
    entity,
    "    #[test]\n    fn unknown_entity_type_is_skipped_without_guessing() {",
    "    #[test]\n    fn set_entity_motion_uses_vanilla_short_velocity_scale_and_clamp() {\n        let payload = encode_set_entity_motion(\n            entity_id(),\n            Velocity::new([0.4, -0.25, 5.0]).unwrap(),\n        )\n        .unwrap();\n        assert_eq!(payload[0], 7);\n        assert_eq!(&payload[1..3], &3200_i16.to_be_bytes());\n        assert_eq!(&payload[3..5], &(-2000_i16).to_be_bytes());\n        assert_eq!(&payload[5..7], &31200_i16.to_be_bytes());\n    }\n\n    #[test]\n    fn unknown_entity_type_is_skipped_without_guessing() {",
)

play_lib = ROOT / "crates/rom-play/src/lib.rs"
replace_once(
    play_lib,
    "    encode_player_info_remove, encode_player_info_update, encode_remove_entities,\n    encode_rotate_head, encode_teleport_entity,\n",
    "    encode_player_info_remove, encode_player_info_update, encode_remove_entities,\n    encode_rotate_head, encode_set_entity_motion, encode_teleport_entity,\n",
)

protocol = ROOT / "crates/rom-protocol/src/lib.rs"
replace_once(protocol, "    SetEntityData,\n    SetEquipment,", "    SetEntityData,\n    SetEntityMotion,\n    SetEquipment,")
replace_once(protocol, "        Self::SetEntityData,\n        Self::SetEquipment,", "        Self::SetEntityData,\n        Self::SetEntityMotion,\n        Self::SetEquipment,")
replace_once(protocol, "            | Self::SetEntityData\n            | Self::SetEquipment", "            | Self::SetEntityData\n            | Self::SetEntityMotion\n            | Self::SetEquipment")

catalog = ROOT / "crates/rom-protocol/src/packet_catalog.rs"
replace_once(
    catalog,
    "        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_entity_data\") => {\n            Some(PacketKind::SetEntityData)\n        }\n        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_equipment\") => {",
    "        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_entity_data\") => {\n            Some(PacketKind::SetEntityData)\n        }\n        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_entity_motion\") => {\n            Some(PacketKind::SetEntityMotion)\n        }\n        (ProtocolPhase::Play, PacketDirection::Clientbound, \"set_equipment\") => {",
)
replace_once(
    catalog,
    "        PacketKind::SetEntityData => \"minecraft:set_entity_data\",\n        PacketKind::SetEquipment => \"minecraft:set_equipment\",",
    "        PacketKind::SetEntityData => \"minecraft:set_entity_data\",\n        PacketKind::SetEntityMotion => \"minecraft:set_entity_motion\",\n        PacketKind::SetEquipment => \"minecraft:set_equipment\",",
)
replace_once(
    catalog,
    "    #[test]\n    fn recognizes_set_experience_as_optional_typed_packet() {",
    "    #[test]\n    fn recognizes_set_entity_motion_as_optional_typed_packet() {\n        assert_eq!(\n            known_packet_kind(\n                ProtocolPhase::Play,\n                PacketDirection::Clientbound,\n                \"set_entity_motion\",\n            ),\n            Some(PacketKind::SetEntityMotion)\n        );\n        assert_eq!(\n            canonical_packet_name(PacketKind::SetEntityMotion),\n            \"minecraft:set_entity_motion\"\n        );\n    }\n\n    #[test]\n    fn recognizes_set_experience_as_optional_typed_packet() {",
)

replication = ROOT / "crates/rom-server/src/game_replication.rs"
replace_once(
    replication,
    "    encode_rotate_head, encode_set_equipment, encode_set_experience, encode_set_health,\n    encode_teleport_entity,\n",
    "    encode_rotate_head, encode_set_entity_motion, encode_set_equipment, encode_set_experience,\n    encode_set_health, encode_teleport_entity,\n",
)
replace_once(
    replication,
    "        GameEvent::PlayerGameModeChanged { uuid, current, .. } => {",
    "        GameEvent::PlayerVelocityChanged {\n            uuid,\n            entity_id,\n            velocity,\n        } => {\n            let payload = encode_set_entity_motion(entity_id, velocity)\n                .context(\"cannot encode player entity motion\")?;\n            for (target, connection) in connections.iter_mut() {\n                if *target == uuid || connection.entities.contains_key(&uuid) {\n                    connection.queue(\n                        PlayOutput::ProtocolPacket {\n                            kind: PacketKind::SetEntityMotion,\n                            payload: payload.clone(),\n                        },\n                        exit,\n                    );\n                }\n                if let Some(tracked) = connection.entities.get_mut(&uuid)\n                    && tracked.entity_id == entity_id\n                {\n                    tracked.velocity = velocity;\n                }\n            }\n        }\n        GameEvent::PlayerGameModeChanged { uuid, current, .. } => {",
)

print("Integrated authoritative entity velocity synchronization.")
