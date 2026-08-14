use anyhow::{Context, Result, bail};
use rom_protocol::{PacketCatalog, PacketDescriptor, PacketKind, PacketTable};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

pub const ROMPACK_SCHEMA_VERSION: u32 = 8;
const ROMPACK_MAGIC: &[u8; 8] = b"ROMPACK\0";
const HEADER_BYTES: usize = ROMPACK_MAGIC.len() + 4 + 8;
const TRAILER_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RomPackLimits {
    pub max_file_bytes: u64,
    pub max_json_bytes: u64,
    pub max_packets: usize,
    pub max_items: usize,
    pub max_entity_types: usize,
    pub max_data_components: usize,
    pub max_sections: usize,
    pub max_registries: usize,
    pub max_entries_per_registry: usize,
    pub max_resources: usize,
    pub max_resource_bytes: u64,
    pub max_identifier_bytes: usize,
    pub max_path_bytes: usize,
}

impl Default for RomPackLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 64 * 1024 * 1024,
            max_json_bytes: 32 * 1024 * 1024,
            max_packets: 4_096,
            max_items: 100_000,
            max_entity_types: 100_000,
            max_data_components: 100_000,
            max_sections: 1_024,
            max_registries: 1_024,
            max_entries_per_registry: 1_000_000,
            max_resources: 1_000_000,
            max_resource_bytes: 64 * 1024 * 1024,
            max_identifier_bytes: 512,
            max_path_bytes: 2_048,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPack {
    pub metadata: RomPackMetadata,
    /// Typed packet IDs understood by the current native runtime.
    pub packets: Vec<RomPackPacket>,
    /// Complete generated packet inventory, including packets not implemented yet.
    pub packet_catalog: Vec<PacketDescriptor>,
    pub world: RomPackWorld,
    /// Static item registry IDs used by Play-state ItemStack codecs.
    pub items: Vec<RomPackItem>,
    /// Static entity-type IDs used by Play-state entity replication codecs.
    pub entity_types: Vec<RomPackEntityType>,
    /// Static data-component-type IDs used by version-aware ItemStack codecs.
    pub data_components: Vec<RomPackDataComponent>,
    pub registries: Vec<RomPackRegistry>,
    pub resources: Vec<RomPackResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackMetadata {
    pub schema_version: u32,
    pub minecraft_version: String,
    pub protocol: i32,
    pub patch_set: String,
    pub extractor: String,
    pub source: RomPackSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackSource {
    pub official_server_sha1: String,
    pub official_server_size: u64,
    pub game_jar_path: String,
    pub game_jar_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackWorld {
    pub data_version: i32,
    pub overworld_min_section_y: i32,
    pub overworld_section_count: usize,
    pub dimension: String,
    pub dimension_type_id: i32,
    pub sea_level: i32,
    pub floor_y: i32,
    pub spawn_x: i32,
    pub spawn_z: i32,
    pub block_states: RomPackBlockStates,
    pub biomes: RomPackBiomes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackBlockStates {
    pub air: u32,
    pub stone: u32,
    pub grass: u32,
    pub dirt: u32,
    pub bedrock: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackBiomes {
    pub plains: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackPacket {
    pub kind: PacketKind,
    pub id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackItem {
    pub item: String,
    pub protocol_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackEntityType {
    pub entity_type: String,
    pub protocol_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackDataComponent {
    pub component: String,
    pub protocol_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackRegistry {
    pub id: String,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomPackResource {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RomPackSummary {
    pub path: PathBuf,
    pub sha256: String,
    pub size: u64,
    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub item_count: usize,
    pub data_component_count: usize,
    pub registry_count: usize,
    pub registry_entry_count: usize,
    pub resource_count: usize,
}

pub fn encode_rompack(pack: &RomPack, limits: RomPackLimits) -> Result<Vec<u8>> {
    validate_rompack(pack, limits)?;
    let json = serde_json::to_vec(pack).context("cannot serialize RoM version pack")?;
    if json.len() as u64 > limits.max_json_bytes {
        bail!("RoM version-pack JSON exceeds the configured limit");
    }

    let total = HEADER_BYTES
        .checked_add(json.len())
        .and_then(|value| value.checked_add(TRAILER_BYTES))
        .context("RoM version-pack size overflow")?;
    if total as u64 > limits.max_file_bytes {
        bail!("RoM version pack exceeds the configured file limit");
    }

    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(ROMPACK_MAGIC);
    bytes.extend_from_slice(&ROMPACK_SCHEMA_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&json);
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

pub fn decode_rompack(bytes: &[u8], limits: RomPackLimits) -> Result<RomPack> {
    if bytes.len() as u64 > limits.max_file_bytes {
        bail!("RoM version pack exceeds the configured file limit");
    }
    if bytes.len() < HEADER_BYTES + TRAILER_BYTES {
        bail!("RoM version pack is truncated");
    }
    if &bytes[..ROMPACK_MAGIC.len()] != ROMPACK_MAGIC {
        bail!("invalid RoM version-pack magic");
    }

    let schema_offset = ROMPACK_MAGIC.len();
    let schema = u32::from_le_bytes(
        bytes[schema_offset..schema_offset + 4]
            .try_into()
            .expect("fixed schema slice"),
    );
    if schema != ROMPACK_SCHEMA_VERSION {
        bail!("unsupported RoM version-pack schema {schema}");
    }

    let length_offset = schema_offset + 4;
    let json_length = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .expect("fixed length slice"),
    );
    if json_length > limits.max_json_bytes {
        bail!("RoM version-pack JSON exceeds the configured limit");
    }
    let json_length = usize::try_from(json_length).context("version-pack JSON length overflow")?;
    let expected = HEADER_BYTES
        .checked_add(json_length)
        .and_then(|value| value.checked_add(TRAILER_BYTES))
        .context("RoM version-pack size overflow")?;
    if bytes.len() != expected {
        bail!("RoM version-pack length does not match its header");
    }

    let payload_end = HEADER_BYTES + json_length;
    let expected_digest = &bytes[payload_end..];
    let actual_digest = Sha256::digest(&bytes[..payload_end]);
    if expected_digest != &actual_digest[..] {
        bail!("RoM version-pack integrity digest mismatch");
    }

    let pack: RomPack = serde_json::from_slice(&bytes[HEADER_BYTES..payload_end])
        .context("cannot parse RoM version-pack JSON")?;
    validate_rompack(&pack, limits)?;
    Ok(pack)
}

pub fn write_rompack(path: impl AsRef<Path>, pack: &RomPack) -> Result<RomPackSummary> {
    write_rompack_with_limits(path, pack, RomPackLimits::default())
}

pub fn write_rompack_with_limits(
    path: impl AsRef<Path>,
    pack: &RomPack,
    limits: RomPackLimits,
) -> Result<RomPackSummary> {
    let path = path.as_ref();
    let bytes = encode_rompack(pack, limits)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let temporary = path.with_extension("rompack.part");
    let _ = fs::remove_file(&temporary);
    let mut file = File::create(&temporary)
        .with_context(|| format!("cannot create {}", temporary.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    file.sync_all()
        .with_context(|| format!("cannot sync {}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("cannot replace {}", path.display()))?;
    }
    fs::rename(&temporary, path)
        .with_context(|| format!("cannot move version pack into {}", path.display()))?;
    Ok(summary(path.to_path_buf(), &bytes, pack))
}

pub fn read_rompack(path: impl AsRef<Path>) -> Result<(RomPack, RomPackSummary)> {
    read_rompack_with_limits(path, RomPackLimits::default())
}

pub fn read_rompack_with_limits(
    path: impl AsRef<Path>,
    limits: RomPackLimits,
) -> Result<(RomPack, RomPackSummary)> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).with_context(|| format!("cannot stat {}", path.display()))?;
    if metadata.len() > limits.max_file_bytes {
        bail!("RoM version pack exceeds the configured file limit");
    }
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let pack = decode_rompack(&bytes, limits)?;
    let summary = summary(path.to_path_buf(), &bytes, &pack);
    Ok((pack, summary))
}

pub fn validate_rompack(pack: &RomPack, limits: RomPackLimits) -> Result<()> {
    let metadata = &pack.metadata;
    if metadata.schema_version != ROMPACK_SCHEMA_VERSION {
        bail!(
            "version-pack metadata schema {} does not match container schema {}",
            metadata.schema_version,
            ROMPACK_SCHEMA_VERSION
        );
    }
    validate_component("Minecraft version", &metadata.minecraft_version, 64)?;
    if metadata.protocol < 0 {
        bail!("version-pack protocol cannot be negative");
    }
    validate_component(
        "patch set",
        &metadata.patch_set,
        limits.max_identifier_bytes,
    )?;
    validate_component(
        "extractor",
        &metadata.extractor,
        limits.max_identifier_bytes,
    )?;
    validate_hex(
        "official server SHA-1",
        &metadata.source.official_server_sha1,
        40,
    )?;
    if metadata.source.official_server_size == 0 {
        bail!("official server size cannot be zero");
    }
    validate_safe_relative_path(
        "game JAR path",
        &metadata.source.game_jar_path,
        limits.max_path_bytes,
    )?;
    validate_hex("game JAR SHA-256", &metadata.source.game_jar_sha256, 64)?;

    let world = &pack.world;
    if world.data_version < 0 {
        bail!("world data version cannot be negative");
    }
    if world.overworld_section_count == 0 || world.overworld_section_count > limits.max_sections {
        bail!("overworld section count is outside the configured range");
    }
    validate_resource_location(
        "world dimension",
        &world.dimension,
        limits.max_identifier_bytes,
    )?;
    if world.dimension_type_id < 0 {
        bail!("dimension type ID cannot be negative");
    }
    let section_count = i32::try_from(world.overworld_section_count)
        .context("overworld section count exceeds i32")?;
    let section_end = world
        .overworld_min_section_y
        .checked_add(section_count)
        .context("overworld section range overflow")?;
    let min_block_y = world
        .overworld_min_section_y
        .checked_mul(16)
        .context("overworld minimum block y overflow")?;
    let max_block_y = section_end
        .checked_mul(16)
        .and_then(|value| value.checked_sub(1))
        .context("overworld maximum block y overflow")?;
    let floor_bottom = world
        .floor_y
        .checked_sub(3)
        .context("flat-world floor range overflow")?;
    let player_spawn_y = world
        .floor_y
        .checked_add(2)
        .context("flat-world player spawn overflow")?;
    for (label, value) in [
        ("sea level", world.sea_level),
        ("flat-world floor bottom", floor_bottom),
        ("flat-world player spawn", player_spawn_y),
    ] {
        if !(min_block_y..=max_block_y).contains(&value) {
            bail!("{label} {value} is outside world height {min_block_y}..={max_block_y}");
        }
    }
    let block_states = [
        world.block_states.air,
        world.block_states.stone,
        world.block_states.grass,
        world.block_states.dirt,
        world.block_states.bedrock,
    ];
    if block_states.into_iter().collect::<BTreeSet<_>>().len() != block_states.len() {
        bail!("required flat-world block-state IDs must be distinct");
    }

    if pack.packets.is_empty() {
        bail!("version pack does not contain a typed packet table");
    }
    if pack.packets.len() > limits.max_packets {
        bail!("version pack contains too many typed packet records");
    }
    if pack.packet_catalog.is_empty() {
        bail!("version pack does not contain a generated packet catalog");
    }
    if pack.packet_catalog.len() > limits.max_packets {
        bail!("version pack contains too many packet catalog records");
    }
    let catalog = PacketCatalog::new(pack.packet_catalog.clone())
        .context("invalid generated packet catalog")?;
    if catalog.entries() != pack.packet_catalog.as_slice() {
        bail!("version-pack packet catalog must be canonically sorted");
    }
    let catalog_table = catalog
        .typed_table()
        .context("cannot derive typed packet table from generated catalog")?;

    let mut previous_packet = None;
    let mut packet_table = PacketTable::new();
    for packet in &pack.packets {
        if previous_packet.is_some_and(|previous| previous >= packet.kind) {
            bail!("version-pack packets must be strictly sorted and unique");
        }
        previous_packet = Some(packet.kind);
        packet_table
            .insert(packet.kind, packet.id)
            .with_context(|| format!("invalid packet record {:?}", packet.kind))?;
        if catalog_table.id(packet.kind) != Some(packet.id) {
            bail!(
                "typed packet {:?} ID {} does not match the generated packet catalog",
                packet.kind,
                packet.id
            );
        }
    }

    if pack.items.len() > limits.max_items {
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
        if previous_entity_type.is_some_and(|previous| previous >= entity_type.entity_type.as_str())
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
            bail!(
                "duplicate entity type protocol ID {}",
                entity_type.protocol_id
            );
        }
    }
    if !pack.entity_types.is_empty()
        && !pack
            .entity_types
            .iter()
            .any(|entity_type| entity_type.entity_type == "minecraft:player")
    {
        bail!("non-empty version-pack entity-type registry is missing minecraft:player");
    }

    if pack.data_components.len() > limits.max_data_components {
        bail!("version pack contains too many data-component registry records");
    }
    let mut previous_component: Option<&str> = None;
    let mut component_protocol_ids = BTreeSet::new();
    for component in &pack.data_components {
        validate_resource_location(
            "data component ID",
            &component.component,
            limits.max_identifier_bytes,
        )?;
        if previous_component.is_some_and(|previous| previous >= component.component.as_str()) {
            bail!("version-pack data components must be strictly sorted and unique");
        }
        previous_component = Some(&component.component);
        if component.protocol_id < 0 {
            bail!(
                "data component {} protocol ID cannot be negative",
                component.component
            );
        }
        if !component_protocol_ids.insert(component.protocol_id) {
            bail!(
                "duplicate data component protocol ID {}",
                component.protocol_id
            );
        }
    }

    if pack.registries.len() > limits.max_registries {
        bail!("version pack contains too many registries");
    }
    let mut previous_registry: Option<&str> = None;
    for registry in &pack.registries {
        validate_resource_location("registry ID", &registry.id, limits.max_identifier_bytes)?;
        if previous_registry.is_some_and(|previous| previous >= registry.id.as_str()) {
            bail!("version-pack registries must be strictly sorted and unique");
        }
        previous_registry = Some(&registry.id);
        if registry.entries.len() > limits.max_entries_per_registry {
            bail!("registry {} contains too many entries", registry.id);
        }
        let mut previous_entry: Option<&str> = None;
        for entry in &registry.entries {
            validate_resource_location("registry entry", entry, limits.max_identifier_bytes)?;
            if previous_entry.is_some_and(|previous| previous >= entry.as_str()) {
                bail!(
                    "registry {} entries must be strictly sorted and unique",
                    registry.id
                );
            }
            previous_entry = Some(entry);
        }
    }

    let dimension_registry = pack
        .registries
        .iter()
        .find(|registry| registry.id == "minecraft:dimension_type")
        .context("version pack is missing minecraft:dimension_type")?;
    let dimension_type_id =
        usize::try_from(world.dimension_type_id).context("dimension type ID exceeds usize")?;
    if dimension_type_id >= dimension_registry.entries.len() {
        bail!(
            "dimension type ID {} exceeds registry size {}",
            world.dimension_type_id,
            dimension_registry.entries.len()
        );
    }

    if pack.resources.len() > limits.max_resources {
        bail!("version pack contains too many source resources");
    }
    let mut previous_resource: Option<&str> = None;
    for resource in &pack.resources {
        validate_safe_relative_path("resource path", &resource.path, limits.max_path_bytes)?;
        if previous_resource.is_some_and(|previous| previous >= resource.path.as_str()) {
            bail!("version-pack resources must be strictly sorted and unique");
        }
        previous_resource = Some(&resource.path);
        if resource.size > limits.max_resource_bytes {
            bail!(
                "resource {} exceeds the configured size limit",
                resource.path
            );
        }
        validate_hex("resource SHA-256", &resource.sha256, 64)?;
    }
    Ok(())
}

fn summary(path: PathBuf, bytes: &[u8], pack: &RomPack) -> RomPackSummary {
    RomPackSummary {
        path,
        sha256: sha256_hex(bytes),
        size: bytes.len() as u64,
        packet_count: pack.packets.len(),
        packet_catalog_count: pack.packet_catalog.len(),
        item_count: pack.items.len(),
        data_component_count: pack.data_components.len(),
        registry_count: pack.registries.len(),
        registry_entry_count: pack
            .registries
            .iter()
            .map(|registry| registry.entries.len())
            .sum(),
        resource_count: pack.resources.len(),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_component(label: &str, value: &str, max_bytes: usize) -> Result<()> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        bail!("invalid {label}");
    }
    Ok(())
}

fn validate_resource_location(label: &str, value: &str, max_bytes: usize) -> Result<()> {
    validate_component(label, value, max_bytes)?;
    let Some((namespace, path)) = value.split_once(':') else {
        bail!("{label} must be a namespaced resource location");
    };
    if namespace.is_empty()
        || path.is_empty()
        || !namespace.bytes().all(is_resource_byte)
        || !path.bytes().all(is_resource_path_byte)
        || path.starts_with('/')
        || path.ends_with('/')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        bail!("invalid {label}: {value}");
    }
    Ok(())
}

fn validate_safe_relative_path(label: &str, value: &str, max_bytes: usize) -> Result<()> {
    validate_component(label, value, max_bytes)?;
    if value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        bail!("invalid {label}: {value}");
    }
    Ok(())
}

fn validate_hex(label: &str, value: &str, expected_length: usize) -> Result<()> {
    if value.len() != expected_length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid {label}");
    }
    Ok(())
}

const fn is_resource_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
}

const fn is_resource_path_byte(byte: u8) -> bool {
    is_resource_byte(byte) || byte == b'/'
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_pack() -> RomPack {
        RomPack {
            metadata: RomPackMetadata {
                schema_version: ROMPACK_SCHEMA_VERSION,
                minecraft_version: "26.1.2".to_owned(),
                protocol: 775,
                patch_set: "builtin:26.1.2".to_owned(),
                extractor: "rom-bootstrap/0.1.0".to_owned(),
                source: RomPackSource {
                    official_server_sha1: "11".repeat(20),
                    official_server_size: 123,
                    game_jar_path: "META-INF/versions/server.jar".to_owned(),
                    game_jar_sha256: "22".repeat(32),
                },
            },
            world: RomPackWorld {
                data_version: 4_790,
                overworld_min_section_y: -4,
                overworld_section_count: 24,
                dimension: "minecraft:overworld".to_owned(),
                dimension_type_id: 0,
                sea_level: 63,
                floor_y: 63,
                spawn_x: 0,
                spawn_z: 0,
                block_states: RomPackBlockStates {
                    air: 0,
                    stone: 1,
                    grass: 9,
                    dirt: 10,
                    bedrock: 85,
                },
                biomes: RomPackBiomes { plains: 40 },
            },
            packets: vec![
                RomPackPacket {
                    kind: PacketKind::Handshake,
                    id: 0,
                },
                RomPackPacket {
                    kind: PacketKind::StatusRequest,
                    id: 0,
                },
                RomPackPacket {
                    kind: PacketKind::StatusResponse,
                    id: 0,
                },
            ],
            items: vec![
                RomPackItem {
                    item: "minecraft:air".to_owned(),
                    protocol_id: 0,
                },
                RomPackItem {
                    item: "minecraft:stone".to_owned(),
                    protocol_id: 1,
                },
            ],
            entity_types: vec![
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
                PacketDescriptor::new(
                    rom_protocol::ProtocolPhase::Handshake,
                    rom_protocol::PacketDirection::Serverbound,
                    "minecraft:intention",
                    0,
                )
                .unwrap(),
                PacketDescriptor::new(
                    rom_protocol::ProtocolPhase::Status,
                    rom_protocol::PacketDirection::Serverbound,
                    "minecraft:status_request",
                    0,
                )
                .unwrap(),
                PacketDescriptor::new(
                    rom_protocol::ProtocolPhase::Status,
                    rom_protocol::PacketDirection::Clientbound,
                    "minecraft:status_response",
                    0,
                )
                .unwrap(),
            ],
            registries: vec![
                RomPackRegistry {
                    id: "minecraft:dimension_type".to_owned(),
                    entries: vec![
                        "minecraft:overworld".to_owned(),
                        "minecraft:the_nether".to_owned(),
                    ],
                },
                RomPackRegistry {
                    id: "minecraft:worldgen/biome".to_owned(),
                    entries: vec!["minecraft:forest".to_owned(), "minecraft:plains".to_owned()],
                },
            ],
            resources: vec![RomPackResource {
                path: "data/minecraft/worldgen/biome/plains.json".to_owned(),
                size: 10,
                sha256: "33".repeat(32),
            }],
        }
    }

    #[test]
    fn encoding_is_deterministic_and_round_trips() {
        let pack = sample_pack();
        let first = encode_rompack(&pack, RomPackLimits::default()).unwrap();
        let second = encode_rompack(&pack, RomPackLimits::default()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            decode_rompack(&first, RomPackLimits::default()).unwrap(),
            pack
        );
    }

    #[test]
    fn file_round_trip_reports_counts_and_digest() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("26.1.2.rompack");
        let pack = sample_pack();
        let written = write_rompack(&path, &pack).unwrap();
        let (decoded, read) = read_rompack(&path).unwrap();
        assert_eq!(decoded, pack);
        assert_eq!(written.sha256, read.sha256);
        assert_eq!(written.packet_count, 3);
        assert_eq!(written.packet_catalog_count, 3);
        assert_eq!(written.item_count, 2);
        assert_eq!(written.registry_count, 2);
        assert_eq!(written.registry_entry_count, 4);
        assert_eq!(written.resource_count, 1);
    }

    #[test]
    fn tampering_is_detected() {
        let pack = sample_pack();
        let mut bytes = encode_rompack(&pack, RomPackLimits::default()).unwrap();
        bytes[HEADER_BYTES] ^= 1;
        assert!(decode_rompack(&bytes, RomPackLimits::default()).is_err());
    }

    #[test]
    fn rejects_unsorted_or_unsafe_records() {
        let mut pack = sample_pack();
        pack.world.overworld_section_count = 0;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.world.block_states.stone = pack.world.block_states.air;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.world.dimension.clear();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.world.dimension_type_id = 9;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.world.floor_y = 400;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.packets.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.packet_catalog.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.packets = vec![
            RomPackPacket {
                kind: PacketKind::StatusRequest,
                id: 0,
            },
            RomPackPacket {
                kind: PacketKind::PingRequest,
                id: 0,
            },
        ];
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.items.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.items[1].protocol_id = pack.items[0].protocol_id;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.entity_types.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.entity_types[1].protocol_id = pack.entity_types[0].protocol_id;
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.registries[0].entries.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.resources[0].path = "../server.jar".to_owned();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());
    }
}
