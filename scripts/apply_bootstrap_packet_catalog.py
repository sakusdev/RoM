from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


# Register the report parser module.
path = Path("crates/rom-bootstrap/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(text, "mod extract;\nmod setup;", "mod extract;\nmod packet_report;\nmod setup;", "packet report module")
path.write_text(text, encoding="utf-8")

# Wire report selection and full catalog into extraction.
path = Path("crates/rom-bootstrap/src/extract.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''use super::{
    BOOTSTRAP_SCHEMA_VERSION, BootstrapManifest, BootstrapStage, absolute_path, eula_is_accepted,
    verify_file, write_json,
};''',
    '''use super::{
    BOOTSTRAP_SCHEMA_VERSION, BootstrapManifest, BootstrapStage, absolute_path, eula_is_accepted,
    packet_report::read_packet_report, verify_file, write_json,
};''',
    "packet report import",
)
text = replace_once(
    text,
    '''use ferrum_rompack::{
    ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackMetadata,
    RomPackPacket, RomPackRegistry, RomPackResource, RomPackSource, RomPackSummary, RomPackWorld,
    read_rompack, sha256_hex, write_rompack,
};''',
    '''use ferrum_protocol::{PacketCatalog, PacketDescriptor, PacketKind, canonical_packet_name};
use ferrum_rompack::{
    ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackMetadata,
    RomPackPacket, RomPackRegistry, RomPackResource, RomPackSource, RomPackSummary, RomPackWorld,
    read_rompack, sha256_hex, write_rompack,
};''',
    "protocol catalog imports",
)
text = replace_once(
    text,
    '''pub struct GenerateOptions {
    pub instance: PathBuf,
    pub force: bool,
}''',
    '''pub struct GenerateOptions {
    pub instance: PathBuf,
    pub force: bool,
    /// Optional Mojang-generated reports/packets.json. When omitted, standard
    /// instance locations are checked before falling back to the built-in core.
    pub packet_report: Option<PathBuf>,
}''',
    "generate report option",
)
text = replace_once(
    text,
    '''    pub packet_count: usize,
    pub world_data_version: i32,''',
    '''    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub world_data_version: i32,''',
    "generate catalog count",
)
text = replace_once(
    text,
    '''    #[serde(default)]
    pub packet_count: usize,
    pub registry_count: usize,''',
    '''    #[serde(default)]
    pub packet_count: usize,
    #[serde(default)]
    pub packet_catalog_count: usize,
    pub registry_count: usize,''',
    "pack record catalog count",
)
text = replace_once(
    text,
    '''    let game_jar = resolve_game_jar(&official_jar)?;
    let (registries, resources) = extract_registry_inventory(&game_jar.bytes)?;
    let packets = builtin_packet_inventory()?;
    let world = builtin_world_metadata();''',
    '''    let game_jar = resolve_game_jar(&official_jar)?;
    let (registries, resources) = extract_registry_inventory(&game_jar.bytes)?;
    let packet_catalog = resolve_packet_catalog(&instance, options.packet_report.as_deref())?;
    let packets = typed_packet_inventory(&packet_catalog)?;
    let world = builtin_world_metadata();''',
    "catalog extraction",
)
text = replace_once(
    text,
    '''        packets,
        world,
        registries,
        resources,''',
    '''        packets,
        packet_catalog: packet_catalog.entries().to_vec(),
        world,
        registries,
        resources,''',
    "rompack catalog storage",
)
text = replace_once(
    text,
    '''        packet_count: summary.packet_count,
        registry_count: summary.registry_count,''',
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        registry_count: summary.registry_count,''',
    "pack record catalog count value",
)
text = replace_once(
    text,
    '''        packet_count: summary.packet_count,
        world_data_version: pack.world.data_version,''',
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        world_data_version: pack.world.data_version,''',
    "new report catalog count",
)
text = replace_once(
    text,
    '''        && summary.packet_count == record.packet_count
        && summary.registry_count == record.registry_count''',
    '''        && summary.packet_count == record.packet_count
        && summary.packet_catalog_count == record.packet_catalog_count
        && summary.registry_count == record.registry_count''',
    "cached catalog count validation",
)
# report_from_existing contains the same packet_count line after the first replacement.
text = replace_once(
    text,
    '''        packet_count: summary.packet_count,
        world_data_version: pack.world.data_version,''',
    '''        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        world_data_version: pack.world.data_version,''',
    "existing report catalog count",
)
old_inventory = '''fn builtin_packet_inventory() -> Result<Vec<RomPackPacket>> {
    let profile = version_26_1_2::protocol_profile()
        .context("cannot build the built-in 26.1.2 packet table")?;
    Ok(profile
        .packets()
        .iter()
        .map(|(kind, id)| RomPackPacket { kind, id })
        .collect())
}
'''
new_inventory = '''fn resolve_packet_catalog(instance: &Path, requested: Option<&Path>) -> Result<PacketCatalog> {
    if let Some(path) = requested {
        return read_packet_report(path);
    }
    for candidate in [
        instance.join("generated/reports/packets.json"),
        instance.join("generated-reports/reports/packets.json"),
        instance.join("reports/packets.json"),
    ] {
        if candidate.is_file() {
            return read_packet_report(&candidate);
        }
    }
    builtin_packet_catalog()
}

fn builtin_packet_catalog() -> Result<PacketCatalog> {
    let profile = version_26_1_2::protocol_profile()
        .context("cannot build the built-in 26.1.2 packet table")?;
    let entries = profile
        .packets()
        .iter()
        .map(|(kind, id)| {
            PacketDescriptor::new(
                kind.phase(),
                kind.direction(),
                canonical_packet_name(kind),
                id,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    PacketCatalog::new(entries).context("cannot build built-in packet catalog")
}

fn typed_packet_inventory(catalog: &PacketCatalog) -> Result<Vec<RomPackPacket>> {
    let table = catalog
        .typed_table()
        .context("cannot derive typed packet inventory from packet catalog")?;
    Ok(PacketKind::ALL
        .iter()
        .filter_map(|kind| table.id(*kind).map(|id| RomPackPacket { kind: *kind, id }))
        .collect())
}

fn builtin_packet_inventory() -> Result<Vec<RomPackPacket>> {
    typed_packet_inventory(&builtin_packet_catalog()?)
}
'''
text = replace_once(text, old_inventory, new_inventory, "catalog inventory helpers")
text = replace_once(
    text,
    '''    let expected_packets = builtin_packet_inventory()?;
    if packets != expected_packets {
        bail!(
            "generated packet table does not match the built-in 26.1.2 profile: expected {} records, got {}",
            expected_packets.len(),
            packets.len()
        );
    }''',
    '''    let expected_packets = builtin_packet_inventory()?;
    let actual_packets: BTreeMap<_, _> = packets
        .iter()
        .map(|packet| (packet.kind, packet.id))
        .collect();
    for expected in expected_packets {
        match actual_packets.get(&expected.kind) {
            Some(actual) if *actual == expected.id => {}
            Some(actual) => bail!(
                "generated core packet {:?} ID {} does not match built-in ID {}",
                expected.kind,
                actual,
                expected.id
            ),
            None => bail!("generated packet catalog is missing core packet {:?}", expected.kind),
        }
    }''',
    "core subset packet validation",
)
path.write_text(text, encoding="utf-8")

# Add CLI surface and pass packet report through setup/generate.
path = Path("crates/rom-bootstrap/src/main.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''        /// RoM workspace to build when no server binary is supplied or found beside rom-bootstrap.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,''',
    '''        /// Mojang-generated reports/packets.json for the selected version.
        #[arg(long, value_name = "PATH")]
        packet_report: Option<PathBuf>,

        /// RoM workspace to build when no server binary is supplied or found beside rom-bootstrap.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,''',
    "setup packet report CLI",
)
text = replace_once(
    text,
    '''        /// Regenerate the pack even when the recorded pack is already valid.
        #[arg(long)]
        force: bool,

        /// Print the result as JSON.''',
    '''        /// Regenerate the pack even when the recorded pack is already valid.
        #[arg(long)]
        force: bool,

        /// Mojang-generated reports/packets.json. Standard instance paths are auto-detected.
        #[arg(long, value_name = "PATH")]
        packet_report: Option<PathBuf>,

        /// Print the result as JSON.''',
    "generate packet report CLI",
)
text = replace_once(
    text,
    '''            force_generate,
            workspace,
            server_binary,''',
    '''            force_generate,
            packet_report,
            workspace,
            server_binary,''',
    "setup match packet report",
)
text = replace_once(
    text,
    '''                force_generate,
                workspace,
                server_binary,''',
    '''                force_generate,
                packet_report,
                workspace,
                server_binary,''',
    "setup options packet report",
)
text = replace_once(
    text,
    '''        Command::Generate {
            instance,
            force,
            json,
        } => {
            let report = generate_version_pack(&GenerateOptions { instance, force })?;''',
    '''        Command::Generate {
            instance,
            force,
            packet_report,
            json,
        } => {
            let report = generate_version_pack(&GenerateOptions {
                instance,
                force,
                packet_report,
            })?;''',
    "generate options packet report",
)
text = replace_once(
    text,
    '''                    "Packets: {} / data version: {} / sections: {}+{} / registries: {} / entries: {} / source resources: {}",
                    report.packet_count,
                    report.world_data_version,''',
    '''                    "Typed packets: {} / full packet catalog: {} / data version: {} / sections: {}+{} / registries: {} / entries: {} / source resources: {}",
                    report.packet_count,
                    report.packet_catalog_count,
                    report.world_data_version,''',
    "generate catalog output",
)
path.write_text(text, encoding="utf-8")

# Setup passes packet-report through to generation and tests record its count.
path = Path("crates/rom-bootstrap/src/setup.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''    pub force_generate: bool,
    pub workspace: PathBuf,''',
    '''    pub force_generate: bool,
    pub packet_report: Option<PathBuf>,
    pub workspace: PathBuf,''',
    "setup option packet report",
)
text = replace_once(
    text,
    '''        instance: options.instance.clone(),
        force: options.force_generate,
    })?;''',
    '''        instance: options.instance.clone(),
        force: options.force_generate,
        packet_report: options.packet_report.clone(),
    })?;''',
    "setup generate packet report",
)
text = text.replace(
    '''            packet_count: 1,
            registry_count: 1,''',
    '''            packet_count: 1,
            packet_catalog_count: 1,
            registry_count: 1,''',
)
path.write_text(text, encoding="utf-8")
