from pathlib import Path


def one(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected 1, found {count}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text()
text = one(
    text,
    """    BlockPosition, CommonPlayerSpawnInfo, DataComponentProtocolRegistry, DefaultSpawnPosition,
    GlobalPosition, ItemProtocolRegistry, JoinGame, PlayerPosition, PositionMoveRotation,
""",
    """    BlockPosition, CommonPlayerSpawnInfo, DataComponentProtocolRegistry, DefaultSpawnPosition,
    EntityProtocolRegistry, GlobalPosition, ItemProtocolRegistry, JoinGame, PlayerPosition,
    PositionMoveRotation,
""",
    "entity registry import",
)
text = one(
    text,
    """        let game_replication =
            spawn_game_replication(&game_runtime, GameReplicationConfig::default())?;
""",
    """        let game_replication = spawn_game_replication(
            &game_runtime,
            GameReplicationConfig {
                entity_protocol_ids: builtin_entity_protocol_ids()?,
                ..GameReplicationConfig::default()
            },
        )?;
""",
    "replication registry config",
)
anchor = """fn builtin_26_1_2_registry_payloads() -> Result<Vec<Vec<u8>>> {
    encode_registry_payloads(version_26_1_2::configuration_registries())
}

"""
helper = r'''fn builtin_entity_protocol_ids() -> Result<EntityProtocolRegistry> {
    let registry = version_26_1_2::SYNCHRONIZED_REGISTRIES
        .iter()
        .find(|registry| registry.id == "minecraft:entity_type")
        .context("built-in profile is missing minecraft:entity_type registry")?;
    let mut entries = Vec::with_capacity(registry.entries.len());
    for (protocol_id, name) in registry.entries.iter().copied().enumerate() {
        entries.push((
            name,
            i32::try_from(protocol_id).context("entity protocol ID exceeds i32")?,
        ));
    }
    EntityProtocolRegistry::new(entries).context("cannot build built-in entity protocol registry")
}

'''
text = one(text, anchor, anchor + helper, "entity registry helper")
path.write_text(text)

path = Path("crates/ferrum-server/src/game_replication.rs")
text = path.read_text()
text = one(
    text,
    "use ferrum_game::{GameEvent, PLAYER_INVENTORY_SLOTS, PlayerUuid};",
    "use ferrum_game::{GameEvent, PLAYER_INVENTORY_SLOTS, PlayerUuid};\nuse ferrum_play::EntityProtocolRegistry;",
    "replication registry import",
)
text = one(
    text,
    """pub struct GameReplicationConfig {
    pub event_capacity: NonZeroUsize,
    pub command_capacity: NonZeroUsize,
    pub pending_output_limit: NonZeroUsize,
    pub poll_interval: Duration,
}
""",
    """pub struct GameReplicationConfig {
    pub event_capacity: NonZeroUsize,
    pub command_capacity: NonZeroUsize,
    pub pending_output_limit: NonZeroUsize,
    pub poll_interval: Duration,
    pub entity_protocol_ids: EntityProtocolRegistry,
}
""",
    "replication config field",
)
text = one(
    text,
    """            poll_interval: DEFAULT_POLL_INTERVAL,
        }
""",
    """            poll_interval: DEFAULT_POLL_INTERVAL,
            entity_protocol_ids: EntityProtocolRegistry::default(),
        }
""",
    "replication config default",
)
path.write_text(text)
