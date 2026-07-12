from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement target, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# .rompack schema v6 stores authoritative item protocol IDs.
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 5;",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 6;",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''    pub max_packets: usize,
    pub max_sections: usize,''',
    '''    pub max_packets: usize,
    pub max_items: usize,
    pub max_sections: usize,''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''            max_packets: 4_096,
            max_sections: 1_024,''',
    '''            max_packets: 4_096,
            max_items: 100_000,
            max_sections: 1_024,''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''    pub packet_catalog: Vec<PacketDescriptor>,
    pub world: RomPackWorld,
    pub registries: Vec<RomPackRegistry>,''',
    '''    pub packet_catalog: Vec<PacketDescriptor>,
    pub world: RomPackWorld,
    /// Static item registry IDs used by Play-state ItemStack codecs.
    pub items: Vec<RomPackItem>,
    pub registries: Vec<RomPackRegistry>,''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackRegistry {''',
    '''#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackItem {
    pub item: String,
    pub protocol_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackRegistry {''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub registry_count: usize,''',
    '''    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub item_count: usize,
    pub registry_count: usize,''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''    if pack.registries.len() > limits.max_registries {''',
    '''    if pack.items.len() > limits.max_items {
        bail!("version pack contains too many item registry records");
    }
    let mut previous_item: Option<&str> = None;
    let mut item_protocol_ids = BTreeSet::new();
    for item in &pack.items {
        validate_resource_location("item ID", &item.item, limits.max_identifier_bytes)?;
        if previous_item.is_some_and(|previous| previous >= item.item.as_str()) {
            bail!("version-pack items must be strictly sorted and unique");
        }
        previous_item = Some(&item.item);
        if item.protocol_id < 0 {
            bail!("item {} protocol ID cannot be negative", item.item);
        }
        if !item_protocol_ids.insert(item.protocol_id) {
            bail!("duplicate item protocol ID {}", item.protocol_id);
        }
    }

    if pack.registries.len() > limits.max_registries {''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''        packet_count: pack.packets.len(),
        packet_catalog_count: pack.packet_catalog.len(),
        registry_count: pack.registries.len(),''',
    '''        packet_count: pack.packets.len(),
        packet_catalog_count: pack.packet_catalog.len(),
        item_count: pack.items.len(),
        registry_count: pack.registries.len(),''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''            packet_catalog: PacketCatalog::new([''',
    '''            items: vec![
                RomPackItem {
                    item: "minecraft:air".to_owned(),
                    protocol_id: 0,
                },
                RomPackItem {
                    item: "minecraft:stone".to_owned(),
                    protocol_id: 1,
                },
            ],
            packet_catalog: PacketCatalog::new([''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''        assert_eq!(written.packet_count, 3);
        assert_eq!(written.packet_catalog_count, 3);
        assert_eq!(written.registry_count, 2);''',
    '''        assert_eq!(written.packet_count, 3);
        assert_eq!(written.packet_catalog_count, 3);
        assert_eq!(written.item_count, 2);
        assert_eq!(written.registry_count, 2);''',
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    '''        let mut pack = sample_pack();
        pack.registries[0].entries.reverse();''',
    '''        let mut pack = sample_pack();
        pack.items.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.items[1].protocol_id = pack.items[0].protocol_id;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.registries[0].entries.reverse();''',
)

# Bootstrap module and option plumbing.
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    '''mod extract;
mod packet_report;
mod setup;''',
    '''mod extract;
mod packet_report;
mod registry_report;
mod setup;''',
)
replace_once(
    "crates/rom-bootstrap/src/setup.rs",
    '''    pub force_generate: bool,
    pub packet_report: Option<PathBuf>,
    pub workspace: PathBuf,''',
    '''    pub force_generate: bool,
    pub packet_report: Option<PathBuf>,
    pub registry_report: Option<PathBuf>,
    pub workspace: PathBuf,''',
)
replace_once(
    "crates/rom-bootstrap/src/setup.rs",
    '''        force: options.force_generate,
        packet_report: options.packet_report.clone(),
    })?;''',
    '''        force: options.force_generate,
        packet_report: options.packet_report.clone(),
        registry_report: options.registry_report.clone(),
    })?;''',
)

# CLI flags for one-shot setup and generation.
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    '''        /// RoM workspace to build when no server binary is supplied or found beside rom-bootstrap.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,''',
    '''        /// Mojang-generated reports/registries.json for item protocol IDs.
        #[arg(long, value_name = "PATH")]
        registry_report: Option<PathBuf>,

        /// RoM workspace to build when no server binary is supplied or found beside rom-bootstrap.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,''',
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    '''        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Build ferrum-server''',
    '''        /// Mojang-generated reports/registries.json. Standard instance paths are auto-detected.
        #[arg(long, value_name = "PATH")]
        registry_report: Option<PathBuf>,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Build ferrum-server''',
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    '''            force_generate,
            packet_report,
            workspace,''',
    '''            force_generate,
            packet_report,
            registry_report,
            workspace,''',
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    '''                force_generate,
                packet_report,
                workspace,''',
    '''                force_generate,
                packet_report,
                registry_report,
                workspace,''',
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    '''            force,
            packet_report,
            json,''',
    '''            force,
            packet_report,
            registry_report,
            json,''',
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    '''                force,
                packet_report,
            })?;''',
    '''                force,
                packet_report,
                registry_report,
            })?;''',
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    '''                    "Typed packets: {} / full packet catalog: {} / data version: {} / sections: {}+{} / registries: {} / entries: {} / source resources: {}",
                    report.packet_count,
                    report.packet_catalog_count,
                    report.world_data_version,''',
    '''                    "Typed packets: {} / full packet catalog: {} / items: {} / data version: {} / sections: {}+{} / registries: {} / entries: {} / source resources: {}",
                    report.packet_count,
                    report.packet_catalog_count,
                    report.item_count,
                    report.world_data_version,''',
)

# Extract and cache the palette alongside packet metadata.
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''    packet_report::read_packet_report, verify_file, write_json,''',
    '''    packet_report::read_packet_report, registry_report::read_item_registry_report, verify_file,
    write_json,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''    RomPackPacket, RomPackRegistry, RomPackResource, RomPackSource, RomPackSummary, RomPackWorld,
    read_rompack, sha256_hex, write_rompack,''',
    '''    RomPackItem, RomPackPacket, RomPackRegistry, RomPackResource, RomPackSource, RomPackSummary,
    RomPackWorld, read_rompack, sha256_hex, write_rompack,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''    pub packet_report: Option<PathBuf>,
}''',
    '''    pub packet_report: Option<PathBuf>,
    /// Optional Mojang-generated reports/registries.json for static item IDs.
    pub registry_report: Option<PathBuf>,
}''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub world_data_version: i32,''',
    '''    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub item_count: usize,
    pub world_data_version: i32,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''    pub packet_catalog_count: usize,
    pub registry_count: usize,''',
    '''    pub packet_catalog_count: usize,
    #[serde(default)]
    pub item_count: usize,
    pub registry_count: usize,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''    let packet_catalog = resolve_packet_catalog(&instance, options.packet_report.as_deref())?;
    let packets = typed_packet_inventory(&packet_catalog)?;''',
    '''    let packet_catalog = resolve_packet_catalog(&instance, options.packet_report.as_deref())?;
    let items = resolve_item_registry(&instance, options.registry_report.as_deref())?;
    let packets = typed_packet_inventory(&packet_catalog)?;''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''        packet_catalog: packet_catalog.entries().to_vec(),
        world,
        registries,''',
    '''        packet_catalog: packet_catalog.entries().to_vec(),
        world,
        items,
        registries,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        registry_count: summary.registry_count,''',
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        item_count: summary.item_count,
        registry_count: summary.registry_count,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        world_data_version: pack.world.data_version,''',
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        item_count: summary.item_count,
        world_data_version: pack.world.data_version,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''        && summary.packet_catalog_count == record.packet_catalog_count
        && summary.registry_count == record.registry_count''',
    '''        && summary.packet_catalog_count == record.packet_catalog_count
        && summary.item_count == record.item_count
        && summary.registry_count == record.registry_count''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        world_data_version: pack.world.data_version,''',
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        item_count: summary.item_count,
        world_data_version: pack.world.data_version,''',
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    '''fn builtin_packet_catalog() -> Result<PacketCatalog> {''',
    '''fn resolve_item_registry(instance: &Path, requested: Option<&Path>) -> Result<Vec<RomPackItem>> {
    if let Some(path) = requested {
        return read_item_registry_report(path);
    }
    for candidate in [
        instance.join("generated/reports/registries.json"),
        instance.join("generated-reports/reports/registries.json"),
        instance.join("reports/registries.json"),
    ] {
        if candidate.is_file() {
            return read_item_registry_report(&candidate);
        }
    }
    Ok(Vec::new())
}

fn builtin_packet_catalog() -> Result<PacketCatalog> {''',
)

# Setup test fixtures need the new provenance counter.
replace_once(
    "crates/rom-bootstrap/src/setup.rs",
    '''            packet_catalog_count: 1,
            registry_count: 1,''',
    '''            packet_catalog_count: 1,
            item_count: 1,
            registry_count: 1,''',
)
replace_once(
    "crates/rom-bootstrap/src/setup.rs",
    '''            packet_catalog_count: 1,
            registry_count: 1,''',
    '''            packet_catalog_count: 1,
            item_count: 1,
            registry_count: 1,''',
)
