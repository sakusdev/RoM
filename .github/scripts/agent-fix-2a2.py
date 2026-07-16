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

path = "crates/ferrum-server/src/game_replication.rs"
text = load(path)
text = sub_once(
    text,
    r"        GameEvent::PlayerConnected \{ uuid, name, \.\. \} => \{.*?\n        \}\n        GameEvent::PlayerDisconnected",
    '''        GameEvent::PlayerConnected { uuid, name, .. } => {
            let snapshot = if entity_replication_enabled(&config.entity_protocol_ids) {
                Some(player_snapshot(runtime, uuid)?.with_context(|| {
                    format!("connected player {uuid:?} is missing from authoritative state")
                })?)
            } else {
                None
            };
            let vitals = runtime
                .with_state(|state| state.player(uuid).map(|player| player.vitals))
                .context("cannot read connected player vitals")?;
            if let Some(connection) = connections.get_mut(&uuid)
                && connection.active
                && !connection.self_initialized
            {
                if let Some(vitals) = vitals {
                    queue_set_health(connection, vitals, exit)?;
                }
                if let Some(snapshot) = snapshot.as_ref() {
                    queue_player_info_update(connection, snapshot, exit)?;
                }
                connection.self_initialized = true;
            }
            if let Some(snapshot) = snapshot {
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
        GameEvent::PlayerDisconnected''',
    "player connected dispatch",
)
text = sub_once(
    text,
    r"        GameEvent::PlayerKilled \{.*?\n        \}\n        GameEvent::PlayerRespawned \{.*?\n        \}\n        GameEvent::SelectedHotbarChanged",
    '''        GameEvent::PlayerKilled {
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
                        queue_player_entity_remove(connection, uuid, Some(entity_id), exit)?;
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
        }
        GameEvent::SelectedHotbarChanged''',
    "death and respawn dispatch",
)
text = replace_once(
    text,
    '''const fn protocol_game_mode(game_mode: GameMode) -> i8 {
    match game_mode {
        GameMode::Survival => 0,
        GameMode::Creative => 1,
        GameMode::Adventure => 2,
        GameMode::Spectator => 3,
    }
}
''',
    '''const fn protocol_game_mode(game_mode: GameMode) -> i8 {
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
''',
    "previous game mode helper",
)
save(path, text)
