use super::{
    BOOTSTRAP_SCHEMA_VERSION, BootstrapManifest, BootstrapStage, absolute_path, eula_is_accepted,
    packet_report::read_packet_report,
    registry_report::{RegistryProtocolReport, read_registry_protocol_report},
    verify_file, write_json,
};
use anyhow::{Context, Result, bail};
use ferrum_protocol::{PacketCatalog, PacketDescriptor, PacketKind, canonical_packet_name};
use ferrum_rompack::{
    ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackMetadata,
    RomPackPacket, RomPackRegistry, RomPackResource, RomPackSource, RomPackSummary, RomPackWorld,
    read_rompack, sha256_hex, write_rompack,
};
use ferrum_version_26_1_2 as version_26_1_2;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
};
use zip::ZipArchive;

const MAX_OUTER_JAR_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_GAME_JAR_BYTES: u64 = 768 * 1024 * 1024;
const MAX_VERSIONS_LIST_BYTES: u64 = 1024 * 1024;
const MAX_RESOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_RESOURCE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 1_000_000;

const REGISTRY_SPECS: &[(&str, &str)] = &[
    ("minecraft:banner_pattern", "banner_pattern"),
    ("minecraft:cat_sound_variant", "cat_sound_variant"),
    ("minecraft:cat_variant", "cat_variant"),
    ("minecraft:chat_type", "chat_type"),
    ("minecraft:chicken_sound_variant", "chicken_sound_variant"),
    ("minecraft:chicken_variant", "chicken_variant"),
    ("minecraft:cow_sound_variant", "cow_sound_variant"),
    ("minecraft:cow_variant", "cow_variant"),
    ("minecraft:damage_type", "damage_type"),
    ("minecraft:dialog", "dialog"),
    ("minecraft:dimension_type", "dimension_type"),
    ("minecraft:enchantment", "enchantment"),
    ("minecraft:frog_variant", "frog_variant"),
    ("minecraft:instrument", "instrument"),
    ("minecraft:jukebox_song", "jukebox_song"),
    ("minecraft:painting_variant", "painting_variant"),
    ("minecraft:pig_sound_variant", "pig_sound_variant"),
    ("minecraft:pig_variant", "pig_variant"),
    ("minecraft:test_environment", "test_environment"),
    ("minecraft:test_instance", "test_instance"),
    ("minecraft:timeline", "timeline"),
    ("minecraft:trim_material", "trim_material"),
    ("minecraft:trim_pattern", "trim_pattern"),
    ("minecraft:wolf_sound_variant", "wolf_sound_variant"),
    ("minecraft:wolf_variant", "wolf_variant"),
    ("minecraft:world_clock", "world_clock"),
    ("minecraft:worldgen/biome", "worldgen/biome"),
    (
        "minecraft:zombie_nautilus_variant",
        "zombie_nautilus_variant",
    ),
];

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub instance: PathBuf,
    pub force: bool,
    /// Optional Mojang-generated reports/packets.json. When omitted, standard
    /// instance locations are checked before falling back to the built-in core.
    pub packet_report: Option<PathBuf>,
    /// Optional Mojang-generated reports/registries.json for static item IDs.
    pub registry_report: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerateReport {
    pub instance: PathBuf,
    pub minecraft_version: String,
    pub protocol: i32,
    pub version_pack: PathBuf,
    pub version_pack_sha256: String,
    pub version_pack_size: u64,
    pub game_jar_path: String,
    pub game_jar_sha256: String,
    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub item_count: usize,
    pub world_data_version: i32,
    pub overworld_min_section_y: i32,
    pub overworld_section_count: usize,
    pub world_dimension: String,
    pub dimension_type_id: i32,
    pub sea_level: i32,
    pub floor_y: i32,
    pub spawn_x: i32,
    pub spawn_z: i32,
    pub registry_count: usize,
    pub registry_entry_count: usize,
    pub resource_count: usize,
    pub reused_existing_pack: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PackRecord {
    pub local_path: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub packet_count: usize,
    #[serde(default)]
    pub packet_catalog_count: usize,
    #[serde(default)]
    pub item_count: usize,
    pub registry_count: usize,
    pub registry_entry_count: usize,
    pub resource_count: usize,
}

#[derive(Debug, Clone, Default)]
pub(super) struct VersionPackStatus {
    pub path: Option<PathBuf>,
    pub sha256: Option<String>,
    pub verified: bool,
}

pub fn generate_version_pack(options: &GenerateOptions) -> Result<GenerateReport> {
    let instance = absolute_path(&options.instance)?;
    let manifest_path = instance.join("rom-bootstrap.json");
    let mut manifest = read_bootstrap_manifest(&manifest_path)?;
    if manifest.schema_version != BOOTSTRAP_SCHEMA_VERSION {
        bail!(
            "unsupported bootstrap manifest schema {}",
            manifest.schema_version
        );
    }
    if !eula_is_accepted(&instance.join("eula.txt"))? {
        bail!("Minecraft EULA acceptance is missing; run rom-bootstrap prepare first");
    }

    let official_jar = instance.join(&manifest.source.local_path);
    if !official_jar.is_file()
        || !verify_file(&official_jar, &manifest.source.sha1, manifest.source.size)?
    {
        bail!("official server artifact is missing or failed integrity verification");
    }
    if manifest.source.size == 0 || manifest.source.size > MAX_OUTER_JAR_BYTES {
        bail!("official server artifact size is outside the extractor limit");
    }

    let relative_pack = format!(
        "versions/{}/{}.rompack",
        manifest.minecraft_version, manifest.minecraft_version
    );
    let pack_path = instance.join(&relative_pack);
    if !options.force {
        if let Some(record) = &manifest.pack {
            if record.local_path == relative_pack {
                let status = verify_version_pack_record(&instance, &manifest)?;
                if status.verified {
                    let (pack, summary) = read_rompack(&pack_path)?;
                    return Ok(report_from_existing(instance, pack, summary));
                }
            }
        }
    }

    let game_jar = resolve_game_jar(&official_jar)?;
    let (registries, resources) = extract_registry_inventory(&game_jar.bytes)?;
    let packet_catalog = resolve_packet_catalog(&instance, options.packet_report.as_deref())?;
    let protocol_registries =
        resolve_protocol_registries(&instance, options.registry_report.as_deref())?;
    let items = protocol_registries.items;
    let data_components = protocol_registries.data_components;
    let packets = typed_packet_inventory(&packet_catalog)?;
    let world = builtin_world_metadata();
    validate_against_builtin_profile(
        &manifest.minecraft_version,
        manifest.protocol,
        &manifest.source.sha1,
        &packets,
        &world,
        &registries,
    )?;

    let pack = RomPack {
        metadata: RomPackMetadata {
            schema_version: ROMPACK_SCHEMA_VERSION,
            minecraft_version: manifest.minecraft_version.clone(),
            protocol: manifest.protocol,
            patch_set: manifest.patch_set.clone(),
            extractor: format!("rom-bootstrap/{}", env!("CARGO_PKG_VERSION")),
            source: RomPackSource {
                official_server_sha1: manifest.source.sha1.clone(),
                official_server_size: manifest.source.size,
                game_jar_path: game_jar.path.clone(),
                game_jar_sha256: game_jar.sha256.clone(),
            },
        },
        packets,
        packet_catalog: packet_catalog.entries().to_vec(),
        world,
        items,
        data_components,
        registries,
        resources,
    };
    let summary = write_rompack(&pack_path, &pack)?;
    let record = PackRecord {
        local_path: relative_pack,
        sha256: summary.sha256.clone(),
        size: summary.size,
        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        item_count: summary.item_count,
        registry_count: summary.registry_count,
        registry_entry_count: summary.registry_entry_count,
        resource_count: summary.resource_count,
    };
    manifest.stage = BootstrapStage::VersionPackGenerated;
    manifest.pack = Some(record.clone());
    write_json(manifest_path, &manifest)?;
    write_json(
        instance
            .join("versions")
            .join(&manifest.minecraft_version)
            .join("rompack.json"),
        &record,
    )?;

    Ok(GenerateReport {
        instance,
        minecraft_version: manifest.minecraft_version,
        protocol: manifest.protocol,
        version_pack: summary.path,
        version_pack_sha256: summary.sha256,
        version_pack_size: summary.size,
        game_jar_path: game_jar.path,
        game_jar_sha256: game_jar.sha256,
        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        item_count: summary.item_count,
        world_data_version: pack.world.data_version,
        overworld_min_section_y: pack.world.overworld_min_section_y,
        overworld_section_count: pack.world.overworld_section_count,
        world_dimension: pack.world.dimension.clone(),
        dimension_type_id: pack.world.dimension_type_id,
        sea_level: pack.world.sea_level,
        floor_y: pack.world.floor_y,
        spawn_x: pack.world.spawn_x,
        spawn_z: pack.world.spawn_z,
        registry_count: summary.registry_count,
        registry_entry_count: summary.registry_entry_count,
        resource_count: summary.resource_count,
        reused_existing_pack: false,
    })
}

pub(super) fn verify_version_pack_record(
    instance: &Path,
    manifest: &BootstrapManifest,
) -> Result<VersionPackStatus> {
    let Some(record) = &manifest.pack else {
        return Ok(VersionPackStatus::default());
    };
    let path = instance.join(&record.local_path);
    if !path.is_file() {
        return Ok(VersionPackStatus {
            path: Some(path),
            sha256: Some(record.sha256.clone()),
            verified: false,
        });
    }
    let (pack, summary) = match read_rompack(&path) {
        Ok(value) => value,
        Err(_) => {
            return Ok(VersionPackStatus {
                path: Some(path),
                sha256: Some(record.sha256.clone()),
                verified: false,
            });
        }
    };
    let valid = summary.sha256.eq_ignore_ascii_case(&record.sha256)
        && summary.size == record.size
        && summary.packet_count == record.packet_count
        && summary.packet_catalog_count == record.packet_catalog_count
        && summary.item_count == record.item_count
        && summary.registry_count == record.registry_count
        && summary.registry_entry_count == record.registry_entry_count
        && summary.resource_count == record.resource_count
        && pack.metadata.minecraft_version == manifest.minecraft_version
        && pack.metadata.protocol == manifest.protocol
        && pack
            .metadata
            .source
            .official_server_sha1
            .eq_ignore_ascii_case(&manifest.source.sha1)
        && validate_against_builtin_profile(
            &pack.metadata.minecraft_version,
            pack.metadata.protocol,
            &pack.metadata.source.official_server_sha1,
            &pack.packets,
            &pack.world,
            &pack.registries,
        )
        .is_ok();
    Ok(VersionPackStatus {
        path: Some(path),
        sha256: Some(summary.sha256),
        verified: valid,
    })
}

fn report_from_existing(
    instance: PathBuf,
    pack: RomPack,
    summary: RomPackSummary,
) -> GenerateReport {
    GenerateReport {
        instance,
        minecraft_version: pack.metadata.minecraft_version,
        protocol: pack.metadata.protocol,
        version_pack: summary.path,
        version_pack_sha256: summary.sha256,
        version_pack_size: summary.size,
        game_jar_path: pack.metadata.source.game_jar_path,
        game_jar_sha256: pack.metadata.source.game_jar_sha256,
        packet_count: summary.packet_count,
        packet_catalog_count: summary.packet_catalog_count,
        item_count: summary.item_count,
        world_data_version: pack.world.data_version,
        overworld_min_section_y: pack.world.overworld_min_section_y,
        overworld_section_count: pack.world.overworld_section_count,
        world_dimension: pack.world.dimension.clone(),
        dimension_type_id: pack.world.dimension_type_id,
        sea_level: pack.world.sea_level,
        floor_y: pack.world.floor_y,
        spawn_x: pack.world.spawn_x,
        spawn_z: pack.world.spawn_z,
        registry_count: summary.registry_count,
        registry_entry_count: summary.registry_entry_count,
        resource_count: summary.resource_count,
        reused_existing_pack: true,
    }
}

fn read_bootstrap_manifest(path: &Path) -> Result<BootstrapManifest> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("cannot parse {}", path.display()))
}

#[derive(Debug)]
struct ResolvedGameJar {
    path: String,
    sha256: String,
    bytes: Vec<u8>,
}

fn resolve_game_jar(path: &Path) -> Result<ResolvedGameJar> {
    let outer_bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    if outer_bytes.len() as u64 > MAX_OUTER_JAR_BYTES {
        bail!("official server artifact exceeds the extractor limit");
    }
    if archive_contains_registry_resources(Cursor::new(&outer_bytes))? {
        return Ok(ResolvedGameJar {
            path: "server.jar".to_owned(),
            sha256: sha256_hex(&outer_bytes),
            bytes: outer_bytes,
        });
    }

    let mut archive = ZipArchive::new(Cursor::new(&outer_bytes))
        .context("official server artifact is not a valid ZIP/JAR")?;
    ensure_archive_entry_limit(&archive)?;
    let candidate = select_embedded_game_jar(&mut archive)?;
    let bytes = read_zip_entry(&mut archive, &candidate.path, MAX_GAME_JAR_BYTES)?;
    if let Some(expected_sha256) = candidate.sha256 {
        let actual = sha256_hex(&bytes);
        if !actual.eq_ignore_ascii_case(&expected_sha256) {
            bail!("embedded game JAR SHA-256 mismatch: expected {expected_sha256}, got {actual}");
        }
    }
    ZipArchive::new(Cursor::new(&bytes)).context("embedded game JAR is not a valid ZIP/JAR")?;
    Ok(ResolvedGameJar {
        path: candidate.path,
        sha256: sha256_hex(&bytes),
        bytes,
    })
}

#[derive(Debug)]
struct GameJarCandidate {
    path: String,
    sha256: Option<String>,
}

fn select_embedded_game_jar<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<GameJarCandidate> {
    match read_versions_list(archive) {
        Ok(Some(candidate)) => Ok(candidate),
        Ok(None) => find_single_embedded_jar(archive),
        Err(list_error) => find_single_embedded_jar(archive).with_context(|| {
            format!(
                "cannot parse META-INF/versions.list and cannot select an unambiguous embedded game JAR fallback: {list_error:#}"
            )
        }),
    }
}

fn read_versions_list<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Option<GameJarCandidate>> {
    let Ok(file) = archive.by_name("META-INF/versions.list") else {
        return Ok(None);
    };
    if file.size() > MAX_VERSIONS_LIST_BYTES {
        bail!("META-INF/versions.list exceeds the extractor limit");
    }
    let mut text = String::new();
    file.take(MAX_VERSIONS_LIST_BYTES + 1)
        .read_to_string(&mut text)
        .context("cannot read META-INF/versions.list")?;
    let mut candidates = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 {
            bail!("invalid META-INF/versions.list record");
        }
        let listed_path = fields[2];
        validate_zip_path(listed_path)?;
        let path = if listed_path.starts_with("META-INF/versions/") {
            listed_path.to_owned()
        } else {
            format!("META-INF/versions/{listed_path}")
        };
        validate_zip_path(&path)?;
        if !path.ends_with(".jar") {
            bail!("invalid embedded game JAR path in versions.list: {listed_path}");
        }
        let sha256 = fields[0];
        let digest = if sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Some(sha256.to_ascii_lowercase())
        } else {
            None
        };
        candidates.push(GameJarCandidate {
            path,
            sha256: digest,
        });
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop()),
        count => bail!("versions.list contains {count} embedded game JARs; expected one"),
    }
}

fn find_single_embedded_jar<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<GameJarCandidate> {
    let mut candidates = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = file.name();
        if !file.is_dir() && name.starts_with("META-INF/versions/") && name.ends_with(".jar") {
            validate_zip_path(name)?;
            if file.size() > MAX_GAME_JAR_BYTES {
                bail!("embedded game JAR exceeds the extractor limit");
            }
            candidates.push(name.to_owned());
        }
    }
    match candidates.len() {
        1 => Ok(GameJarCandidate {
            path: candidates.pop().expect("one candidate"),
            sha256: None,
        }),
        0 => bail!(
            "official server artifact does not contain registry resources or an embedded game JAR"
        ),
        count => bail!("official server artifact contains {count} embedded game JAR candidates"),
    }
}

fn archive_contains_registry_resources<R: Read + Seek>(reader: R) -> Result<bool> {
    let mut archive =
        ZipArchive::new(reader).context("official server artifact is not a valid ZIP/JAR")?;
    ensure_archive_entry_limit(&archive)?;
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if !file.is_dir()
            && REGISTRY_SPECS.iter().any(|(_, directory)| {
                file.name()
                    .starts_with(&format!("data/minecraft/{directory}/"))
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn extract_registry_inventory(
    bytes: &[u8],
) -> Result<(Vec<RomPackRegistry>, Vec<RomPackResource>)> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).context("game JAR is not a valid ZIP/JAR")?;
    ensure_archive_entry_limit(&archive)?;
    let mut registries: BTreeMap<&str, BTreeSet<String>> = REGISTRY_SPECS
        .iter()
        .map(|(id, _)| (*id, BTreeSet::new()))
        .collect();
    let mut resources = Vec::new();
    let mut total_resource_bytes = 0_u64;

    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_owned();
        validate_zip_path(&name)?;
        let Some((registry_id, resource_id)) = registry_resource(&name)? else {
            continue;
        };
        let expected_size = file.size();
        if expected_size > MAX_RESOURCE_BYTES {
            bail!("registry resource {name} exceeds the per-resource limit");
        }
        total_resource_bytes = total_resource_bytes
            .checked_add(expected_size)
            .context("registry resource size overflow")?;
        if total_resource_bytes > MAX_TOTAL_RESOURCE_BYTES {
            bail!("registry resources exceed the total extractor limit");
        }
        let mut data = Vec::with_capacity(usize::try_from(file.size()).unwrap_or(0));
        file.take(MAX_RESOURCE_BYTES + 1)
            .read_to_end(&mut data)
            .with_context(|| format!("cannot read registry resource {name}"))?;
        if data.len() as u64 != expected_size {
            bail!("registry resource {name} length changed while reading");
        }
        serde_json::from_slice::<serde_json::Value>(&data)
            .with_context(|| format!("registry resource {name} is not valid JSON"))?;
        let inserted = registries
            .get_mut(registry_id)
            .expect("registry spec initialized")
            .insert(resource_id);
        if !inserted {
            bail!("duplicate registry resource ID derived from {name}");
        }
        resources.push(RomPackResource {
            path: name,
            size: data.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&data)),
        });
    }

    resources.sort_by(|left, right| left.path.cmp(&right.path));
    let registries = registries
        .into_iter()
        .map(|(id, entries)| {
            if entries.is_empty() {
                bail!("game JAR is missing synchronized registry {id}");
            }
            Ok(RomPackRegistry {
                id: id.to_owned(),
                entries: entries.into_iter().collect(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((registries, resources))
}

fn registry_resource(path: &str) -> Result<Option<(&'static str, String)>> {
    if !path.ends_with(".json") {
        return Ok(None);
    }
    for (registry_id, directory) in REGISTRY_SPECS {
        let prefix = format!("data/minecraft/{directory}/");
        let Some(relative) = path.strip_prefix(&prefix) else {
            continue;
        };
        let relative = relative.strip_suffix(".json").expect("JSON suffix checked");
        if relative.is_empty()
            || relative.starts_with('/')
            || relative.ends_with('/')
            || relative
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
            || !relative.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-' | b'.' | b'/')
            })
        {
            bail!("invalid registry resource path: {path}");
        }
        return Ok(Some((*registry_id, format!("minecraft:{relative}"))));
    }
    Ok(None)
}

fn resolve_packet_catalog(instance: &Path, requested: Option<&Path>) -> Result<PacketCatalog> {
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

fn resolve_protocol_registries(
    instance: &Path,
    requested: Option<&Path>,
) -> Result<RegistryProtocolReport> {
    if let Some(path) = requested {
        return read_registry_protocol_report(path);
    }
    for candidate in [
        instance.join("generated/reports/registries.json"),
        instance.join("generated-reports/reports/registries.json"),
        instance.join("reports/registries.json"),
    ] {
        if candidate.is_file() {
            return read_registry_protocol_report(&candidate);
        }
    }
    Ok(RegistryProtocolReport {
        items: Vec::new(),
        data_components: Vec::new(),
    })
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

fn builtin_world_metadata() -> RomPackWorld {
    RomPackWorld {
        data_version: version_26_1_2::WORLD_VERSION,
        overworld_min_section_y: version_26_1_2::OVERWORLD_MIN_SECTION_Y,
        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,
        dimension: version_26_1_2::OVERWORLD_DIMENSION.to_owned(),
        dimension_type_id: version_26_1_2::OVERWORLD_DIMENSION_TYPE_ID,
        sea_level: version_26_1_2::OVERWORLD_SEA_LEVEL,
        floor_y: version_26_1_2::FLAT_WORLD_FLOOR_Y,
        spawn_x: version_26_1_2::FLAT_WORLD_SPAWN_X,
        spawn_z: version_26_1_2::FLAT_WORLD_SPAWN_Z,
        block_states: RomPackBlockStates {
            air: version_26_1_2::AIR_BLOCK_STATE_ID,
            stone: version_26_1_2::STONE_BLOCK_STATE_ID,
            grass: version_26_1_2::GRASS_BLOCK_STATE_ID,
            dirt: version_26_1_2::DIRT_BLOCK_STATE_ID,
            bedrock: version_26_1_2::BEDROCK_BLOCK_STATE_ID,
        },
        biomes: RomPackBiomes {
            plains: version_26_1_2::PLAINS_BIOME_ID,
        },
    }
}

fn validate_against_builtin_profile(
    version: &str,
    protocol: i32,
    official_sha1: &str,
    packets: &[RomPackPacket],
    world: &RomPackWorld,
    registries: &[RomPackRegistry],
) -> Result<()> {
    if version != version_26_1_2::PROFILE_NAME {
        bail!("no built-in extraction validator exists for Minecraft {version}");
    }
    if protocol != version_26_1_2::PROTOCOL_VERSION {
        bail!("generated pack protocol {protocol} does not match the built-in profile");
    }
    if !official_sha1.eq_ignore_ascii_case(version_26_1_2::OFFICIAL_SERVER_SHA1) {
        bail!("official server SHA-1 does not match the built-in 26.1.2 provenance record");
    }

    let expected_world = builtin_world_metadata();
    if *world != expected_world {
        bail!("generated world metadata does not match the built-in 26.1.2 profile");
    }

    let expected_packets = builtin_packet_inventory()?;
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
            None => bail!(
                "generated packet catalog is missing core packet {:?}",
                expected.kind
            ),
        }
    }

    let expected: BTreeMap<_, _> = version_26_1_2::SYNCHRONIZED_REGISTRIES
        .iter()
        .map(|registry| {
            (
                registry.id,
                registry.entries.iter().copied().collect::<BTreeSet<_>>(),
            )
        })
        .collect();
    let actual: BTreeMap<_, _> = registries
        .iter()
        .map(|registry| {
            (
                registry.id.as_str(),
                registry
                    .entries
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect();
    if actual != expected {
        let actual_count: usize = actual.values().map(BTreeSet::len).sum();
        bail!(
            "extracted synchronized registries do not match the built-in 26.1.2 manifest: expected {} registries and {} entries, got {} registries and {} entries",
            version_26_1_2::REGISTRY_COUNT,
            version_26_1_2::REGISTRY_ENTRY_COUNT,
            actual.len(),
            actual_count
        );
    }
    Ok(())
}

fn read_zip_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    limit: u64,
) -> Result<Vec<u8>> {
    let file = archive
        .by_name(path)
        .with_context(|| format!("embedded game JAR is missing: {path}"))?;
    if file.is_dir() || file.size() == 0 || file.size() > limit {
        bail!("embedded game JAR size is outside the extractor limit");
    }
    let expected_size = file.size();
    let mut bytes = Vec::with_capacity(usize::try_from(expected_size).unwrap_or(0));
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("cannot read embedded game JAR {path}"))?;
    if bytes.len() as u64 != expected_size {
        bail!("embedded game JAR length changed while reading");
    }
    Ok(bytes)
}

fn ensure_archive_entry_limit<R: Read + Seek>(archive: &ZipArchive<R>) -> Result<()> {
    if archive.len() > MAX_ZIP_ENTRIES {
        bail!("ZIP/JAR contains too many entries");
    }
    Ok(())
}

fn validate_zip_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        bail!("unsafe ZIP/JAR entry path: {path}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use tempfile::tempdir;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    fn build_game_jar() -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (_, directory) in REGISTRY_SPECS {
                writer
                    .start_file(format!("data/minecraft/{directory}/sample.json"), options)
                    .unwrap();
                writer.write_all(br#"{"value":1}"#).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    fn build_outer_jar(game_jar: &[u8]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            let digest = sha256_hex(game_jar);
            writer
                .start_file("META-INF/versions.list", options)
                .unwrap();
            writer
                .write_all(format!("{digest}\t26.1.2\t26.1.2/server-26.1.2.jar\n").as_bytes())
                .unwrap();
            writer
                .start_file("META-INF/versions/26.1.2/server-26.1.2.jar", options)
                .unwrap();
            writer.write_all(game_jar).unwrap();
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    fn build_outer_jar_with_invalid_versions_list(game_jar: &[u8]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            writer
                .start_file("META-INF/versions.list", options)
                .unwrap();
            writer
                .write_all(b"not a valid versions.list row\n")
                .unwrap();
            writer
                .start_file("META-INF/versions/26.1.2/server-26.1.2.jar", options)
                .unwrap();
            writer.write_all(game_jar).unwrap();
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn derives_sorted_registry_and_resource_inventory() {
        let (registries, resources) = extract_registry_inventory(&build_game_jar()).unwrap();
        assert_eq!(registries.len(), REGISTRY_SPECS.len());
        assert_eq!(resources.len(), REGISTRY_SPECS.len());
        assert!(registries.windows(2).all(|pair| pair[0].id < pair[1].id));
        assert!(resources.windows(2).all(|pair| pair[0].path < pair[1].path));
        assert!(
            registries
                .iter()
                .all(|registry| { registry.entries == ["minecraft:sample".to_owned()] })
        );
    }

    #[test]
    fn resolves_and_verifies_the_embedded_game_jar() {
        let directory = tempdir().unwrap();
        let game_jar = build_game_jar();
        let outer = build_outer_jar(&game_jar);
        let path = directory.path().join("server.jar");
        fs::write(&path, outer).unwrap();
        let resolved = resolve_game_jar(&path).unwrap();
        assert_eq!(resolved.path, "META-INF/versions/26.1.2/server-26.1.2.jar");
        assert_eq!(resolved.sha256, sha256_hex(&game_jar));
        assert_eq!(resolved.bytes, game_jar);
    }

    #[test]
    fn falls_back_to_single_embedded_game_jar_when_versions_list_is_invalid() {
        let directory = tempdir().unwrap();
        let game_jar = build_game_jar();
        let outer = build_outer_jar_with_invalid_versions_list(&game_jar);
        let path = directory.path().join("server.jar");
        fs::write(&path, outer).unwrap();
        let resolved = resolve_game_jar(&path).unwrap();
        assert_eq!(resolved.path, "META-INF/versions/26.1.2/server-26.1.2.jar");
        assert_eq!(resolved.sha256, sha256_hex(&game_jar));
        assert_eq!(resolved.bytes, game_jar);
    }

    #[test]
    fn rejects_invalid_registry_json() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (_, directory) in REGISTRY_SPECS {
                writer
                    .start_file(format!("data/minecraft/{directory}/sample.json"), options)
                    .unwrap();
                if *directory == "chat_type" {
                    writer.write_all(b"not json").unwrap();
                } else {
                    writer.write_all(br#"{"value":1}"#).unwrap();
                }
            }
            writer.finish().unwrap();
        }
        assert!(extract_registry_inventory(&cursor.into_inner()).is_err());
    }
}
