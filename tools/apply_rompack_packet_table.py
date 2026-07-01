from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# PacketKind names are now part of the stable local .rompack schema.
replace_once(
    "crates/ferrum-protocol/Cargo.toml",
    "rust-version.workspace = true\n",
    "rust-version.workspace = true\n\n[dependencies]\nserde.workspace = true\n",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "use std::{\n",
    "use serde::{Deserialize, Serialize};\nuse std::{\n",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum PacketKind {\n",
    "#[derive(\n"
    "    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,\n"
    ")]\n"
    "#[serde(rename_all = \"snake_case\")]\n"
    "pub enum PacketKind {\n",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "impl PacketKind {\n    #[must_use]\n    pub const fn phase(self) -> ProtocolPhase {\n",
    "impl PacketKind {\n"
    "    pub const ALL: &'static [Self] = &[\n"
    "        Self::Handshake,\n"
    "        Self::StatusRequest,\n"
    "        Self::PingRequest,\n"
    "        Self::StatusResponse,\n"
    "        Self::PongResponse,\n"
    "        Self::LoginStart,\n"
    "        Self::LoginAcknowledged,\n"
    "        Self::LoginDisconnect,\n"
    "        Self::LoginSuccess,\n"
    "        Self::ConfigurationAcknowledged,\n"
    "        Self::ConfigurationClientInformation,\n"
    "        Self::ConfigurationDisconnect,\n"
    "        Self::RegistryData,\n"
    "        Self::FeatureFlags,\n"
    "        Self::UpdateTags,\n"
    "        Self::SelectKnownPacksRequest,\n"
    "        Self::SelectKnownPacksResponse,\n"
    "        Self::FinishConfiguration,\n"
    "        Self::PlayLogin,\n"
    "        Self::ChunkBatchStart,\n"
    "        Self::ChunkBatchFinished,\n"
    "        Self::ChunkBatchReceived,\n"
    "        Self::LevelChunkWithLight,\n"
    "        Self::SetChunkCacheCenter,\n"
    "        Self::DefaultSpawnPosition,\n"
    "        Self::PlayerPosition,\n"
    "        Self::SystemChat,\n"
    "        Self::AcceptTeleportation,\n"
    "        Self::PlayDisconnect,\n"
    "        Self::KeepAliveRequest,\n"
    "        Self::KeepAliveResponse,\n"
    "        Self::ClientTickEnd,\n"
    "        Self::MovePlayerPosition,\n"
    "        Self::MovePlayerPositionRotation,\n"
    "        Self::MovePlayerRotation,\n"
    "        Self::MovePlayerStatusOnly,\n"
    "        Self::PlayerAction,\n"
    "        Self::UseItemOn,\n"
    "        Self::BlockChangedAck,\n"
    "        Self::BlockUpdate,\n"
    "        Self::ForgetLevelChunk,\n"
    "    ];\n\n"
    "    #[must_use]\n"
    "    pub const fn phase(self) -> ProtocolPhase {\n",
)

# Add packet metadata to schema v2 and validate it through PacketTable itself.
replace_once(
    "crates/ferrum-rompack/Cargo.toml",
    "anyhow.workspace = true\n",
    "anyhow.workspace = true\nferrum-protocol = { path = \"../ferrum-protocol\" }\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "use serde::{Deserialize, Serialize};\n",
    "use ferrum_protocol::{PacketKind, PacketTable};\nuse serde::{Deserialize, Serialize};\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 1;\n",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 2;\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    pub max_file_bytes: u64,\n    pub max_json_bytes: u64,\n",
    "    pub max_file_bytes: u64,\n    pub max_json_bytes: u64,\n    pub max_packets: usize,\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "            max_file_bytes: 64 * 1024 * 1024,\n            max_json_bytes: 32 * 1024 * 1024,\n",
    "            max_file_bytes: 64 * 1024 * 1024,\n            max_json_bytes: 32 * 1024 * 1024,\n            max_packets: 4_096,\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "pub struct RomPack {\n    pub metadata: RomPackMetadata,\n    pub registries: Vec<RomPackRegistry>,\n",
    "pub struct RomPack {\n    pub metadata: RomPackMetadata,\n    pub packets: Vec<RomPackPacket>,\n    pub registries: Vec<RomPackRegistry>,\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\npub struct RomPackRegistry {\n",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n"
    "pub struct RomPackPacket {\n"
    "    pub kind: PacketKind,\n"
    "    pub id: i32,\n"
    "}\n\n"
    "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n"
    "pub struct RomPackRegistry {\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    pub size: u64,\n    pub registry_count: usize,\n",
    "    pub size: u64,\n    pub packet_count: usize,\n    pub registry_count: usize,\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    validate_hex(\"game JAR SHA-256\", &metadata.source.game_jar_sha256, 64)?;\n\n"
    "    if pack.registries.len() > limits.max_registries {\n",
    "    validate_hex(\"game JAR SHA-256\", &metadata.source.game_jar_sha256, 64)?;\n\n"
    "    if pack.packets.is_empty() {\n"
    "        bail!(\"version pack does not contain a packet table\");\n"
    "    }\n"
    "    if pack.packets.len() > limits.max_packets {\n"
    "        bail!(\"version pack contains too many packet records\");\n"
    "    }\n"
    "    let mut previous_packet = None;\n"
    "    let mut packet_table = PacketTable::new();\n"
    "    for packet in &pack.packets {\n"
    "        if previous_packet.is_some_and(|previous| previous >= packet.kind) {\n"
    "            bail!(\"version-pack packets must be strictly sorted and unique\");\n"
    "        }\n"
    "        previous_packet = Some(packet.kind);\n"
    "        packet_table\n"
    "            .insert(packet.kind, packet.id)\n"
    "            .with_context(|| format!(\"invalid packet record {:?}\", packet.kind))?;\n"
    "    }\n\n"
    "    if pack.registries.len() > limits.max_registries {\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "        size: bytes.len() as u64,\n        registry_count: pack.registries.len(),\n",
    "        size: bytes.len() as u64,\n        packet_count: pack.packets.len(),\n        registry_count: pack.registries.len(),\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "            registries: vec![RomPackRegistry {\n",
    "            packets: vec![\n"
    "                RomPackPacket {\n"
    "                    kind: PacketKind::Handshake,\n"
    "                    id: 0,\n"
    "                },\n"
    "                RomPackPacket {\n"
    "                    kind: PacketKind::StatusRequest,\n"
    "                    id: 0,\n"
    "                },\n"
    "                RomPackPacket {\n"
    "                    kind: PacketKind::StatusResponse,\n"
    "                    id: 0,\n"
    "                },\n"
    "            ],\n"
    "            registries: vec![RomPackRegistry {\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "        assert_eq!(written.registry_count, 1);\n",
    "        assert_eq!(written.packet_count, 3);\n        assert_eq!(written.registry_count, 1);\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    fn rejects_unsorted_or_unsafe_records() {\n        let mut pack = sample_pack();\n",
    "    fn rejects_unsorted_or_unsafe_records() {\n"
    "        let mut pack = sample_pack();\n"
    "        pack.packets.reverse();\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n"
    "        pack.packets = vec![\n"
    "            RomPackPacket {\n"
    "                kind: PacketKind::StatusRequest,\n"
    "                id: 0,\n"
    "            },\n"
    "            RomPackPacket {\n"
    "                kind: PacketKind::PingRequest,\n"
    "                id: 0,\n"
    "            },\n"
    "        ];\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n",
)

# Generate the exact built-in packet table into the local pack and revalidate it.
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    ROMPACK_SCHEMA_VERSION, RomPack, RomPackMetadata, RomPackRegistry, RomPackResource,\n"
    "    RomPackSource, RomPackSummary, read_rompack, sha256_hex, write_rompack,\n",
    "    ROMPACK_SCHEMA_VERSION, RomPack, RomPackMetadata, RomPackPacket, RomPackRegistry,\n"
    "    RomPackResource, RomPackSource, RomPackSummary, read_rompack, sha256_hex, write_rompack,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    pub registry_count: usize,\n",
    "    pub packet_count: usize,\n    pub registry_count: usize,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    pub size: u64,\n    pub registry_count: usize,\n",
    "    pub size: u64,\n    #[serde(default)]\n    pub packet_count: usize,\n    pub registry_count: usize,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    let (registries, resources) = extract_registry_inventory(&game_jar.bytes)?;\n"
    "    validate_against_builtin_profile(\n",
    "    let (registries, resources) = extract_registry_inventory(&game_jar.bytes)?;\n"
    "    let packets = builtin_packet_inventory()?;\n"
    "    validate_against_builtin_profile(\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        &manifest.source.sha1,\n        &registries,\n    )?;\n\n    let pack = RomPack {\n",
    "        &manifest.source.sha1,\n        &packets,\n        &registries,\n    )?;\n\n    let pack = RomPack {\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        },\n        registries,\n        resources,\n",
    "        },\n        packets,\n        registries,\n        resources,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        size: summary.size,\n        registry_count: summary.registry_count,\n",
    "        size: summary.size,\n        packet_count: summary.packet_count,\n        registry_count: summary.registry_count,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        game_jar_sha256: game_jar.sha256,\n        registry_count: summary.registry_count,\n",
    "        game_jar_sha256: game_jar.sha256,\n        packet_count: summary.packet_count,\n        registry_count: summary.registry_count,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        && summary.registry_count == record.registry_count\n",
    "        && summary.packet_count == record.packet_count\n        && summary.registry_count == record.registry_count\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "            &pack.metadata.source.official_server_sha1,\n            &pack.registries,\n",
    "            &pack.metadata.source.official_server_sha1,\n            &pack.packets,\n            &pack.registries,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        game_jar_sha256: pack.metadata.source.game_jar_sha256,\n        registry_count: summary.registry_count,\n",
    "        game_jar_sha256: pack.metadata.source.game_jar_sha256,\n        packet_count: summary.packet_count,\n        registry_count: summary.registry_count,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "fn validate_against_builtin_profile(\n"
    "    version: &str,\n"
    "    protocol: i32,\n"
    "    official_sha1: &str,\n"
    "    registries: &[RomPackRegistry],\n"
    ") -> Result<()> {\n",
    "fn builtin_packet_inventory() -> Result<Vec<RomPackPacket>> {\n"
    "    let profile = version_26_1_2::protocol_profile()\n"
    "        .context(\"cannot build the built-in 26.1.2 packet table\")?;\n"
    "    Ok(profile\n"
    "        .packets()\n"
    "        .iter()\n"
    "        .map(|(kind, id)| RomPackPacket { kind, id })\n"
    "        .collect())\n"
    "}\n\n"
    "fn validate_against_builtin_profile(\n"
    "    version: &str,\n"
    "    protocol: i32,\n"
    "    official_sha1: &str,\n"
    "    packets: &[RomPackPacket],\n"
    "    registries: &[RomPackRegistry],\n"
    ") -> Result<()> {\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    if !official_sha1.eq_ignore_ascii_case(version_26_1_2::OFFICIAL_SERVER_SHA1) {\n"
    "        bail!(\"official server SHA-1 does not match the built-in 26.1.2 provenance record\");\n"
    "    }\n\n"
    "    let expected: BTreeMap<_, _> = version_26_1_2::SYNCHRONIZED_REGISTRIES\n",
    "    if !official_sha1.eq_ignore_ascii_case(version_26_1_2::OFFICIAL_SERVER_SHA1) {\n"
    "        bail!(\"official server SHA-1 does not match the built-in 26.1.2 provenance record\");\n"
    "    }\n\n"
    "    let expected_packets = builtin_packet_inventory()?;\n"
    "    if packets != expected_packets {\n"
    "        bail!(\n"
    "            \"generated packet table does not match the built-in 26.1.2 profile: expected {} records, got {}\",\n"
    "            expected_packets.len(),\n"
    "            packets.len()\n"
    "        );\n"
    "    }\n\n"
    "    let expected: BTreeMap<_, _> = version_26_1_2::SYNCHRONIZED_REGISTRIES\n",
)

replace_once(
    "crates/rom-bootstrap/src/main.rs",
    "                    \"Registries: {} / entries: {} / source resources: {}\",\n"
    "                    report.registry_count, report.registry_entry_count, report.resource_count\n",
    "                    \"Packets: {} / registries: {} / entries: {} / source resources: {}\",\n"
    "                    report.packet_count,\n"
    "                    report.registry_count,\n"
    "                    report.registry_entry_count,\n"
    "                    report.resource_count\n",
)

# Load numeric packet IDs from the generated pack and cache that profile in ServerConfig.
replace_once(
    "crates/ferrum-server/src/main.rs",
    "use ferrum_rompack::{RomPack, read_rompack};\n",
    "use ferrum_rompack::{RomPack, RomPackPacket, read_rompack};\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    packets: PacketIds,\n}\n",
    "    packets: PacketIds,\n    runtime_profile: Option<ProtocolProfile>,\n}\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    let config = ServerConfig::from_file(&config_path)\n"
    "        .with_context(|| format!(\"cannot load {}\", config_path.display()))?;\n"
    "    config\n"
    "        .protocol_profile()\n"
    "        .context(\"cannot build configured protocol profile\")?;\n"
    "    if let Some(version_pack) = &cli.version_pack {\n"
    "        validate_version_pack(version_pack)?;\n"
    "    }\n",
    "    let mut config = ServerConfig::from_file(&config_path)\n"
    "        .with_context(|| format!(\"cannot load {}\", config_path.display()))?;\n"
    "    let runtime_profile = if let Some(version_pack) = &cli.version_pack {\n"
    "        load_version_pack_profile(version_pack, &config)?\n"
    "    } else {\n"
    "        config\n"
    "            .protocol_profile()\n"
    "            .context(\"cannot build configured protocol profile\")?\n"
    "    };\n"
    "    config.runtime_profile = Some(runtime_profile);\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "fn validate_version_pack(path: &Path) -> Result<()> {\n",
    "fn load_version_pack_profile(path: &Path, config: &ServerConfig) -> Result<ProtocolProfile> {\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    validate_builtin_26_1_2_pack(&pack)?;\n"
    "    println!(\n"
    "        \"loaded RoM version pack {} (SHA-256 {}, {} registries / {} entries)\",\n"
    "        canonical.display(),\n"
    "        summary.sha256,\n"
    "        summary.registry_count,\n"
    "        summary.registry_entry_count\n"
    "    );\n"
    "    Ok(())\n"
    "}\n\n"
    "fn validate_builtin_26_1_2_pack(pack: &RomPack) -> Result<()> {\n",
    "    validate_builtin_26_1_2_pack(&pack)?;\n"
    "    if config.profile_name.as_deref() != Some(version_26_1_2::PROFILE_NAME)\n"
    "        || config.protocol != pack.metadata.protocol\n"
    "        || config.version_name != version_26_1_2::VERSION_NAME\n"
    "    {\n"
    "        bail!(\"server configuration does not match the generated 26.1.2 version pack\");\n"
    "    }\n"
    "    let profile = protocol_profile_from_packets(\n"
    "        &config.version_name,\n"
    "        pack.metadata.protocol,\n"
    "        &pack.packets,\n"
    "    )?;\n"
    "    println!(\n"
    "        \"loaded RoM version pack {} (SHA-256 {}, {} packets, {} registries / {} entries)\",\n"
    "        canonical.display(),\n"
    "        summary.sha256,\n"
    "        summary.packet_count,\n"
    "        summary.registry_count,\n"
    "        summary.registry_entry_count\n"
    "    );\n"
    "    Ok(profile)\n"
    "}\n\n"
    "fn protocol_profile_from_packets(\n"
    "    version_name: &str,\n"
    "    protocol: i32,\n"
    "    packets: &[RomPackPacket],\n"
    ") -> Result<ProtocolProfile> {\n"
    "    let expected: BTreeSet<_> = PacketKind::ALL.iter().copied().collect();\n"
    "    let actual: BTreeSet<_> = packets.iter().map(|packet| packet.kind).collect();\n"
    "    if actual != expected || packets.len() != expected.len() {\n"
    "        bail!(\n"
    "            \"version pack packet kinds do not match the runtime: expected {}, got {}\",\n"
    "            expected.len(),\n"
    "            packets.len()\n"
    "        );\n"
    "    }\n\n"
    "    let mut table = PacketTable::new();\n"
    "    for packet in packets {\n"
    "        table.insert(packet.kind, packet.id)?;\n"
    "    }\n"
    "    ProtocolProfile::new(version_name, protocol, table)\n"
    "        .context(\"cannot build protocol profile from the generated version pack\")\n"
    "}\n\n"
    "fn validate_builtin_26_1_2_pack(pack: &RomPack) -> Result<()> {\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    if !pack\n"
    "        .metadata\n"
    "        .source\n"
    "        .official_server_sha1\n"
    "        .eq_ignore_ascii_case(version_26_1_2::OFFICIAL_SERVER_SHA1)\n"
    "    {\n"
    "        bail!(\"version pack official-source SHA-1 does not match the built-in profile\");\n"
    "    }\n\n"
    "    let expected: BTreeMap<_, _> = version_26_1_2::SYNCHRONIZED_REGISTRIES\n",
    "    if !pack\n"
    "        .metadata\n"
    "        .source\n"
    "        .official_server_sha1\n"
    "        .eq_ignore_ascii_case(version_26_1_2::OFFICIAL_SERVER_SHA1)\n"
    "    {\n"
    "        bail!(\"version pack official-source SHA-1 does not match the built-in profile\");\n"
    "    }\n"
    "    if pack.metadata.patch_set != \"builtin:26.1.2\" {\n"
    "        bail!(\"version pack patch-set identity does not match the built-in profile\");\n"
    "    }\n\n"
    "    let expected: BTreeMap<_, _> = version_26_1_2::SYNCHRONIZED_REGISTRIES\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "            packets: PacketIds::default(),\n",
    "            packets: PacketIds::default(),\n            runtime_profile: None,\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    fn protocol_profile(&self) -> Result<ProtocolProfile> {\n"
    "        if self.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME) {\n",
    "    fn protocol_profile(&self) -> Result<ProtocolProfile> {\n"
    "        if let Some(profile) = &self.runtime_profile {\n"
    "            return Ok(profile.clone());\n"
    "        }\n"
    "        if self.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME) {\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    #[test]\n    fn completes_builtin_26_1_2_known_pack_configuration_sequence() {\n",
    "    #[test]\n"
    "    fn generated_packet_ids_drive_the_runtime_profile() {\n"
    "        let built_in = version_26_1_2::protocol_profile().unwrap();\n"
    "        let mut packets: Vec<_> = built_in\n"
    "            .packets()\n"
    "            .iter()\n"
    "            .map(|(kind, id)| RomPackPacket { kind, id })\n"
    "            .collect();\n"
    "        packets\n"
    "            .iter_mut()\n"
    "            .find(|packet| packet.kind == PacketKind::SystemChat)\n"
    "            .unwrap()\n"
    "            .id = 0x7a;\n"
    "        let profile = protocol_profile_from_packets(\n"
    "            version_26_1_2::VERSION_NAME,\n"
    "            version_26_1_2::PROTOCOL_VERSION,\n"
    "            &packets,\n"
    "        )\n"
    "        .unwrap();\n"
    "        assert_eq!(profile.packets().require(PacketKind::SystemChat).unwrap(), 0x7a);\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn completes_builtin_26_1_2_known_pack_configuration_sequence() {\n",
)

# Documentation and migration note.
replace_once(
    "README.md",
    "- All 28 synchronized 26.1.2 registries with 382 vanilla entries\n",
    "- Packet IDs loaded from the generated schema-v2 `.rompack` during Bootstrap startup\n"
    "- All 28 synchronized 26.1.2 registries with 382 vanilla entries\n",
)
replace_once(
    "README.md",
    "- Runtime replacement of the remaining built-in packet/profile constants with generated pack data\n",
    "- Runtime replacement of remaining built-in world, block-state, biome, and dimension constants with generated pack data\n",
)
replace_once(
    "README.md",
    "The extractor opens the verified local JAR, resolves the bundled game JAR when present, validates all selected JSON resources, derives the exact synchronized-registry identifiers, compares them with the built-in 26.1.2 manifest, and writes an integrity-protected `.rompack`.\n",
    "The extractor opens the verified local JAR, resolves the bundled game JAR when present, validates all selected JSON resources, derives the synchronized-registry identifiers, adds the exact semantic packet table, compares both with the built-in 26.1.2 profile, and writes an integrity-protected schema-v2 `.rompack`. Existing schema-v1 packs must be regenerated with `generate --force`.\n",
)
replace_once(
    "README.md",
    "- `ferrum-rompack` — deterministic pack encoding, integrity validation, and bounded decoding\n",
    "- `ferrum-rompack` — deterministic packet/profile metadata encoding, integrity validation, and bounded decoding\n",
)
replace_once(
    "README.md",
    "1. Package `rom-bootstrap` alongside `rom-server` in native release archives\n"
    "2. Move more version-specific runtime metadata from built-in Rust constants into generated packs\n"
    "3. Wire dedicated network workers into the authoritative 20 TPS runtime\n"
    "4. Add full block interaction and inventory validation\n"
    "5. Add entities and entity tracking\n"
    "6. Add persistent Anvil region loading and saving\n"
    "7. Add Microsoft account authentication and encrypted online mode\n"
    "8. Add additional Minecraft version profiles\n",
    "1. Move remaining world, block-state, biome, and dimension metadata into generated packs\n"
    "2. Wire dedicated network workers into the authoritative 20 TPS runtime\n"
    "3. Add full block interaction and inventory validation\n"
    "4. Add entities and entity tracking\n"
    "5. Add persistent Anvil region loading and saving\n"
    "6. Add Microsoft account authentication and encrypted online mode\n"
    "7. Add additional Minecraft version profiles\n",
)

replace_once(
    "docs/BOOTSTRAP.md",
    "8. Compare the resulting 28 registries and 382 identifiers with the built-in 26.1.2 manifest.\n"
    "9. Write a deterministic `.rompack` with a container SHA-256 trailer and provenance metadata.\n"
    "10. Revalidate the pack before `rom-bootstrap run`, then pass it to the native server for a second profile check.\n",
    "8. Add the exact 26.1.2 semantic packet table and compare it with the built-in generation profile.\n"
    "9. Compare the resulting 28 registries and 382 identifiers with the built-in 26.1.2 manifest.\n"
    "10. Write a deterministic schema-v2 `.rompack` with a container SHA-256 trailer and provenance metadata.\n"
    "11. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile` from the packet IDs inside the pack.\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "Use `--force` to regenerate an already valid pack. Generation is deterministic for the same verified source JAR and extractor version.\n",
    "Use `--force` to regenerate an already valid pack. Generation is deterministic for the same verified source JAR and extractor version. Schema-v1 packs are intentionally rejected after the packet-table migration and must be regenerated.\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "1. Move packet tables and additional version-specific runtime metadata into generated packs.\n"
    "2. Add more independently testable extractors only when the server consumes their output.\n"
    "3. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.\n",
    "1. Move remaining world, block-state, biome, and dimension metadata into generated packs.\n"
    "2. Add more independently testable extractors only when the server consumes their output.\n"
    "3. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.\n",
)
