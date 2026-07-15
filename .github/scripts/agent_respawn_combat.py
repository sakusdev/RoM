from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def patch(path: str, transform) -> None:
    file = Path(path)
    text = file.read_text()
    updated = transform(text)
    if updated == text:
        raise SystemExit(f"no changes made to {path}")
    file.write_text(updated)


def patch_protocol_lib(text: str) -> str:
    text = replace_once(
        text,
        """    KeepAliveResponse,
    ClientTickEnd,
    MovePlayerPosition,
""",
        """    KeepAliveResponse,
    ClientTickEnd,
    ClientCommand,
    MovePlayerPosition,
""",
        "client command variant",
    )
    text = replace_once(
        text,
        """    SetEquipment,
    SetHealth,
    PlayerInfoUpdate,
""",
        """    SetEquipment,
    SetHealth,
    HurtAnimation,
    PlayerCombatKill,
    Respawn,
    PlayerInfoUpdate,
""",
        "combat packet variants",
    )
    text = replace_once(
        text,
        """        Self::KeepAliveResponse,
        Self::ClientTickEnd,
        Self::MovePlayerPosition,
""",
        """        Self::KeepAliveResponse,
        Self::ClientTickEnd,
        Self::ClientCommand,
        Self::MovePlayerPosition,
""",
        "client command all",
    )
    text = replace_once(
        text,
        """        Self::SetEquipment,
        Self::SetHealth,
        Self::PlayerInfoUpdate,
""",
        """        Self::SetEquipment,
        Self::SetHealth,
        Self::HurtAnimation,
        Self::PlayerCombatKill,
        Self::Respawn,
        Self::PlayerInfoUpdate,
""",
        "combat all",
    )
    text = replace_once(
        text,
        """            | Self::SetEquipment
            | Self::SetHealth
            | Self::PlayerInfoUpdate
""",
        """            | Self::SetEquipment
            | Self::SetHealth
            | Self::HurtAnimation
            | Self::PlayerCombatKill
            | Self::Respawn
            | Self::PlayerInfoUpdate
""",
        "combat phase",
    )
    text = replace_once(
        text,
        """            | Self::KeepAliveResponse
            | Self::ClientTickEnd
            | Self::MovePlayerPosition
""",
        """            | Self::KeepAliveResponse
            | Self::ClientTickEnd
            | Self::ClientCommand
            | Self::MovePlayerPosition
""",
        "client command phase",
    )
    return text


def patch_packet_catalog(text: str) -> str:
    text = replace_once(
        text,
        """        (ProtocolPhase::Play, PacketDirection::Serverbound, "client_tick_end") => {
            Some(PacketKind::ClientTickEnd)
        }
""",
        """        (ProtocolPhase::Play, PacketDirection::Serverbound, "client_tick_end") => {
            Some(PacketKind::ClientTickEnd)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "client_command") => {
            Some(PacketKind::ClientCommand)
        }
""",
        "client command catalog",
    )
    text = replace_once(
        text,
        """        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_health") => {
            Some(PacketKind::SetHealth)
        }
""",
        """        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_health") => {
            Some(PacketKind::SetHealth)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "hurt_animation") => {
            Some(PacketKind::HurtAnimation)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "player_combat_kill") => {
            Some(PacketKind::PlayerCombatKill)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "respawn") => {
            Some(PacketKind::Respawn)
        }
""",
        "combat packet catalog",
    )
    text = replace_once(
        text,
        """        PacketKind::ClientTickEnd => "minecraft:client_tick_end",
        PacketKind::MovePlayerPosition => "minecraft:move_player_pos",
""",
        """        PacketKind::ClientTickEnd => "minecraft:client_tick_end",
        PacketKind::ClientCommand => "minecraft:client_command",
        PacketKind::MovePlayerPosition => "minecraft:move_player_pos",
""",
        "client command canonical",
    )
    text = replace_once(
        text,
        """        PacketKind::SetEquipment => "minecraft:set_equipment",
        PacketKind::SetHealth => "minecraft:set_health",
        PacketKind::PlayerInfoUpdate => "minecraft:player_info_update",
""",
        """        PacketKind::SetEquipment => "minecraft:set_equipment",
        PacketKind::SetHealth => "minecraft:set_health",
        PacketKind::HurtAnimation => "minecraft:hurt_animation",
        PacketKind::PlayerCombatKill => "minecraft:player_combat_kill",
        PacketKind::Respawn => "minecraft:respawn",
        PacketKind::PlayerInfoUpdate => "minecraft:player_info_update",
""",
        "combat canonical",
    )
    test = """

    #[test]
    fn recognizes_respawn_combat_and_client_command_packets() {
        for (direction, name, kind) in [
            (
                PacketDirection::Serverbound,
                "client_command",
                PacketKind::ClientCommand,
            ),
            (
                PacketDirection::Clientbound,
                "hurt_animation",
                PacketKind::HurtAnimation,
            ),
            (
                PacketDirection::Clientbound,
                "player_combat_kill",
                PacketKind::PlayerCombatKill,
            ),
            (
                PacketDirection::Clientbound,
                "respawn",
                PacketKind::Respawn,
            ),
        ] {
            assert_eq!(
                known_packet_kind(ProtocolPhase::Play, direction, name),
                Some(kind)
            );
            assert_eq!(canonical_packet_name(kind), format!("minecraft:{name}"));
        }
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("packet catalog test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_version(text: str) -> str:
    text = replace_once(
        text,
        """        (PacketKind::SystemChat, 0x79),
        (PacketKind::SetHealth, 0x68),
        (PacketKind::AcceptTeleportation, 0x00),
""",
        """        (PacketKind::SystemChat, 0x79),
        (PacketKind::SetHealth, 0x68),
        (PacketKind::HurtAnimation, 0x2a),
        (PacketKind::PlayerCombatKill, 0x44),
        (PacketKind::Respawn, 0x52),
        (PacketKind::AcceptTeleportation, 0x00),
""",
        "combat IDs",
    )
    text = replace_once(
        text,
        """        (PacketKind::ClientTickEnd, 0x0d),
        (PacketKind::MovePlayerPosition, 0x1e),
""",
        """        (PacketKind::ClientTickEnd, 0x0d),
        (PacketKind::ClientCommand, 0x0c),
        (PacketKind::MovePlayerPosition, 0x1e),
""",
        "client command ID",
    )
    text = replace_once(
        text,
        """        assert_eq!(packets.require(PacketKind::SetHealth).unwrap(), 0x68);
        assert_eq!(packets.require(PacketKind::ChunkBatchStart).unwrap(), 0x0c);
""",
        """        assert_eq!(packets.require(PacketKind::SetHealth).unwrap(), 0x68);
        assert_eq!(packets.require(PacketKind::HurtAnimation).unwrap(), 0x2a);
        assert_eq!(packets.require(PacketKind::PlayerCombatKill).unwrap(), 0x44);
        assert_eq!(packets.require(PacketKind::Respawn).unwrap(), 0x52);
        assert_eq!(packets.require(PacketKind::ClientCommand).unwrap(), 0x0c);
        assert_eq!(packets.require(PacketKind::ChunkBatchStart).unwrap(), 0x0c);
""",
        "combat ID tests",
    )
    return text


def patch_play_lib(text: str) -> str:
    text = replace_once(
        text,
        """use ferrum_nbt::{Tag, encode_anonymous};
use ferrum_world::{BlockStateId, ChunkSection, StaticChunk};
""",
        """use ferrum_game::EntityId;
use ferrum_nbt::{Tag, encode_anonymous};
use ferrum_world::{BlockStateId, ChunkSection, StaticChunk};
""",
        "EntityId import",
    )
    text = replace_once(
        text,
        """pub struct JoinGame {
""",
        """#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RespawnDataToKeep {
    Nothing = 0,
    Attributes = 1,
    EntityData = 2,
    All = 3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Respawn {
    pub spawn_info: CommonPlayerSpawnInfo,
    pub data_to_keep: RespawnDataToKeep,
}

pub struct JoinGame {
""",
        "respawn types",
    )
    text = replace_once(
        text,
        """pub fn encode_system_chat(message: &str, overlay: bool) -> Result<Vec<u8>, PlayEncodeError> {
""",
        """pub fn encode_hurt_animation(
    entity_id: EntityId,
    yaw: f32,
) -> Result<Vec<u8>, PlayEncodeError> {
    if !yaw.is_finite() {
        return Err(PlayEncodeError::NonFinite { field: "hurt yaw" });
    }
    let mut output = Vec::new();
    write_numeric_id(&mut output, "entity", entity_id.get())?;
    output.extend_from_slice(&yaw.to_be_bytes());
    Ok(output)
}

pub fn encode_player_combat_kill(
    entity_id: EntityId,
    message: &str,
) -> Result<Vec<u8>, PlayEncodeError> {
    let mut output = Vec::new();
    write_numeric_id(&mut output, "entity", entity_id.get())?;
    output.extend_from_slice(&encode_component(message)?);
    Ok(output)
}

pub fn encode_respawn(packet: &Respawn) -> Result<Vec<u8>, PlayEncodeError> {
    require_non_negative("dimension_type_id", packet.spawn_info.dimension_type_id)?;
    require_non_negative("portal_cooldown", packet.spawn_info.portal_cooldown)?;
    let mut output = Vec::new();
    encode_common_spawn_info(&mut output, &packet.spawn_info)?;
    output.push(packet.data_to_keep as u8);
    Ok(output)
}

pub fn encode_system_chat(message: &str, overlay: bool) -> Result<Vec<u8>, PlayEncodeError> {
""",
        "combat and respawn codecs",
    )
    test = """

    #[test]
    fn encodes_hurt_combat_kill_and_respawn_packets() {
        let entity_id = EntityId::new(7).unwrap();
        assert_eq!(
            encode_hurt_animation(entity_id, 90.0).unwrap(),
            [vec![7], 90.0_f32.to_be_bytes().to_vec()].concat()
        );

        let mut expected_kill = vec![7];
        expected_kill.extend_from_slice(&encode_component("Steve died").unwrap());
        assert_eq!(
            encode_player_combat_kill(entity_id, "Steve died").unwrap(),
            expected_kill
        );

        let packet = Respawn {
            spawn_info: CommonPlayerSpawnInfo {
                dimension_type_id: 0,
                dimension: "minecraft:overworld".to_owned(),
                seed: 0,
                game_mode: 0,
                previous_game_mode: 0,
                is_debug: false,
                is_flat: true,
                last_death_location: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            data_to_keep: RespawnDataToKeep::Attributes,
        };
        let payload = encode_respawn(&packet).unwrap();
        assert_eq!(payload.last(), Some(&(RespawnDataToKeep::Attributes as u8)));
        assert!(payload.windows("minecraft:overworld".len()).any(|window| {
            window == "minecraft:overworld".as_bytes()
        }));
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("play codec test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_game_state(text: str) -> str:
    text = replace_once(
        text,
        """    PlayerKilled {
        uuid: PlayerUuid,
    },
""",
        """    PlayerKilled {
        uuid: PlayerUuid,
        entity_id: EntityId,
        name: String,
    },
    PlayerRespawned {
        uuid: PlayerUuid,
        entity_id: EntityId,
        transform: Transform,
        game_mode: GameMode,
        previous_game_mode: GameMode,
    },
""",
        "death and respawn events",
    )
    text = text.replace(
        "events.extend(self.finish_player_death(uuid)?);",
        "events.extend(self.finish_player_death(uuid, entity_id)?);",
    )
    if text.count("finish_player_death(uuid, entity_id)") != 2:
        raise SystemExit("death finalization call count mismatch")
    text = replace_once(
        text,
        """    fn finish_player_death(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {
""",
        """    pub fn respawn_player(
        &mut self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let (game_mode, previous_game_mode, vitals) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if !player.vitals.is_dead() {
                return Err(GameStateError::PlayerAlive { uuid });
            }
            let previous_game_mode = player.game_mode;
            player.vitals = Vitals::default();
            (player.game_mode, previous_game_mode, player.vitals)
        };
        let entity = self
            .entities
            .get_mut(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?;
        entity.transform = transform;
        entity.velocity = crate::Velocity::ZERO;
        Ok(vec![
            GameEvent::PlayerRespawned {
                uuid,
                entity_id,
                transform,
                game_mode,
                previous_game_mode,
            },
            GameEvent::PlayerVitalsChanged { uuid, vitals },
        ])
    }

    fn finish_player_death(
        &mut self,
        uuid: PlayerUuid,
        entity_id: EntityId,
    ) -> Result<Vec<GameEvent>, GameStateError> {
""",
        "respawn state method",
    )
    text = replace_once(
        text,
        """        let mut events = vec![GameEvent::PlayerKilled { uuid }];
""",
        """        let mut events = vec![GameEvent::PlayerKilled {
            uuid,
            entity_id,
            name: player.name.clone(),
        }];
""",
        "death event details",
    )
    text = replace_once(
        text,
        """    #[error("player {uuid:?} is dead and must respawn before healing")]
    PlayerDead { uuid: PlayerUuid },
""",
        """    #[error("player {uuid:?} is dead and must respawn before healing")]
    PlayerDead { uuid: PlayerUuid },
    #[error("player {uuid:?} is alive and cannot respawn")]
    PlayerAlive { uuid: PlayerUuid },
""",
        "alive player error",
    )
    text = text.replace(
        "matches!(fatal[2], GameEvent::PlayerKilled { .. })",
        "matches!(fatal[2], GameEvent::PlayerKilled { .. })",
    )
    test = """

    #[test]
    fn respawn_resets_vitals_transform_and_velocity_without_reallocating_entity() {
        let uuid = PlayerUuid::new(32);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let entity_id = state.player(uuid).unwrap().entity_id.unwrap();
        state
            .entities_mut()
            .get_mut(entity_id)
            .unwrap()
            .velocity = crate::Velocity([1.0, 2.0, 3.0]);
        state.kill_player(uuid).unwrap();
        let respawn = Transform::new([8.5, 70.0, -3.5], 45.0, 0.0, false).unwrap();
        let events = state.respawn_player(uuid, respawn).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::PlayerRespawned {
                    entity_id: event_entity_id,
                    transform,
                    ..
                },
                GameEvent::PlayerVitalsChanged { vitals, .. }
            ] if *event_entity_id == entity_id
                && *transform == respawn
                && *vitals == Vitals::default()
        ));
        let entity = state.entities().get(entity_id).unwrap();
        assert_eq!(entity.transform, respawn);
        assert_eq!(entity.velocity, crate::Velocity::ZERO);
        assert_eq!(state.player(uuid).unwrap().vitals, Vitals::default());
        assert!(matches!(
            state.respawn_player(uuid, respawn),
            Err(GameStateError::PlayerAlive { .. })
        ));
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("game state test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_game_runtime(text: str) -> str:
    text = replace_once(
        text,
        """    pub fn kill_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.kill_player(uuid)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn click_container(
""",
        """    pub fn kill_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
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

    pub fn click_container(
""",
        "runtime respawn wrapper",
    )
    return text


def patch_play_runtime(text: str) -> str:
    text = replace_once(
        text,
        """    fn select_hotbar(self, selected_hotbar: u8) -> Result<()> {
""",
        """    fn respawn(self, transform: Transform) -> Result<bool> {
        let dead = self
            .runtime
            .with_state(|state| {
                state
                    .player(self.player_uuid)
                    .is_some_and(|player| player.vitals.is_dead())
            })?;
        if !dead {
            return Ok(false);
        }
        self.runtime.respawn_player(self.player_uuid, transform)?;
        Ok(true)
    }

    fn select_hotbar(self, selected_hotbar: u8) -> Result<()> {
""",
        "gameplay respawn method",
    )
    text = replace_once(
        text,
        """                Some(PacketKind::ChatCommand) => {
""",
        """                Some(PacketKind::ClientCommand) => {
                    match decode_client_command(&mut packet_reader)? {
                        ClientCommandAction::PerformRespawn => {
                            if let Some(gameplay) = gameplay {
                                let position = player_spawn_position(world_profile);
                                let transform = Transform::new(position, 0.0, 0.0, false)?;
                                if gameplay.respawn(transform)? {
                                    player = PlayerState::new(position, 0.0, 0.0, false, false)?;
                                    let respawn_chunk = player.chunk_pos();
                                    if respawn_chunk != view.center() {
                                        let delta = view.recenter(respawn_chunk)?;
                                        shared_world.ensure_chunks_loaded(&delta.newly_visible)?;
                                        send_chunk_view_delta(
                                            writer,
                                            profile,
                                            shared_world,
                                            respawn_chunk,
                                            &delta,
                                            play_reader,
                                        )?;
                                    }
                                }
                            }
                        }
                        ClientCommandAction::RequestStats
                        | ClientCommandAction::RequestGameruleValues => {}
                    }
                }
                Some(PacketKind::ChatCommand) => {
""",
        "client command dispatch",
    )
    text = replace_once(
        text,
        """fn decode_chat_command(reader: &mut PacketReader<'_>) -> Result<String> {
""",
        """#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientCommandAction {
    PerformRespawn,
    RequestStats,
    RequestGameruleValues,
}

fn decode_client_command(reader: &mut PacketReader<'_>) -> Result<ClientCommandAction> {
    let action = match reader.read_varint()? {
        0 => ClientCommandAction::PerformRespawn,
        1 => ClientCommandAction::RequestStats,
        2 => ClientCommandAction::RequestGameruleValues,
        value => bail!("unknown client command action {value}"),
    };
    if !reader.take_remaining().is_empty() {
        bail!("client command packet contains trailing bytes");
    }
    Ok(action)
}

fn decode_chat_command(reader: &mut PacketReader<'_>) -> Result<String> {
""",
        "client command decoder",
    )
    test = """

    #[test]
    fn decodes_client_command_actions_and_rejects_invalid_payloads() {
        for (payload, expected) in [
            (&[0_u8][..], ClientCommandAction::PerformRespawn),
            (&[1_u8][..], ClientCommandAction::RequestStats),
            (&[2_u8][..], ClientCommandAction::RequestGameruleValues),
        ] {
            let mut reader = PacketReader::new(payload);
            assert_eq!(decode_client_command(&mut reader).unwrap(), expected);
        }
        let mut unknown = PacketReader::new(&[3]);
        assert!(decode_client_command(&mut unknown).is_err());
        let mut trailing = PacketReader::new(&[0, 0]);
        assert!(decode_client_command(&mut trailing).is_err());
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("play runtime test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_replication(text: str) -> str:
    text = replace_once(
        text,
        """    encode_player_info_update, encode_remove_entities, encode_rotate_head, encode_set_equipment,
    encode_set_health, encode_teleport_entity,
""",
        """    CommonPlayerSpawnInfo, Respawn, RespawnDataToKeep, encode_hurt_animation,
    encode_player_combat_kill, encode_player_info_update, encode_remove_entities,
    encode_respawn, encode_rotate_head, encode_set_equipment, encode_set_health,
    encode_teleport_entity,
""",
        "replication codec imports",
    )
    text = replace_once(
        text,
        """use ferrum_protocol::PacketKind;
""",
        """use ferrum_protocol::PacketKind;
use ferrum_rompack::RomPackWorld;
""",
        "world profile import",
    )
    text = replace_once(
        text,
        """    pub data_component_protocol_ids: DataComponentProtocolRegistry,
}
""",
        """    pub data_component_protocol_ids: DataComponentProtocolRegistry,
    pub world: Option<RomPackWorld>,
}
""",
        "replication world config",
    )
    text = replace_once(
        text,
        """            data_component_protocol_ids: DataComponentProtocolRegistry::default(),
        }
""",
        """            data_component_protocol_ids: DataComponentProtocolRegistry::default(),
            world: None,
        }
""",
        "replication world default",
    )
    text = replace_once(
        text,
        """        GameEvent::PlayerDamaged { .. } => {}
""",
        """        GameEvent::PlayerDamaged {
            uuid, entity_id, ..
        } => {
            let payload = encode_hurt_animation(entity_id, 0.0)
                .context("cannot encode player hurt animation")?;
            for (target, connection) in connections.iter_mut() {
                if *target == uuid || connection.entities.contains_key(&uuid) {
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
""",
        "hurt animation dispatch",
    )
    text = replace_once(
        text,
        """        GameEvent::PlayerKilled { uuid } => {
            target_chat(connections, uuid, "You died".to_owned(), false, exit)
        }
""",
        """        GameEvent::PlayerKilled {
            uuid,
            entity_id,
            name,
        } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(
                    PlayOutput::ProtocolPacket {
                        kind: PacketKind::PlayerCombatKill,
                        payload: encode_player_combat_kill(
                            entity_id,
                            &format!("{name} died"),
                        )
                        .context("cannot encode player combat death")?,
                    },
                    exit,
                );
            }
        }
        GameEvent::PlayerRespawned {
            uuid,
            entity_id,
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
                    previous_game_mode: protocol_game_mode(previous_game_mode),
                    is_debug: false,
                    is_flat: true,
                    last_death_location: None,
                    portal_cooldown: 0,
                    sea_level: world.sea_level,
                },
                data_to_keep: RespawnDataToKeep::Attributes,
            };
            if let Some(connection) = connections.get_mut(&uuid) {
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
            if entity_replication_enabled(&config.entity_protocol_ids) {
                for (target, connection) in connections.iter_mut() {
                    if *target == uuid {
                        continue;
                    }
                    if let Some(mut tracked) = connection.entities.get(&uuid).cloned() {
                        tracked.entity_id = entity_id;
                        tracked.velocity = Velocity::ZERO;
                        queue_player_absolute_teleport(connection, &tracked, transform, exit)?;
                        if let Some(snapshot) = connection.entities.get_mut(&uuid) {
                            snapshot.entity_id = entity_id;
                            snapshot.transform = transform;
                            snapshot.velocity = Velocity::ZERO;
                        }
                    }
                }
            }
        }
""",
        "death and respawn dispatch",
    )
    text = replace_once(
        text,
        """fn entity_replication_enabled(registry: &EntityProtocolRegistry) -> bool {
""",
        """const fn protocol_game_mode(game_mode: GameMode) -> i8 {
    match game_mode {
        GameMode::Survival => 0,
        GameMode::Creative => 1,
        GameMode::Adventure => 2,
        GameMode::Spectator => 3,
    }
}

fn entity_replication_enabled(registry: &EntityProtocolRegistry) -> bool {
""",
        "protocol game mode helper",
    )
    text = replace_once(
        text,
        """            item_protocol_ids: ItemProtocolRegistry::new([("minecraft:stone", 1)]).unwrap(),
            ..GameReplicationConfig::default()
""",
        """            item_protocol_ids: ItemProtocolRegistry::new([("minecraft:stone", 1)]).unwrap(),
            world: Some(crate::play_runtime::builtin_world_profile()),
            ..GameReplicationConfig::default()
""",
        "entity test config world",
    )
    test = """

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
        game.connect_player(steve, "Steve", spawn()).unwrap();
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetHealth,
        );
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
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetHealth,
        );
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerCombatKill,
        );

        let respawn = Transform::new([0.5, 64.0, 0.5], 0.0, 0.0, false).unwrap();
        game.respawn_player(steve, respawn).unwrap();
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::Respawn,
        );
        assert!(matches!(
            recv_output(&writer, &mut workers, &mut inputs),
            PlayOutput::PlayerTeleport { transform, .. } if transform == respawn
        ));
        recv_protocol(
            &writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetHealth,
        );
        service.shutdown().unwrap();
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("replication test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_main(text: str) -> str:
    text = replace_once(
        text,
        """        let center = play_runtime::spawn_chunk(&world);
        let game_state = match game_state {
""",
        """        let center = play_runtime::spawn_chunk(&world);
        let replication_world = world.clone();
        let game_state = match game_state {
""",
        "replication world clone",
    )
    text = replace_once(
        text,
        """                data_component_protocol_ids,
                ..GameReplicationConfig::default()
""",
        """                data_component_protocol_ids,
                world: Some(replication_world),
                ..GameReplicationConfig::default()
""",
        "replication world config",
    )
    return text


def patch_roadmap(text: str) -> str:
    return replace_once(
        text,
        """Dedicated Play reader/writer queues, a shared 20 TPS runtime, authoritative gameplay persistence, inventory replication, join/leave messages, offline-mode chat, multi-client player spawning, relative/absolute movement, tab-list lifecycle, metadata placeholders, head rotation, equipment synchronization, authoritative damage/healing state, and subject-only Set Health replication are implemented. Damage-source animation, combat death packets, respawn flow, non-player entity gameplay, visibility/range-based entity tracking, and complete Vanilla systems remain incomplete.
""",
        """Dedicated Play reader/writer queues, a shared 20 TPS runtime, authoritative gameplay persistence, inventory replication, join/leave messages, offline-mode chat and commands, multi-client player spawning, relative/absolute movement, tab-list lifecycle, metadata placeholders, head rotation, equipment synchronization, authoritative damage/healing state, Hurt Animation, combat death, client-command respawn, and subject-only Set Health replication are implemented. Damage Event source typing, attribute resynchronization, non-player entity gameplay, visibility/range-based entity tracking, and complete Vanilla systems remain incomplete.
""",
        "roadmap respawn status",
    )


patch("crates/ferrum-protocol/src/lib.rs", patch_protocol_lib)
patch("crates/ferrum-protocol/src/packet_catalog.rs", patch_packet_catalog)
patch("crates/ferrum-version-26-1-2/src/lib.rs", patch_version)
patch("crates/ferrum-play/src/lib.rs", patch_play_lib)
patch("crates/ferrum-game/src/state.rs", patch_game_state)
patch("crates/ferrum-server/src/game_runtime.rs", patch_game_runtime)
patch("crates/ferrum-server/src/play_runtime.rs", patch_play_runtime)
patch("crates/ferrum-server/src/game_replication.rs", patch_replication)
patch("crates/ferrum-server/src/main.rs", patch_main)
patch("docs/SERVER_ROADMAP.md", patch_roadmap)
