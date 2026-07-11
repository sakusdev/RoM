from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


# Enrich disconnect events with the stable player name needed by replication.
path = Path("crates/ferrum-game/src/state.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''    PlayerDisconnected {
        uuid: PlayerUuid,
        entity_id: Option<EntityId>,
    },''',
    '''    PlayerDisconnected {
        uuid: PlayerUuid,
        name: String,
        entity_id: Option<EntityId>,
    },''',
    "disconnect event name",
)
text = replace_once(
    text,
    '''        let entity_id = player.entity_id;
        if let Some(id) = entity_id {
            self.entities.despawn(id);
        }
        player.disconnect();
        Ok(vec![GameEvent::PlayerDisconnected { uuid, entity_id }])''',
    '''        let name = player.name.clone();
        let entity_id = player.entity_id;
        if let Some(id) = entity_id {
            self.entities.despawn(id);
        }
        player.disconnect();
        Ok(vec![GameEvent::PlayerDisconnected {
            uuid,
            name,
            entity_id,
        }])''',
    "disconnect event emission",
)
path.write_text(text, encoding="utf-8")

# Add semantic outputs so the writer remains version-aware.
path = Path("crates/ferrum-server/src/authoritative_runtime.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use anyhow::{Context, Result};\nuse ferrum_play::PlayerMovement;",
    "use anyhow::{Context, Result};\nuse ferrum_game::Transform;\nuse ferrum_play::PlayerMovement;",
    "transform import",
)
text = replace_once(
    text,
    "#[derive(Debug, Clone, PartialEq, Eq)]\npub enum PlayOutput {",
    "#[derive(Debug, Clone, PartialEq)]\npub enum PlayOutput {",
    "play output derive",
)
text = replace_once(
    text,
    '''    /// Request a protocol-aware Play disconnect with this reason.
    Disconnect(String),
}''',
    '''    /// Request a protocol-aware Play disconnect with this reason.
    Disconnect(String),
    /// Send a protocol-aware system chat component.
    SystemChat { message: String, overlay: bool },
    /// Teleport this connection using a connection-local teleport identifier.
    PlayerTeleport {
        teleport_id: i32,
        transform: Transform,
    },
}''',
    "semantic play outputs",
)
path.write_text(text, encoding="utf-8")

# Export the replication service.
path = Path("crates/ferrum-server/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    "pub mod game_runtime;\n",
    "pub mod game_replication;\npub mod game_runtime;\n",
    "replication module export",
)
path.write_text(text, encoding="utf-8")

# Wire the global replication service into the server lifecycle and live writer.
path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''    authoritative_runtime::{PlayInput, PlayOutput},
    game_runtime::SharedGameRuntime,''',
    '''    authoritative_runtime::{PlayInput, PlayOutput},
    game_replication::{
        GameReplicationConfig, GameReplicationService, spawn_game_replication,
    },
    game_runtime::SharedGameRuntime,''',
    "replication imports",
)
text = replace_once(
    text,
    '''    game_runtime: SharedGameRuntime,
    game_service: GameService,
    shutdown: Arc<AtomicBool>,''',
    '''    game_runtime: SharedGameRuntime,
    game_service: GameService,
    game_replication: GameReplicationService,
    shutdown: Arc<AtomicBool>,''',
    "server replication field",
)
text = replace_once(
    text,
    '''        let shared_play_runtime = spawn_shared_play_runtime(shared_runtime_config)?;
        Ok(Self {''',
    '''        let shared_play_runtime = spawn_shared_play_runtime(shared_runtime_config)?;
        let game_replication =
            spawn_game_replication(&game_runtime, GameReplicationConfig::default())?;
        Ok(Self {''',
    "replication service construction",
)
text = replace_once(
    text,
    '''            game_runtime,
            game_service,
            shutdown: Arc::new(AtomicBool::new(false)),''',
    '''            game_runtime,
            game_service,
            game_replication,
            shutdown: Arc::new(AtomicBool::new(false)),''',
    "replication service storage",
)
text = replace_once(
    text,
    '''        let (play_reader, play_writer) = match endpoints {
            Ok(endpoints) => endpoints,
            Err(error) => {
                let _ = self.game_runtime.disconnect_player(player_uuid);
                return Err(error.into());
            }
        };
        self.online_players.fetch_add(1, Ordering::Relaxed);''',
    '''        let (play_reader, play_writer) = match endpoints {
            Ok(endpoints) => endpoints,
            Err(error) => {
                let _ = self.game_runtime.disconnect_player(player_uuid);
                return Err(error.into());
            }
        };
        if let Err(error) = self
            .game_replication
            .control()
            .register(player_uuid, play_reader.clone())
        {
            let _ = play_reader.try_disconnect();
            let _ = self.game_runtime.disconnect_player(player_uuid);
            return Err(error.context("cannot register player for gameplay replication"));
        }
        self.online_players.fetch_add(1, Ordering::Relaxed);''',
    "replication connection registration",
)
text = replace_once(
    text,
    '''    fn shutdown(self) -> Result<GameServiceExit> {
        self.game_service.shutdown()
    }''',
    '''    fn shutdown(self) -> Result<GameServiceExit> {
        let replication = self.game_replication.shutdown()?;
        println!(
            "game replication stopped after {} events, {} outputs sent, {} deferred, {} dropped",
            replication.events,
            replication.sent_outputs,
            replication.deferred_outputs,
            replication.dropped_outputs
        );
        self.game_service.shutdown()
    }''',
    "replication shutdown",
)
text = replace_once(
    text,
    '''    fn drop(&mut self) {
        let _ = self.play_reader.try_disconnect();
        let _ = self.state.game_runtime.disconnect_player(self.player_uuid);''',
    '''    fn drop(&mut self) {
        let _ = self
            .state
            .game_replication
            .control()
            .unregister(self.player_uuid);
        let _ = self.play_reader.try_disconnect();
        let _ = self.state.game_runtime.disconnect_player(self.player_uuid);''',
    "replication connection unregistration",
)
text = replace_once(
    text,
    '''struct PlayOutputPacketIds {
    keep_alive_request: i32,
    disconnect: i32,
}''',
    '''struct PlayOutputPacketIds {
    keep_alive_request: i32,
    disconnect: i32,
    system_chat: i32,
    player_position: i32,
}''',
    "semantic output packet ids",
)
text = replace_once(
    text,
    '''        keep_alive_request: profile.packets().require(PacketKind::KeepAliveRequest)?,
        disconnect: profile.packets().require(PacketKind::PlayDisconnect)?,''',
    '''        keep_alive_request: profile.packets().require(PacketKind::KeepAliveRequest)?,
        disconnect: profile.packets().require(PacketKind::PlayDisconnect)?,
        system_chat: profile.packets().require(PacketKind::SystemChat)?,
        player_position: profile.packets().require(PacketKind::PlayerPosition)?,''',
    "semantic output packet id lookup",
)
text = replace_once(
    text,
    '''        PlayOutput::Disconnect(reason) => {
            let payload = encode_play_disconnect(&reason)?;
            write_packet(
                writer,
                &build_packet(packet_ids.disconnect, |body| {
                    body.extend_from_slice(&payload);
                    Ok(())
                })?,
            )?;
            PlayWriterDirective::Stop
        }
    };''',
    '''        PlayOutput::Disconnect(reason) => {
            let payload = encode_play_disconnect(&reason)?;
            write_packet(
                writer,
                &build_packet(packet_ids.disconnect, |body| {
                    body.extend_from_slice(&payload);
                    Ok(())
                })?,
            )?;
            PlayWriterDirective::Stop
        }
        PlayOutput::SystemChat { message, overlay } => {
            let payload = encode_system_chat(&message, overlay)?;
            write_packet(
                writer,
                &build_packet(packet_ids.system_chat, |body| {
                    body.extend_from_slice(&payload);
                    Ok(())
                })?,
            )?;
            PlayWriterDirective::Continue
        }
        PlayOutput::PlayerTeleport {
            teleport_id,
            transform,
        } => {
            let payload = encode_player_position(&PlayerPosition {
                teleport_id,
                change: PositionMoveRotation {
                    position: transform.position,
                    delta_movement: [0.0; 3],
                    yaw: transform.yaw,
                    pitch: transform.pitch,
                },
                relative_flags: 0,
            })?;
            write_packet(
                writer,
                &build_packet(packet_ids.player_position, |body| {
                    body.extend_from_slice(&payload);
                    Ok(())
                })?,
            )?;
            PlayWriterDirective::Continue
        }
    };''',
    "semantic output wire encoding",
)
path.write_text(text, encoding="utf-8")

# Teach the local movement mirror to refresh after externally issued teleports.
path = Path("crates/ferrum-server/src/play_runtime.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''    fn synchronize(self, player: &PlayerState) -> Result<()> {
        let transform =
            Transform::new(player.position, player.yaw, player.pitch, player.on_ground)?;
        self.runtime.move_player(self.player_uuid, transform)?;
        Ok(())
    }
}''',
    '''    fn synchronize(self, player: &PlayerState) -> Result<()> {
        let transform =
            Transform::new(player.position, player.yaw, player.pitch, player.on_ground)?;
        self.runtime.move_player(self.player_uuid, transform)?;
        Ok(())
    }

    fn refresh(self, player: &mut PlayerState) -> Result<()> {
        let transform = self.runtime.with_state(|state| {
            state
                .player(self.player_uuid)
                .and_then(|player| player.entity_id)
                .and_then(|entity_id| state.entities().get(entity_id))
                .map(|entity| entity.transform)
        })?;
        if let Some(transform) = transform {
            player.position = transform.position;
            player.yaw = transform.yaw;
            player.pitch = transform.pitch;
            player.on_ground = transform.on_ground;
        }
        Ok(())
    }
}''',
    "authoritative transform refresh",
)
text = replace_once(
    text,
    '''                        PlayInput::Movement(movement) => {
                            validate_movement_delta(&player, movement)?;''',
    '''                        PlayInput::Movement(movement) => {
                            if let Some(gameplay) = gameplay {
                                gameplay.refresh(&mut player)?;
                                let authoritative_chunk = player.chunk_pos();
                                if authoritative_chunk != view.center() {
                                    let delta = view.recenter(authoritative_chunk)?;
                                    shared_world.ensure_chunks_loaded(&delta.newly_visible)?;
                                    send_chunk_view_delta(
                                        writer,
                                        profile,
                                        shared_world,
                                        authoritative_chunk,
                                        &delta,
                                        play_reader,
                                    )?;
                                }
                            }
                            validate_movement_delta(&player, movement)?;''',
    "movement refresh before validation",
)
text = replace_once(
    text,
    '''                Some(PacketKind::PlayerAction) => {''',
    '''                Some(PacketKind::AcceptTeleportation) => {
                    let teleport_id = packet_reader.read_varint()?;
                    if teleport_id < 0 {
                        bail!("teleport acknowledgement id cannot be negative");
                    }
                    if !packet_reader.take_remaining().is_empty() {
                        bail!("teleport acknowledgement contains trailing bytes");
                    }
                }
                Some(PacketKind::PlayerAction) => {''',
    "live teleport acknowledgement",
)
path.write_text(text, encoding="utf-8")

# Keep writer tests exhaustive for the new semantic variants.
path = Path("crates/ferrum-server/src/play_writer.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''                    PlayOutput::KeepAliveRequest(id) => writer.write_all(&id.to_be_bytes())?,
                    PlayOutput::Disconnect(_) => return Ok(PlayWriterDirective::Stop),''',
    '''                    PlayOutput::KeepAliveRequest(id) => writer.write_all(&id.to_be_bytes())?,
                    PlayOutput::SystemChat { .. } | PlayOutput::PlayerTeleport { .. } => {}
                    PlayOutput::Disconnect(_) => return Ok(PlayWriterDirective::Stop),''',
    "writer drain test semantic variants",
)
text = replace_once(
    text,
    '''                PlayOutput::Packet(_) | PlayOutput::KeepAliveRequest(_) => {
                    Ok(PlayWriterDirective::Continue)
                }''',
    '''                PlayOutput::Packet(_)
                | PlayOutput::KeepAliveRequest(_)
                | PlayOutput::SystemChat { .. }
                | PlayOutput::PlayerTeleport { .. } => Ok(PlayWriterDirective::Continue)''',
    "writer handler semantic variants",
)
path.write_text(text, encoding="utf-8")
