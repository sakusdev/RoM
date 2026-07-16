from pathlib import Path
import re


def load(path: str) -> str:
    return Path(path).read_text()


def save(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, repl: str, label: str) -> str:
    new, count = re.subn(pattern, repl, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return new

# Wire the authoritative entity id into PlayLogin, activate replication only after
# the complete bootstrap has entered the shared writer queue, and fail hard on
# missing packet IDs.
path = "crates/ferrum-server/src/main.rs"
text = load(path)
text = replace_once(
    text,
    "use ferrum_game::{CommandSource, GameState, PlayerUuid as GamePlayerUuid, Transform};",
    "use ferrum_game::{CommandSource, EntityId, GameState, PlayerUuid as GamePlayerUuid, Transform};",
    "main EntityId import",
)
text = replace_once(
    text,
    '''struct InitialInventorySync {
    control: GameReplicationControl,
    uuid: GamePlayerUuid,
}''',
    '''struct InitialInventorySync {
    control: GameReplicationControl,
    uuid: GamePlayerUuid,
    entity_id: EntityId,
}''',
    "initial sync entity id",
)
text = replace_once(
    text,
    '''        if let Err(error) =
            self.game_runtime
                .connect_player(player_uuid, identity.username.clone(), transform)
        {
            let _ = self.game_replication.control().unregister(player_uuid);
            let _ = play_reader.try_disconnect();
            return Err(error.into());
        }
        self.online_players.fetch_add(1, Ordering::Relaxed);
        Ok(OnlinePlayerGuard {
            state: self,
            connection_id,
            player_uuid,
            play_reader,
            play_writer: Some(play_writer),
        })''',
    '''        if let Err(error) =
            self.game_runtime
                .connect_player(player_uuid, identity.username.clone(), transform)
        {
            let _ = self.game_replication.control().unregister(player_uuid);
            let _ = play_reader.try_disconnect();
            return Err(error.into());
        }
        let entity_id = self
            .game_runtime
            .with_state(|state| state.player(player_uuid).and_then(|player| player.entity_id))?
            .with_context(|| format!("connected player {player_uuid:?} has no entity id"))?;
        self.online_players.fetch_add(1, Ordering::Relaxed);
        Ok(OnlinePlayerGuard {
            state: self,
            connection_id,
            player_uuid,
            entity_id,
            play_reader,
            play_writer: Some(play_writer),
        })''',
    "authoritative entity id capture",
)
text = replace_once(
    text,
    '''    player_uuid: GamePlayerUuid,
    play_reader: PlayReaderEndpoint,''',
    '''    player_uuid: GamePlayerUuid,
    entity_id: EntityId,
    play_reader: PlayReaderEndpoint,''',
    "online guard entity id field",
)
text = replace_once(
    text,
    '''    fn play_reader(&self) -> &PlayReaderEndpoint {
        &self.play_reader
    }''',
    '''    fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    fn play_reader(&self) -> &PlayReaderEndpoint {
        &self.play_reader
    }''',
    "online guard entity id accessor",
)
text = replace_once(
    text,
    '''    let initial_inventory_sync = play_reader.is_some().then(|| InitialInventorySync {
        control: context.state.game_replication.control(),
        uuid: online_player.player_uuid(),
    });''',
    '''    let initial_inventory_sync = play_reader.is_some().then(|| InitialInventorySync {
        control: context.state.game_replication.control(),
        uuid: online_player.player_uuid(),
        entity_id: online_player.entity_id(),
    });''',
    "initial sync construction",
)
text = replace_once(
    text,
    '''    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::PlayLogin,
        &encode_join_game(&static_join_game(config, world_profile))?,
        play_reader,
    )?;''',
    '''    let player_entity_id = initial_inventory_sync
        .as_ref()
        .map(|sync| i32::try_from(sync.entity_id.get()).context("player entity ID exceeds i32"))
        .transpose()?
        .unwrap_or(STATIC_PLAYER_ID);
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::PlayLogin,
        &encode_join_game(&static_join_game(config, world_profile, player_entity_id))?,
        play_reader,
    )?;''',
    "dynamic PlayLogin entity id",
)
text = replace_once(
    text,
    '''    if let Some(sync) = initial_inventory_sync {
        sync.control
            .sync_inventory(sync.uuid)
            .context("cannot queue initial player inventory after Play bootstrap")?;
    }''',
    '''    if let Some(sync) = initial_inventory_sync {
        sync.control
            .activate(sync.uuid)
            .context("cannot activate gameplay replication after Play bootstrap")?;
        sync.control
            .sync_inventory(sync.uuid)
            .context("cannot queue initial player inventory after Play bootstrap")?;
    }''',
    "post-bootstrap replication activation",
)
text = replace_once(
    text,
    "fn static_join_game(config: &ServerConfig, world: &RomPackWorld) -> JoinGame {\n    JoinGame {\n        player_id: STATIC_PLAYER_ID,",
    "fn static_join_game(config: &ServerConfig, world: &RomPackWorld, player_id: i32) -> JoinGame {\n    JoinGame {\n        player_id,",
    "static join game dynamic id",
)
text = replace_once(
    text,
    '''            PlayOutput::ProtocolPacket { kind, payload } => {
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
            }''',
    '''            PlayOutput::ProtocolPacket { kind, payload } => {
                let packet_id = protocol_profile.packets().require(kind)?;
                write_packet(
                    writer,
                    &build_packet(packet_id, |body| {
                        body.extend_from_slice(&payload);
                        Ok(())
                    })?,
                )?;
                writer.flush()?;
                Ok(PlayWriterDirective::Continue)
            }''',
    "missing protocol packet hard failure",
)
text = replace_once(
    text,
    '''    config.runtime_profile = Some(runtime_profile);
    config.item_protocol_ids = item_protocol_ids.clone();''',
    '''    validate_replication_packet_support(
        &runtime_profile,
        &entity_protocol_ids,
        &item_protocol_ids,
    )?;
    config.runtime_profile = Some(runtime_profile);
    config.item_protocol_ids = item_protocol_ids.clone();''',
    "startup packet validation call",
)
# Insert startup validation before registry payload conversion helpers.
text = replace_once(
    text,
    '''fn registry_payloads_from_pack(registries: &[RomPackRegistry]) -> Result<Vec<Vec<u8>>> {''',
    '''fn validate_replication_packet_support(
    profile: &ProtocolProfile,
    entity_protocol_ids: &EntityProtocolRegistry,
    item_protocol_ids: &ItemProtocolRegistry,
) -> Result<()> {
    for kind in [
        PacketKind::SetHealth,
        PacketKind::HurtAnimation,
        PacketKind::PlayerCombatKill,
        PacketKind::Respawn,
    ] {
        profile
            .packets()
            .require(kind)
            .with_context(|| format!("replication requires {kind:?}"))?;
    }
    if entity_protocol_ids.protocol_id("minecraft:player").is_some() {
        for kind in [
            PacketKind::PlayerInfoUpdate,
            PacketKind::PlayerInfoRemove,
            PacketKind::AddEntity,
            PacketKind::RemoveEntities,
            PacketKind::SetEntityData,
            PacketKind::RotateHead,
            PacketKind::MoveEntityPosition,
            PacketKind::MoveEntityPositionRotation,
            PacketKind::MoveEntityRotation,
            PacketKind::TeleportEntity,
        ] {
            profile
                .packets()
                .require(kind)
                .with_context(|| format!("player entity replication requires {kind:?}"))?;
        }
    }
    if !item_protocol_ids.is_empty() {
        profile
            .packets()
            .require(PacketKind::SetEquipment)
            .context("item-backed player replication requires SetEquipment")?;
    }
    Ok(())
}

fn registry_payloads_from_pack(registries: &[RomPackRegistry]) -> Result<Vec<Vec<u8>>> {''',
    "startup packet validation function",
)
# Update direct test calls to static_join_game with the old two-argument form.
text = text.replace("static_join_game(&config, &world)", "static_join_game(&config, &world, STATIC_PLAYER_ID)")
text = text.replace("static_join_game(config, world)", "static_join_game(config, world, STATIC_PLAYER_ID)")
# Add regression tests near the final test-module close.
text = replace_once(
    text,
    '''        assert_eq!(read_varint_io(&mut cursor).unwrap(), expected);
    }
}''',
    '''        assert_eq!(read_varint_io(&mut cursor).unwrap(), expected);
    }

    #[test]
    fn play_login_uses_the_authoritative_player_entity_id() {
        let config = ServerConfig::for_profile(Some(version_26_1_2::PROFILE_NAME)).unwrap();
        let world = play_runtime::builtin_world_profile();
        assert_eq!(static_join_game(&config, &world, 42).player_id, 42);
    }

    #[test]
    fn replication_packet_validation_rejects_incomplete_profiles() {
        let profile = ProtocolProfile::new("test", 1, PacketTable::new()).unwrap();
        let error = validate_replication_packet_support(
            &profile,
            &EntityProtocolRegistry::default(),
            &ItemProtocolRegistry::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("SetHealth"));
    }
}''',
    "main regression tests",
)
save(path, text)
