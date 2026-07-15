from pathlib import Path


def one(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected 1, found {count}")
    return text.replace(old, new, 1)


# Extend the deterministic version pack with an official entity-type protocol
# palette generated from Mojang's registries.json report.
path = Path("crates/ferrum-rompack/src/lib.rs")
text = path.read_text()
text = one(text, "pub const ROMPACK_SCHEMA_VERSION: u32 = 7;", "pub const ROMPACK_SCHEMA_VERSION: u32 = 8;", "rompack schema")
text = one(
    text,
    """    pub max_items: usize,
    pub max_data_components: usize,
""",
    """    pub max_items: usize,
    pub max_entity_types: usize,
    pub max_data_components: usize,
""",
    "entity limit field",
)
text = one(
    text,
    """            max_items: 100_000,
            max_data_components: 100_000,
""",
    """            max_items: 100_000,
            max_entity_types: 100_000,
            max_data_components: 100_000,
""",
    "entity limit default",
)
text = one(
    text,
    """    /// Static item registry IDs used by Play-state ItemStack codecs.
    pub items: Vec<RomPackItem>,
    /// Static data-component-type IDs used by version-aware ItemStack codecs.
""",
    """    /// Static item registry IDs used by Play-state ItemStack codecs.
    pub items: Vec<RomPackItem>,
    /// Static entity-type IDs used by Play-state entity replication codecs.
    pub entity_types: Vec<RomPackEntityType>,
    /// Static data-component-type IDs used by version-aware ItemStack codecs.
""",
    "entity pack field",
)
text = one(
    text,
    """#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackDataComponent {
""",
    """#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackEntityType {
    pub entity_type: String,
    pub protocol_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackDataComponent {
""",
    "entity pack record",
)
validation_anchor = """    if pack.data_components.len() > limits.max_data_components {
"""
entity_validation = r'''    if pack.entity_types.is_empty() {
        bail!("version pack does not contain an entity-type registry");
    }
    if pack.entity_types.len() > limits.max_entity_types {
        bail!("version pack contains too many entity-type registry records");
    }
    let mut previous_entity_type: Option<&str> = None;
    let mut entity_protocol_ids = BTreeSet::new();
    for entity_type in &pack.entity_types {
        validate_resource_location(
            "entity type ID",
            &entity_type.entity_type,
            limits.max_identifier_bytes,
        )?;
        if previous_entity_type
            .is_some_and(|previous| previous >= entity_type.entity_type.as_str())
        {
            bail!("version-pack entity types must be strictly sorted and unique");
        }
        previous_entity_type = Some(&entity_type.entity_type);
        if entity_type.protocol_id < 0 {
            bail!(
                "entity type {} protocol ID cannot be negative",
                entity_type.entity_type
            );
        }
        if !entity_protocol_ids.insert(entity_type.protocol_id) {
            bail!("duplicate entity type protocol ID {}", entity_type.protocol_id);
        }
    }
    if !pack
        .entity_types
        .iter()
        .any(|entity_type| entity_type.entity_type == "minecraft:player")
    {
        bail!("version pack entity-type registry is missing minecraft:player");
    }

'''
text = one(text, validation_anchor, entity_validation + validation_anchor, "entity validation")
text = one(
    text,
    """            data_components: Vec::new(),
            packet_catalog: vec![
""",
    """            entity_types: vec![
                RomPackEntityType {
                    entity_type: "minecraft:item".to_owned(),
                    protocol_id: 0,
                },
                RomPackEntityType {
                    entity_type: "minecraft:player".to_owned(),
                    protocol_id: 1,
                },
            ],
            data_components: Vec::new(),
            packet_catalog: vec![
""",
    "sample entity palette",
)
text = one(
    text,
    """        let mut pack = sample_pack();
        pack.registries[0].entries.reverse();
""",
    """        let mut pack = sample_pack();
        pack.entity_types.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.entity_types[1].protocol_id = pack.entity_types[0].protocol_id;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.registries[0].entries.reverse();
""",
    "entity validation tests",
)
path.write_text(text)


# Parse item, entity type, and data-component IDs from the same bounded report.
path = Path("crates/rom-bootstrap/src/registry_report.rs")
text = path.read_text()
text = one(
    text,
    "use ferrum_rompack::{RomPackDataComponent, RomPackItem};",
    "use ferrum_rompack::{RomPackDataComponent, RomPackEntityType, RomPackItem};",
    "registry report import",
)
text = one(
    text,
    """pub struct RegistryProtocolReport {
    pub items: Vec<RomPackItem>,
    pub data_components: Vec<RomPackDataComponent>,
}
""",
    """pub struct RegistryProtocolReport {
    pub items: Vec<RomPackItem>,
    pub entity_types: Vec<RomPackEntityType>,
    pub data_components: Vec<RomPackDataComponent>,
}
""",
    "registry report entity field",
)
text = one(
    text,
    """    let data_components = parse_registry(&root, "minecraft:data_component_type")?
""",
    """    let entity_types = parse_registry(&root, "minecraft:entity_type")?
        .into_iter()
        .map(|(entity_type, protocol_id)| RomPackEntityType {
            entity_type,
            protocol_id,
        })
        .collect();
    let data_components = parse_registry(&root, "minecraft:data_component_type")?
""",
    "parse entity registry",
)
text = one(
    text,
    """    Ok(RegistryProtocolReport {
        items,
        data_components,
    })
""",
    """    Ok(RegistryProtocolReport {
        items,
        entity_types,
        data_components,
    })
""",
    "return entity registry",
)
text = one(
    text,
    """        "minecraft:data_component_type": {
""",
    """        "minecraft:entity_type": {
            "entries": {
                "minecraft:item": { "protocol_id": 2 },
                "minecraft:player": { "protocol_id": 147 }
            }
        },
        "minecraft:data_component_type": {
""",
    "registry test entity data",
)
text = one(
    text,
    """        assert_eq!(report.items[1].protocol_id, 1);
        assert_eq!(report.data_components[0].component, "minecraft:custom_name");
""",
    """        assert_eq!(report.items[1].protocol_id, 1);
        assert_eq!(report.entity_types[0].entity_type, "minecraft:item");
        assert_eq!(report.entity_types[1].protocol_id, 147);
        assert_eq!(report.data_components[0].component, "minecraft:custom_name");
""",
    "registry entity assertions",
)
path.write_text(text)


# Include the generated entity palette in newly generated packs.
path = Path("crates/rom-bootstrap/src/extract.rs")
text = path.read_text()
text = one(
    text,
    """    let items = protocol_registries.items;
    let data_components = protocol_registries.data_components;
""",
    """    let items = protocol_registries.items;
    let entity_types = protocol_registries.entity_types;
    let data_components = protocol_registries.data_components;
""",
    "extract entity palette",
)
text = one(
    text,
    """        items,
        data_components,
        registries,
""",
    """        items,
        entity_types,
        data_components,
        registries,
""",
    "pack entity palette",
)
path.write_text(text)


# Carry the generated palette into the live server and replication service.
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
    """        registry_payloads,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    """        registry_payloads,
        entity_protocol_ids,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    "runtime tuple names",
)
text = one(
    text,
    """            loaded.registry_payloads,
            loaded.item_protocol_ids,
            loaded.data_component_protocol_ids,
""",
    """            loaded.registry_payloads,
            loaded.entity_protocol_ids,
            loaded.item_protocol_ids,
            loaded.data_component_protocol_ids,
""",
    "loaded tuple values",
)
text = one(
    text,
    """            registry_payloads,
            ItemProtocolRegistry::default(),
            DataComponentProtocolRegistry::default(),
""",
    """            registry_payloads,
            EntityProtocolRegistry::default(),
            ItemProtocolRegistry::default(),
            DataComponentProtocolRegistry::default(),
""",
    "builtin tuple entity default",
)
text = one(
    text,
    """        registry_payloads,
        config.play_policy.clone(),
        loaded_chunks,
""",
    """        registry_payloads,
        entity_protocol_ids,
        config.play_policy.clone(),
        loaded_chunks,
""",
    "production runtime entity registry",
)
text = one(
    text,
    """            registry_payloads,
            config.play_policy.clone(),
            None,
""",
    """            registry_payloads,
            EntityProtocolRegistry::default(),
            config.play_policy.clone(),
            None,
""",
    "test runtime entity default",
)
text = one(
    text,
    """        registry_payloads: Vec<Vec<u8>>,
        play_policy: PlayPolicy,
""",
    """        registry_payloads: Vec<Vec<u8>>,
        entity_protocol_ids: EntityProtocolRegistry,
        play_policy: PlayPolicy,
""",
    "runtime entity parameter",
)
text = one(
    text,
    """        let game_replication =
            spawn_game_replication(&game_runtime, GameReplicationConfig::default())?;
""",
    """        let game_replication = spawn_game_replication(
            &game_runtime,
            GameReplicationConfig {
                entity_protocol_ids,
                ..GameReplicationConfig::default()
            },
        )?;
""",
    "replication entity config",
)
text = one(
    text,
    """            registry_payloads,
            play_policy,
            Some(store),
""",
    """            registry_payloads,
            EntityProtocolRegistry::default(),
            play_policy,
            Some(store),
""",
    "loaded test entity default",
)
text = one(
    text,
    """    registry_payloads: Vec<Vec<u8>>,
    item_protocol_ids: ItemProtocolRegistry,
""",
    """    registry_payloads: Vec<Vec<u8>>,
    entity_protocol_ids: EntityProtocolRegistry,
    item_protocol_ids: ItemProtocolRegistry,
""",
    "loaded pack entity field",
)
text = one(
    text,
    """    let item_protocol_ids = ItemProtocolRegistry::new(
""",
    """    let entity_protocol_ids = EntityProtocolRegistry::new(
        pack.entity_types
            .iter()
            .map(|entity_type| (entity_type.entity_type.clone(), entity_type.protocol_id)),
    )
    .context("cannot build entity protocol registry from version pack")?;
    if entity_protocol_ids.protocol_id("minecraft:player").is_none() {
        bail!("version pack entity protocol registry is missing minecraft:player");
    }
    let item_protocol_ids = ItemProtocolRegistry::new(
""",
    "load entity registry",
)
text = one(
    text,
    """        registry_payloads,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    """        registry_payloads,
        entity_protocol_ids,
        item_protocol_ids,
        data_component_protocol_ids,
""",
    "return loaded entity registry",
)
path.write_text(text)


# Store the palette in replication configuration; the next slice consumes it
# for player-info and entity lifecycle packets.
path = Path("crates/ferrum-server/src/game_replication.rs")
text = path.read_text()
text = one(
    text,
    "use ferrum_game::{GameEvent, PLAYER_INVENTORY_SLOTS, PlayerUuid};",
    "use ferrum_game::{GameEvent, PLAYER_INVENTORY_SLOTS, PlayerUuid};\nuse ferrum_play::EntityProtocolRegistry;",
    "replication entity import",
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
