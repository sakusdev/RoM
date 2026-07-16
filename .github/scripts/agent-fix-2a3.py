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
    r"fn queue_player_spawn\(.*?\n\}\n\nfn player_equipment",
    '''fn queue_player_spawn(
    connection: &mut ReplicationConnection,
    snapshot: PlayerEntitySnapshot,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !entity_replication_enabled(&config.entity_protocol_ids)
        || !connection.active
        || !connection.healthy
    {
        return Ok(());
    }
    if let Some(tracked) = connection.entities.get(&snapshot.uuid) {
        if tracked.entity_id == snapshot.entity_id {
            connection.entities.insert(snapshot.uuid, snapshot);
            return Ok(());
        }
    }
    queue_player_entity_remove(connection, snapshot.uuid, None, exit)?;
    queue_player_info_update(connection, &snapshot, exit)?;
    let payload = encode_add_entity(
        snapshot.entity_id,
        snapshot.uuid,
        "minecraft:player",
        snapshot.transform,
        snapshot.velocity,
        &config.entity_protocol_ids,
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
    if snapshot.equipment.iter().any(|entry| entry.stack.is_some()) {
        queue_player_equipment(
            connection,
            snapshot.entity_id,
            &snapshot.equipment,
            config,
            exit,
        )?;
    }
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::RotateHead,
            payload: encode_rotate_head(snapshot.entity_id, snapshot.transform.yaw)
                .context("cannot encode initial player head rotation")?,
        },
        exit,
    );
    if connection.healthy {
        connection.entities.insert(snapshot.uuid, snapshot);
    }
    Ok(())
}

fn player_equipment''',
    "idempotent player spawn",
)
text = replace_once(
    text,
    '''fn queue_player_remove(
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
}''',
    '''fn queue_player_entity_remove(
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
    Ok(())
}

fn queue_player_remove(
    connection: &mut ReplicationConnection,
    uuid: PlayerUuid,
    fallback_entity_id: Option<EntityId>,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    queue_player_entity_remove(connection, uuid, fallback_entity_id, exit)?;
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::PlayerInfoRemove,
            payload: encode_player_info_remove(&[uuid])
                .context("cannot encode player info removal")?,
        },
        exit,
    );
    Ok(())
}''',
    "entity-only remove helper",
)
text = sub_once(
    text,
    r"\nfn online_player_snapshots\(runtime: &SharedGameRuntime\) -> Result<Vec<PlayerEntitySnapshot>> \{.*?\n\}\n\nfn player_snapshot_from_state",
    "\nfn player_snapshot_from_state",
    "remove obsolete online snapshot helper",
)
save(path, text)
