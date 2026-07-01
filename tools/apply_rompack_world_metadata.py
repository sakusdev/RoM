from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Schema v3 carries the world/chunk IDs consumed by the native runtime.
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "use std::{\n    fs::{self, File},\n",
    "use std::{\n    collections::BTreeSet,\n    fs::{self, File},\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 2;\n",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 3;\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    pub max_packets: usize,\n    pub max_registries: usize,\n",
    "    pub max_packets: usize,\n    pub max_sections: usize,\n    pub max_registries: usize,\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "            max_packets: 4_096,\n            max_registries: 1_024,\n",
    "            max_packets: 4_096,\n            max_sections: 1_024,\n            max_registries: 1_024,\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    pub metadata: RomPackMetadata,\n    pub packets: Vec<RomPackPacket>,\n    pub registries: Vec<RomPackRegistry>,\n",
    "    pub metadata: RomPackMetadata,\n    pub packets: Vec<RomPackPacket>,\n    pub world: RomPackWorld,\n    pub registries: Vec<RomPackRegistry>,\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\npub struct RomPackPacket {\n",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n"
    "pub struct RomPackWorld {\n"
    "    pub data_version: i32,\n"
    "    pub overworld_min_section_y: i32,\n"
    "    pub overworld_section_count: usize,\n"
    "    pub block_states: RomPackBlockStates,\n"
    "    pub biomes: RomPackBiomes,\n"
    "}\n\n"
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n"
    "pub struct RomPackBlockStates {\n"
    "    pub air: u32,\n"
    "    pub stone: u32,\n"
    "    pub grass: u32,\n"
    "    pub dirt: u32,\n"
    "    pub bedrock: u32,\n"
    "}\n\n"
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n"
    "pub struct RomPackBiomes {\n"
    "    pub plains: u32,\n"
    "}\n\n"
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n"
    "pub struct RomPackPacket {\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    validate_hex(\"game JAR SHA-256\", &metadata.source.game_jar_sha256, 64)?;\n\n"
    "    if pack.packets.is_empty() {\n",
    "    validate_hex(\"game JAR SHA-256\", &metadata.source.game_jar_sha256, 64)?;\n\n"
    "    let world = pack.world;\n"
    "    if world.data_version < 0 {\n"
    "        bail!(\"world data version cannot be negative\");\n"
    "    }\n"
    "    if world.overworld_section_count == 0\n"
    "        || world.overworld_section_count > limits.max_sections\n"
    "    {\n"
    "        bail!(\"overworld section count is outside the configured range\");\n"
    "    }\n"
    "    let section_count = i32::try_from(world.overworld_section_count)\n"
    "        .context(\"overworld section count exceeds i32\")?;\n"
    "    let section_end = world\n"
    "        .overworld_min_section_y\n"
    "        .checked_add(section_count)\n"
    "        .context(\"overworld section range overflow\")?;\n"
    "    world\n"
    "        .overworld_min_section_y\n"
    "        .checked_mul(16)\n"
    "        .context(\"overworld minimum block y overflow\")?;\n"
    "    section_end\n"
    "        .checked_mul(16)\n"
    "        .context(\"overworld maximum block y overflow\")?;\n"
    "    let block_states = [\n"
    "        world.block_states.air,\n"
    "        world.block_states.stone,\n"
    "        world.block_states.grass,\n"
    "        world.block_states.dirt,\n"
    "        world.block_states.bedrock,\n"
    "    ];\n"
    "    if block_states.into_iter().collect::<BTreeSet<_>>().len() != block_states.len() {\n"
    "        bail!(\"required flat-world block-state IDs must be distinct\");\n"
    "    }\n\n"
    "    if pack.packets.is_empty() {\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "            packets: vec![\n",
    "            world: RomPackWorld {\n"
    "                data_version: 4_790,\n"
    "                overworld_min_section_y: -4,\n"
    "                overworld_section_count: 24,\n"
    "                block_states: RomPackBlockStates {\n"
    "                    air: 0,\n"
    "                    stone: 1,\n"
    "                    grass: 9,\n"
    "                    dirt: 10,\n"
    "                    bedrock: 85,\n"
    "                },\n"
    "                biomes: RomPackBiomes { plains: 40 },\n"
    "            },\n"
    "            packets: vec![\n",
)
replace_once(
    "crates/ferrum-rompack/src/lib.rs",
    "    fn rejects_unsorted_or_unsafe_records() {\n        let mut pack = sample_pack();\n",
    "    fn rejects_unsorted_or_unsafe_records() {\n"
    "        let mut pack = sample_pack();\n"
    "        pack.world.overworld_section_count = 0;\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n"
    "        pack.world.block_states.stone = pack.world.block_states.air;\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n",
)

# Bootstrap writes and checks the exact built-in world profile.
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    ROMPACK_SCHEMA_VERSION, RomPack, RomPackMetadata, RomPackPacket, RomPackRegistry,\n"
    "    RomPackResource, RomPackSource, RomPackSummary, read_rompack, sha256_hex, write_rompack,\n",
    "    ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackMetadata,\n"
    "    RomPackPacket, RomPackRegistry, RomPackResource, RomPackSource, RomPackSummary,\n"
    "    RomPackWorld, read_rompack, sha256_hex, write_rompack,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    pub packet_count: usize,\n    pub registry_count: usize,\n",
    "    pub packet_count: usize,\n"
    "    pub world_data_version: i32,\n"
    "    pub overworld_min_section_y: i32,\n"
    "    pub overworld_section_count: usize,\n"
    "    pub registry_count: usize,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    let packets = builtin_packet_inventory()?;\n    validate_against_builtin_profile(\n",
    "    let packets = builtin_packet_inventory()?;\n"
    "    let world = builtin_world_metadata();\n"
    "    validate_against_builtin_profile(\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        &manifest.source.sha1,\n        &packets,\n        &registries,\n",
    "        &manifest.source.sha1,\n        &packets,\n        &world,\n        &registries,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        packets,\n        registries,\n",
    "        packets,\n        world,\n        registries,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        packet_count: summary.packet_count,\n        registry_count: summary.registry_count,\n",
    "        packet_count: summary.packet_count,\n"
    "        world_data_version: pack.world.data_version,\n"
    "        overworld_min_section_y: pack.world.overworld_min_section_y,\n"
    "        overworld_section_count: pack.world.overworld_section_count,\n"
    "        registry_count: summary.registry_count,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        packet_count: summary.packet_count,\n        registry_count: summary.registry_count,\n",
    "        packet_count: summary.packet_count,\n"
    "        world_data_version: pack.world.data_version,\n"
    "        overworld_min_section_y: pack.world.overworld_min_section_y,\n"
    "        overworld_section_count: pack.world.overworld_section_count,\n"
    "        registry_count: summary.registry_count,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "            &pack.packets,\n            &pack.registries,\n",
    "            &pack.packets,\n            &pack.world,\n            &pack.registries,\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "fn validate_against_builtin_profile(\n"
    "    version: &str,\n"
    "    protocol: i32,\n"
    "    official_sha1: &str,\n"
    "    packets: &[RomPackPacket],\n"
    "    registries: &[RomPackRegistry],\n",
    "fn builtin_world_metadata() -> RomPackWorld {\n"
    "    RomPackWorld {\n"
    "        data_version: version_26_1_2::WORLD_VERSION,\n"
    "        overworld_min_section_y: version_26_1_2::OVERWORLD_MIN_SECTION_Y,\n"
    "        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,\n"
    "        block_states: RomPackBlockStates {\n"
    "            air: version_26_1_2::AIR_BLOCK_STATE_ID,\n"
    "            stone: version_26_1_2::STONE_BLOCK_STATE_ID,\n"
    "            grass: version_26_1_2::GRASS_BLOCK_STATE_ID,\n"
    "            dirt: version_26_1_2::DIRT_BLOCK_STATE_ID,\n"
    "            bedrock: version_26_1_2::BEDROCK_BLOCK_STATE_ID,\n"
    "        },\n"
    "        biomes: RomPackBiomes {\n"
    "            plains: version_26_1_2::PLAINS_BIOME_ID,\n"
    "        },\n"
    "    }\n"
    "}\n\n"
    "fn validate_against_builtin_profile(\n"
    "    version: &str,\n"
    "    protocol: i32,\n"
    "    official_sha1: &str,\n"
    "    packets: &[RomPackPacket],\n"
    "    world: &RomPackWorld,\n"
    "    registries: &[RomPackRegistry],\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "    let expected_packets = builtin_packet_inventory()?;\n"
    "    if packets != expected_packets {\n",
    "    let expected_world = builtin_world_metadata();\n"
    "    if *world != expected_world {\n"
    "        bail!(\"generated world metadata does not match the built-in 26.1.2 profile\");\n"
    "    }\n\n"
    "    let expected_packets = builtin_packet_inventory()?;\n"
    "    if packets != expected_packets {\n",
)

replace_once(
    "crates/rom-bootstrap/src/main.rs",
    "                    \"Packets: {} / registries: {} / entries: {} / source resources: {}\",\n"
    "                    report.packet_count,\n",
    "                    \"Packets: {} / data version: {} / sections: {}+{} / registries: {} / entries: {} / source resources: {}\",\n"
    "                    report.packet_count,\n"
    "                    report.world_data_version,\n"
    "                    report.overworld_min_section_y,\n"
    "                    report.overworld_section_count,\n",
)

# The native server consumes pack-provided section, block-state, and biome IDs.
replace_once(
    "crates/ferrum-server/src/main.rs",
    "use ferrum_rompack::{RomPack, RomPackPacket, read_rompack};\n",
    "use ferrum_rompack::{RomPack, RomPackPacket, RomPackWorld, read_rompack};\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "impl ServerState {\n    fn new(initial_online_players: i32) -> Self {\n"
    "        Self {\n"
    "            online_players: AtomicI32::new(initial_online_players),\n"
    "            next_connection_id: AtomicU64::new(1),\n"
    "            world: play_runtime::SharedWorld::static_flat(),\n"
    "        }\n"
    "    }\n",
    "impl ServerState {\n"
    "    fn new(initial_online_players: i32) -> Self {\n"
    "        Self::with_world(initial_online_players, play_runtime::builtin_world_profile())\n"
    "            .expect(\"built-in world profile must initialize\")\n"
    "    }\n\n"
    "    fn with_world(initial_online_players: i32, world: RomPackWorld) -> Result<Self> {\n"
    "        Ok(Self {\n"
    "            online_players: AtomicI32::new(initial_online_players),\n"
    "            next_connection_id: AtomicU64::new(1),\n"
    "            world: play_runtime::SharedWorld::new(\n"
    "                ChunkPos {\n"
    "                    x: STATIC_CHUNK_X,\n"
    "                    z: STATIC_CHUNK_Z,\n"
    "                },\n"
    "                world,\n"
    "            )?,\n"
    "        })\n"
    "    }\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    let runtime_profile = if let Some(version_pack) = &cli.version_pack {\n"
    "        load_version_pack_profile(version_pack, &config)?\n"
    "    } else {\n"
    "        config\n"
    "            .protocol_profile()\n"
    "            .context(\"cannot build configured protocol profile\")?\n"
    "    };\n"
    "    config.runtime_profile = Some(runtime_profile);\n"
    "    let state = Arc::new(ServerState::new(config.online_players));\n",
    "    let (runtime_profile, world_profile) = if let Some(version_pack) = &cli.version_pack {\n"
    "        let loaded = load_version_pack(version_pack, &config)?;\n"
    "        (loaded.profile, loaded.world)\n"
    "    } else {\n"
    "        (\n"
    "            config\n"
    "                .protocol_profile()\n"
    "                .context(\"cannot build configured protocol profile\")?,\n"
    "            play_runtime::builtin_world_profile(),\n"
    "        )\n"
    "    };\n"
    "    config.runtime_profile = Some(runtime_profile);\n"
    "    let state = Arc::new(ServerState::with_world(\n"
    "        config.online_players,\n"
    "        world_profile,\n"
    "    )?);\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "fn load_version_pack_profile(path: &Path, config: &ServerConfig) -> Result<ProtocolProfile> {\n",
    "struct LoadedVersionPack {\n"
    "    profile: ProtocolProfile,\n"
    "    world: RomPackWorld,\n"
    "}\n\n"
    "fn load_version_pack(path: &Path, config: &ServerConfig) -> Result<LoadedVersionPack> {\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    let profile =\n"
    "        protocol_profile_from_packets(&config.version_name, pack.metadata.protocol, &pack.packets)?;\n"
    "    println!(\n"
    "        \"loaded RoM version pack {} (SHA-256 {}, {} packets, {} registries / {} entries)\",\n",
    "    validate_world_profile(&pack.world)?;\n"
    "    let profile =\n"
    "        protocol_profile_from_packets(&config.version_name, pack.metadata.protocol, &pack.packets)?;\n"
    "    println!(\n"
    "        \"loaded RoM version pack {} (SHA-256 {}, {} packets, data version {}, {} sections, {} registries / {} entries)\",\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "        summary.packet_count,\n        summary.registry_count,\n",
    "        summary.packet_count,\n"
    "        pack.world.data_version,\n"
    "        pack.world.overworld_section_count,\n"
    "        summary.registry_count,\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    Ok(profile)\n}\n\nfn protocol_profile_from_packets(\n",
    "    Ok(LoadedVersionPack {\n"
    "        profile,\n"
    "        world: pack.world,\n"
    "    })\n"
    "}\n\n"
    "fn validate_world_profile(world: &RomPackWorld) -> Result<()> {\n"
    "    let section_count = i32::try_from(world.overworld_section_count)\n"
    "        .context(\"overworld section count exceeds i32\")?;\n"
    "    let min_block_y = world\n"
    "        .overworld_min_section_y\n"
    "        .checked_mul(16)\n"
    "        .context(\"overworld minimum block y overflow\")?;\n"
    "    let max_block_y = world\n"
    "        .overworld_min_section_y\n"
    "        .checked_add(section_count)\n"
    "        .and_then(|section| section.checked_mul(16))\n"
    "        .and_then(|block| block.checked_sub(1))\n"
    "        .context(\"overworld maximum block y overflow\")?;\n"
    "    if !(min_block_y..=max_block_y).contains(&STATIC_FLOOR_Y) {\n"
    "        bail!(\n"
    "            \"generated world height {min_block_y}..={max_block_y} does not contain the static floor at {STATIC_FLOOR_Y}\"\n"
    "        );\n"
    "    }\n"
    "    Ok(())\n"
    "}\n\n"
    "fn protocol_profile_from_packets(\n",
)

# SharedWorld carries the generated world profile through chunk creation and interactions.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "use ferrum_protocol::{\n",
    "use ferrum_protocol::{\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "use ferrum_runtime::{ConnectionId, DeterministicRuntime, Tick};\n",
    "use ferrum_rompack::{RomPackBiomes, RomPackBlockStates, RomPackWorld};\n"
    "use ferrum_runtime::{ConnectionId, DeterministicRuntime, Tick};\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "pub(super) struct SharedWorld {\n    inner: Mutex<SharedWorldInner>,\n}\n",
    "pub(super) struct SharedWorld {\n"
    "    profile: RomPackWorld,\n"
    "    inner: Mutex<SharedWorldInner>,\n"
    "}\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "impl SharedWorld {\n    pub(super) fn new(center: ChunkPos) -> Result<Self> {\n"
    "        Ok(Self {\n"
    "            inner: Mutex::new(SharedWorldInner {\n"
    "                runtime: new_local_world_runtime(center)?,\n",
    "pub(super) fn builtin_world_profile() -> RomPackWorld {\n"
    "    RomPackWorld {\n"
    "        data_version: version_26_1_2::WORLD_VERSION,\n"
    "        overworld_min_section_y: version_26_1_2::OVERWORLD_MIN_SECTION_Y,\n"
    "        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,\n"
    "        block_states: RomPackBlockStates {\n"
    "            air: version_26_1_2::AIR_BLOCK_STATE_ID,\n"
    "            stone: version_26_1_2::STONE_BLOCK_STATE_ID,\n"
    "            grass: version_26_1_2::GRASS_BLOCK_STATE_ID,\n"
    "            dirt: version_26_1_2::DIRT_BLOCK_STATE_ID,\n"
    "            bedrock: version_26_1_2::BEDROCK_BLOCK_STATE_ID,\n"
    "        },\n"
    "        biomes: RomPackBiomes {\n"
    "            plains: version_26_1_2::PLAINS_BIOME_ID,\n"
    "        },\n"
    "    }\n"
    "}\n\n"
    "impl SharedWorld {\n"
    "    pub(super) fn new(center: ChunkPos, profile: RomPackWorld) -> Result<Self> {\n"
    "        Ok(Self {\n"
    "            profile,\n"
    "            inner: Mutex::new(SharedWorldInner {\n"
    "                runtime: new_local_world_runtime(center, profile)?,\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        Self::new(ChunkPos {\n"
    "            x: STATIC_CHUNK_X,\n"
    "            z: STATIC_CHUNK_Z,\n"
    "        })\n",
    "        Self::new(\n"
    "            ChunkPos {\n"
    "                x: STATIC_CHUNK_X,\n"
    "                z: STATIC_CHUNK_Z,\n"
    "            },\n"
    "            builtin_world_profile(),\n"
    "        )\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        ensure_chunks_loaded(inner.runtime.state_mut(), positions)\n",
    "        ensure_chunks_loaded(inner.runtime.state_mut(), positions, self.profile)\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {\n",
    "    #[must_use]\n"
    "    pub(super) const fn world_profile(&self) -> RomPackWorld {\n"
    "        self.profile\n"
    "    }\n\n"
    "    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "                            BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),\n",
    "                            BlockStateId::new(shared_world.world_profile().block_states.air),\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "                            BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),\n",
    "                            BlockStateId::new(shared_world.world_profile().block_states.stone),\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn new_local_world_runtime(center: ChunkPos) -> Result<LocalWorldRuntime> {\n"
    "    let mut store = ChunkStore::new();\n"
    "    seed_chunk_square(&mut store, center, STATIC_CHUNK_RADIUS)?;\n",
    "fn new_local_world_runtime(center: ChunkPos, profile: RomPackWorld) -> Result<LocalWorldRuntime> {\n"
    "    let mut store = ChunkStore::new();\n"
    "    seed_chunk_square(&mut store, center, STATIC_CHUNK_RADIUS, profile)?;\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn seed_chunk_square(store: &mut ChunkStore, center: ChunkPos, radius: i32) -> Result<()> {\n",
    "fn seed_chunk_square(\n"
    "    store: &mut ChunkStore,\n"
    "    center: ChunkPos,\n"
    "    radius: i32,\n"
    "    profile: RomPackWorld,\n"
    ") -> Result<()> {\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "            store.insert(flat_chunk(ChunkPos { x, z })?);\n",
    "            store.insert(flat_chunk(ChunkPos { x, z }, profile)?);\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn ensure_chunks_loaded(store: &mut ChunkStore, positions: &[ChunkPos]) -> Result<()> {\n",
    "fn ensure_chunks_loaded(\n"
    "    store: &mut ChunkStore,\n"
    "    positions: &[ChunkPos],\n"
    "    profile: RomPackWorld,\n"
    ") -> Result<()> {\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "            store.insert(flat_chunk(*pos)?);\n",
    "            store.insert(flat_chunk(*pos, profile)?);\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        Some(state) if state == BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID)\n",
    "        Some(state) if state == BlockStateId::new(shared_world.world_profile().block_states.air)\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        Some(state)\n"
    "            if state != BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID)\n"
    "                && state != BlockStateId::new(version_26_1_2::BEDROCK_BLOCK_STATE_ID)\n",
    "        Some(state)\n"
    "            if state != BlockStateId::new(shared_world.world_profile().block_states.air)\n"
    "                && state != BlockStateId::new(shared_world.world_profile().block_states.bedrock)\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn flat_chunk(pos: ChunkPos) -> Result<StaticChunk> {\n"
    "    Ok(StaticChunk::flat_overworld(\n"
    "        pos,\n"
    "        version_26_1_2::OVERWORLD_MIN_SECTION_Y,\n"
    "        version_26_1_2::OVERWORLD_SECTION_COUNT,\n"
    "        FlatWorldSpec {\n"
    "            floor_y: STATIC_FLOOR_Y,\n"
    "            air: BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),\n"
    "            bedrock: BlockStateId::new(version_26_1_2::BEDROCK_BLOCK_STATE_ID),\n"
    "            stone: BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),\n"
    "            dirt: BlockStateId::new(version_26_1_2::DIRT_BLOCK_STATE_ID),\n"
    "            grass: BlockStateId::new(version_26_1_2::GRASS_BLOCK_STATE_ID),\n"
    "            biome: BiomeId::new(version_26_1_2::PLAINS_BIOME_ID),\n"
    "        },\n"
    "    )?)\n"
    "}\n",
    "fn flat_chunk(pos: ChunkPos, profile: RomPackWorld) -> Result<StaticChunk> {\n"
    "    Ok(StaticChunk::flat_overworld(\n"
    "        pos,\n"
    "        profile.overworld_min_section_y,\n"
    "        profile.overworld_section_count,\n"
    "        FlatWorldSpec {\n"
    "            floor_y: STATIC_FLOOR_Y,\n"
    "            air: BlockStateId::new(profile.block_states.air),\n"
    "            bedrock: BlockStateId::new(profile.block_states.bedrock),\n"
    "            stone: BlockStateId::new(profile.block_states.stone),\n"
    "            dirt: BlockStateId::new(profile.block_states.dirt),\n"
    "            grass: BlockStateId::new(profile.block_states.grass),\n"
    "            biome: BiomeId::new(profile.biomes.plains),\n"
    "        },\n"
    "    )?)\n"
    "}\n",
)
# Update the two direct runtime constructors in tests.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        let mut runtime = new_local_world_runtime(ChunkPos { x: 0, z: 0 }).unwrap();\n",
    "        let mut runtime =\n"
    "            new_local_world_runtime(ChunkPos { x: 0, z: 0 }, builtin_world_profile()).unwrap();\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        let mut runtime = new_local_world_runtime(ChunkPos { x: 0, z: 0 }).unwrap();\n",
    "        let mut runtime =\n"
    "            new_local_world_runtime(ChunkPos { x: 0, z: 0 }, builtin_world_profile()).unwrap();\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    #[test]\n    fn local_world_runtime_applies_block_events_through_authoritative_ticks() {\n",
    "    #[test]\n"
    "    fn generated_world_profile_drives_chunk_layout_and_block_states() {\n"
    "        let mut profile = builtin_world_profile();\n"
    "        profile.overworld_min_section_y = -2;\n"
    "        profile.overworld_section_count = 8;\n"
    "        profile.block_states.stone = 123;\n"
    "        let world = SharedWorld::new(ChunkPos { x: 0, z: 0 }, profile).unwrap();\n"
    "        let chunk = world.chunk_snapshot(ChunkPos { x: 0, z: 0 }).unwrap();\n"
    "        assert_eq!(chunk.min_section_y(), -2);\n"
    "        assert_eq!(chunk.sections().len(), 8);\n"
    "        assert_eq!(\n"
    "            world.world_block(BlockPos { x: 0, y: 61, z: 0 }).unwrap(),\n"
    "            BlockStateId::new(123)\n"
    "        );\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn local_world_runtime_applies_block_events_through_authoritative_ticks() {\n",
)

# Document schema-v3 migration and completed world metadata consumption.
replace_once(
    "README.md",
    "- Packet IDs loaded from the generated schema-v2 `.rompack` during Bootstrap startup\n",
    "- Packet IDs, world height, flat-world block-state IDs, and biome ID loaded from the generated schema-v3 `.rompack` during Bootstrap startup\n",
)
replace_once(
    "README.md",
    "- Runtime replacement of remaining built-in world, block-state, biome, and dimension constants with generated pack data\n",
    "- Runtime replacement of remaining dimension registry payloads and other gameplay constants with generated pack data\n",
)
replace_once(
    "README.md",
    "writes an integrity-protected schema-v2 `.rompack`. Existing schema-v1 packs must be regenerated with `generate --force`.\n",
    "writes an integrity-protected schema-v3 `.rompack`. Existing schema-v1/v2 packs must be regenerated with `generate --force`.\n",
)
replace_once(
    "README.md",
    "1. Move remaining world, block-state, biome, and dimension metadata into generated packs\n",
    "1. Move remaining dimension registry payloads and gameplay constants into generated packs\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "10. Write a deterministic schema-v2 `.rompack` with a container SHA-256 trailer and provenance metadata.\n"
    "11. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile` from the packet IDs inside the pack.\n",
    "10. Add the world data version, overworld section range, required flat-world block-state IDs, and plains biome ID.\n"
    "11. Write a deterministic schema-v3 `.rompack` with a container SHA-256 trailer and provenance metadata.\n"
    "12. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile` and initial shared world from pack metadata.\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "Schema-v1 packs are intentionally rejected after the packet-table migration and must be regenerated.\n",
    "Schema-v1 and schema-v2 packs are intentionally rejected after the packet-table and world-metadata migrations and must be regenerated.\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "1. Move remaining world, block-state, biome, and dimension metadata into generated packs.\n",
    "1. Move remaining dimension registry payloads and gameplay constants into generated packs.\n",
)
