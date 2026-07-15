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
    """#[derive(Debug, Clone, Default)]
struct PersistenceConfig {
    game: GameServiceConfig,
    world: WorldServiceConfig,
}

""",
    """#[derive(Debug, Clone, Default)]
struct PersistenceConfig {
    game: GameServiceConfig,
    world: WorldServiceConfig,
}

#[derive(Debug, Clone, Default)]
struct RuntimeRegistryData {
    configuration_payloads: Vec<Vec<u8>>,
    entity_protocol_ids: EntityProtocolRegistry,
}

""",
    "runtime registry data type",
)

text = one(
    text,
    """            registry_payloads,
            EntityProtocolRegistry::default(),
            config.play_policy.clone(),
""",
    """            RuntimeRegistryData {
                configuration_payloads: registry_payloads,
                entity_protocol_ids: EntityProtocolRegistry::default(),
            },
            config.play_policy.clone(),
""",
    "test runtime registry bundle",
)

text = one(
    text,
    """    fn with_runtime(
        initial_online_players: i32,
        world: RomPackWorld,
        registry_payloads: Vec<Vec<u8>>,
        entity_protocol_ids: EntityProtocolRegistry,
        play_policy: PlayPolicy,
""",
    """    fn with_runtime(
        initial_online_players: i32,
        world: RomPackWorld,
        registries: RuntimeRegistryData,
        play_policy: PlayPolicy,
""",
    "with runtime signature",
)

text = one(
    text,
    """    ) -> Result<Self> {
        let center = play_runtime::spawn_chunk(&world);
""",
    """    ) -> Result<Self> {
        let RuntimeRegistryData {
            configuration_payloads,
            entity_protocol_ids,
        } = registries;
        let center = play_runtime::spawn_chunk(&world);
""",
    "runtime registry destructure",
)

text = one(
    text,
    """            registry_payloads,
            shared_play_runtime,
""",
    """            registry_payloads: configuration_payloads,
            shared_play_runtime,
""",
    "store configuration payloads",
)

text = one(
    text,
    """            registry_payloads,
            EntityProtocolRegistry::default(),
            play_policy,
            Some(store),
""",
    """            RuntimeRegistryData {
                configuration_payloads: registry_payloads,
                entity_protocol_ids: EntityProtocolRegistry::default(),
            },
            play_policy,
            Some(store),
""",
    "loaded runtime registry bundle",
)

text = one(
    text,
    """        registry_payloads,
        entity_protocol_ids,
        config.play_policy.clone(),
        loaded_chunks,
""",
    """        RuntimeRegistryData {
            configuration_payloads: registry_payloads,
            entity_protocol_ids,
        },
        config.play_policy.clone(),
        loaded_chunks,
""",
    "production runtime registry bundle",
)

path.write_text(text)
