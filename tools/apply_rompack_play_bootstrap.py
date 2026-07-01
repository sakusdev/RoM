from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Centralize the built-in generation profile values used to create schema-v4 packs.
replace_once(
    "crates/ferrum-version-26-1-2/src/lib.rs",
    "pub const PLAINS_BIOME_ID: u32 = 40;\n",
    "pub const PLAINS_BIOME_ID: u32 = 40;\n"
    "pub const OVERWORLD_DIMENSION: &str = \"minecraft:overworld\";\n"
    "pub const OVERWORLD_DIMENSION_TYPE_ID: i32 = 0;\n"
    "pub const OVERWORLD_SEA_LEVEL: i32 = 63;\n"
    "pub const FLAT_WORLD_FLOOR_Y: i32 = 63;\n"
    "pub const FLAT_WORLD_SPAWN_X: i32 = 0;\n"
    "pub const FLAT_WORLD_SPAWN_Z: i32 = 0;\n",
)

# Extend the local pack with the Play bootstrap world values.
rompack = "crates/ferrum-rompack/src/lib.rs"
replace_once(
    rompack,
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 3;\n",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 4;\n",
)
replace_once(
    rompack,
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\npub struct RomPackWorld {\n"
    "    pub data_version: i32,\n"
    "    pub overworld_min_section_y: i32,\n"
    "    pub overworld_section_count: usize,\n"
    "    pub block_states: RomPackBlockStates,\n"
    "    pub biomes: RomPackBiomes,\n"
    "}\n",
    "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n"
    "pub struct RomPackWorld {\n"
    "    pub data_version: i32,\n"
    "    pub overworld_min_section_y: i32,\n"
    "    pub overworld_section_count: usize,\n"
    "    pub dimension: String,\n"
    "    pub dimension_type_id: i32,\n"
    "    pub sea_level: i32,\n"
    "    pub floor_y: i32,\n"
    "    pub spawn_x: i32,\n"
    "    pub spawn_z: i32,\n"
    "    pub block_states: RomPackBlockStates,\n"
    "    pub biomes: RomPackBiomes,\n"
    "}\n",
)
replace_once(
    rompack,
    "    let world = pack.world;\n"
    "    if world.data_version < 0 {\n",
    "    let world = &pack.world;\n"
    "    if world.data_version < 0 {\n",
)
replace_once(
    rompack,
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
    "        .context(\"overworld maximum block y overflow\")?;\n",
    "    validate_resource_location(\n"
    "        \"world dimension\",\n"
    "        &world.dimension,\n"
    "        limits.max_identifier_bytes,\n"
    "    )?;\n"
    "    if world.dimension_type_id < 0 {\n"
    "        bail!(\"dimension type ID cannot be negative\");\n"
    "    }\n"
    "    let section_count = i32::try_from(world.overworld_section_count)\n"
    "        .context(\"overworld section count exceeds i32\")?;\n"
    "    let section_end = world\n"
    "        .overworld_min_section_y\n"
    "        .checked_add(section_count)\n"
    "        .context(\"overworld section range overflow\")?;\n"
    "    let min_block_y = world\n"
    "        .overworld_min_section_y\n"
    "        .checked_mul(16)\n"
    "        .context(\"overworld minimum block y overflow\")?;\n"
    "    let max_block_y = section_end\n"
    "        .checked_mul(16)\n"
    "        .and_then(|value| value.checked_sub(1))\n"
    "        .context(\"overworld maximum block y overflow\")?;\n"
    "    let floor_bottom = world\n"
    "        .floor_y\n"
    "        .checked_sub(3)\n"
    "        .context(\"flat-world floor range overflow\")?;\n"
    "    let player_spawn_y = world\n"
    "        .floor_y\n"
    "        .checked_add(2)\n"
    "        .context(\"flat-world player spawn overflow\")?;\n"
    "    for (label, value) in [\n"
    "        (\"sea level\", world.sea_level),\n"
    "        (\"flat-world floor bottom\", floor_bottom),\n"
    "        (\"flat-world player spawn\", player_spawn_y),\n"
    "    ] {\n"
    "        if !(min_block_y..=max_block_y).contains(&value) {\n"
    "            bail!(\"{label} {value} is outside world height {min_block_y}..={max_block_y}\");\n"
    "        }\n"
    "    }\n",
)
replace_once(
    rompack,
    "    for registry in &pack.registries {\n",
    "    for registry in &pack.registries {\n",
)
replace_once(
    rompack,
    "    if pack.resources.len() > limits.max_resources {\n",
    "    let dimension_registry = pack\n"
    "        .registries\n"
    "        .iter()\n"
    "        .find(|registry| registry.id == \"minecraft:dimension_type\")\n"
    "        .context(\"version pack is missing minecraft:dimension_type\")?;\n"
    "    let dimension_type_id = usize::try_from(world.dimension_type_id)\n"
    "        .context(\"dimension type ID exceeds usize\")?;\n"
    "    if dimension_type_id >= dimension_registry.entries.len() {\n"
    "        bail!(\n"
    "            \"dimension type ID {} exceeds registry size {}\",\n"
    "            world.dimension_type_id,\n"
    "            dimension_registry.entries.len()\n"
    "        );\n"
    "    }\n\n"
    "    if pack.resources.len() > limits.max_resources {\n",
)
replace_once(
    rompack,
    "                overworld_section_count: 24,\n"
    "                block_states: RomPackBlockStates {\n",
    "                overworld_section_count: 24,\n"
    "                dimension: \"minecraft:overworld\".to_owned(),\n"
    "                dimension_type_id: 0,\n"
    "                sea_level: 63,\n"
    "                floor_y: 63,\n"
    "                spawn_x: 0,\n"
    "                spawn_z: 0,\n"
    "                block_states: RomPackBlockStates {\n",
)
replace_once(
    rompack,
    "            registries: vec![RomPackRegistry {\n"
    "                id: \"minecraft:worldgen/biome\".to_owned(),\n"
    "                entries: vec![\"minecraft:forest\".to_owned(), \"minecraft:plains\".to_owned()],\n"
    "            }],\n",
    "            registries: vec![\n"
    "                RomPackRegistry {\n"
    "                    id: \"minecraft:dimension_type\".to_owned(),\n"
    "                    entries: vec![\n"
    "                        \"minecraft:overworld\".to_owned(),\n"
    "                        \"minecraft:the_nether\".to_owned(),\n"
    "                    ],\n"
    "                },\n"
    "                RomPackRegistry {\n"
    "                    id: \"minecraft:worldgen/biome\".to_owned(),\n"
    "                    entries: vec![\n"
    "                        \"minecraft:forest\".to_owned(),\n"
    "                        \"minecraft:plains\".to_owned(),\n"
    "                    ],\n"
    "                },\n"
    "            ],\n",
)
replace_once(
    rompack,
    "        assert_eq!(written.registry_count, 1);\n"
    "        assert_eq!(written.registry_entry_count, 2);\n",
    "        assert_eq!(written.registry_count, 2);\n"
    "        assert_eq!(written.registry_entry_count, 4);\n",
)
replace_once(
    rompack,
    "        let mut pack = sample_pack();\n"
    "        pack.world.block_states.stone = pack.world.block_states.air;\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n"
    "        pack.packets.reverse();\n",
    "        let mut pack = sample_pack();\n"
    "        pack.world.block_states.stone = pack.world.block_states.air;\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n"
    "        pack.world.dimension.clear();\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n"
    "        pack.world.dimension_type_id = 9;\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n"
    "        pack.world.floor_y = 400;\n"
    "        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());\n\n"
    "        let mut pack = sample_pack();\n"
    "        pack.packets.reverse();\n",
)

# Generate and report the new world metadata through Bootstrap.
extract = "crates/rom-bootstrap/src/extract.rs"
replace_once(
    extract,
    "    pub overworld_section_count: usize,\n    pub registry_count: usize,\n",
    "    pub overworld_section_count: usize,\n"
    "    pub world_dimension: String,\n"
    "    pub dimension_type_id: i32,\n"
    "    pub sea_level: i32,\n"
    "    pub floor_y: i32,\n"
    "    pub spawn_x: i32,\n"
    "    pub spawn_z: i32,\n"
    "    pub registry_count: usize,\n",
)
replace_once(
    extract,
    "        overworld_section_count: pack.world.overworld_section_count,\n"
    "        registry_count: summary.registry_count,\n",
    "        overworld_section_count: pack.world.overworld_section_count,\n"
    "        world_dimension: pack.world.dimension.clone(),\n"
    "        dimension_type_id: pack.world.dimension_type_id,\n"
    "        sea_level: pack.world.sea_level,\n"
    "        floor_y: pack.world.floor_y,\n"
    "        spawn_x: pack.world.spawn_x,\n"
    "        spawn_z: pack.world.spawn_z,\n"
    "        registry_count: summary.registry_count,\n",
)
replace_once(
    extract,
    "        overworld_section_count: pack.world.overworld_section_count,\n"
    "        registry_count: summary.registry_count,\n",
    "        overworld_section_count: pack.world.overworld_section_count,\n"
    "        world_dimension: pack.world.dimension.clone(),\n"
    "        dimension_type_id: pack.world.dimension_type_id,\n"
    "        sea_level: pack.world.sea_level,\n"
    "        floor_y: pack.world.floor_y,\n"
    "        spawn_x: pack.world.spawn_x,\n"
    "        spawn_z: pack.world.spawn_z,\n"
    "        registry_count: summary.registry_count,\n",
)
replace_once(
    extract,
    "        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,\n"
    "        block_states: RomPackBlockStates {\n",
    "        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,\n"
    "        dimension: version_26_1_2::OVERWORLD_DIMENSION.to_owned(),\n"
    "        dimension_type_id: version_26_1_2::OVERWORLD_DIMENSION_TYPE_ID,\n"
    "        sea_level: version_26_1_2::OVERWORLD_SEA_LEVEL,\n"
    "        floor_y: version_26_1_2::FLAT_WORLD_FLOOR_Y,\n"
    "        spawn_x: version_26_1_2::FLAT_WORLD_SPAWN_X,\n"
    "        spawn_z: version_26_1_2::FLAT_WORLD_SPAWN_Z,\n"
    "        block_states: RomPackBlockStates {\n",
)

replace_once(
    "crates/rom-bootstrap/src/main.rs",
    "                    report.resource_count\n"
    "                );\n",
    "                    report.resource_count\n"
    "                );\n"
    "                println!(\n"
    "                    \"Play world: {} / dimension type {} / sea {} / floor {} / spawn {},{}\",\n"
    "                    report.world_dimension,\n"
    "                    report.dimension_type_id,\n"
    "                    report.sea_level,\n"
    "                    report.floor_y,\n"
    "                    report.spawn_x,\n"
    "                    report.spawn_z\n"
    "                );\n",
)

# Make the server and shared world consume schema-v4 Play metadata.
main = "crates/ferrum-server/src/main.rs"
replace_once(
    main,
    "const STATIC_LEVEL: &str = \"minecraft:overworld\";\n"
    "const STATIC_PLAYER_ID: i32 = 1;\n"
    "const STATIC_TELEPORT_ID: i32 = 1;\n"
    "const STATIC_CHUNK_RADIUS: i32 = 1;\n"
    "const STATIC_SIMULATION_DISTANCE: i32 = 2;\n"
    "const STATIC_SEA_LEVEL: i32 = 63;\n"
    "const STATIC_FLOOR_Y: i32 = 63;\n"
    "const STATIC_CHUNK_X: i32 = 0;\n"
    "const STATIC_CHUNK_Z: i32 = 0;\n",
    "const STATIC_PLAYER_ID: i32 = 1;\n"
    "const STATIC_TELEPORT_ID: i32 = 1;\n"
    "const STATIC_CHUNK_RADIUS: i32 = 1;\n"
    "const STATIC_SIMULATION_DISTANCE: i32 = 2;\n",
)
replace_once(
    main,
    "            world: play_runtime::SharedWorld::new(\n"
    "                ChunkPos {\n"
    "                    x: STATIC_CHUNK_X,\n"
    "                    z: STATIC_CHUNK_Z,\n"
    "                },\n"
    "                world,\n"
    "            )?,\n",
    "            world: {\n"
    "                let center = play_runtime::spawn_chunk(&world);\n"
    "                play_runtime::SharedWorld::new(center, world)?\n"
    "            },\n",
)
replace_once(
    main,
    "    if !(min_block_y..=max_block_y).contains(&STATIC_FLOOR_Y) {\n"
    "        bail!(\n"
    "            \"generated world height {min_block_y}..={max_block_y} does not contain the static floor at {STATIC_FLOOR_Y}\"\n"
    "        );\n"
    "    }\n",
    "    let player_spawn_y = world\n"
    "        .floor_y\n"
    "        .checked_add(2)\n"
    "        .context(\"generated player spawn overflow\")?;\n"
    "    if !(min_block_y..=max_block_y).contains(&player_spawn_y) {\n"
    "        bail!(\n"
    "            \"generated world height {min_block_y}..={max_block_y} does not contain player spawn y {player_spawn_y}\"\n"
    "        );\n"
    "    }\n",
)
replace_once(
    main,
    "    let _world_subscription = world.shared_world.subscribe(world.connection)?;\n"
    "    let chunk = world.shared_world.chunk_snapshot(ChunkPos {\n"
    "        x: STATIC_CHUNK_X,\n"
    "        z: STATIC_CHUNK_Z,\n"
    "    })?;\n",
    "    let _world_subscription = world.shared_world.subscribe(world.connection)?;\n"
    "    let world_profile = world.shared_world.world_profile();\n"
    "    let center = play_runtime::spawn_chunk(world_profile);\n"
    "    let chunk = world.shared_world.chunk_snapshot(center)?;\n",
)
replace_once(
    main,
    "        &encode_join_game(&static_join_game(config))?,\n",
    "        &encode_join_game(&static_join_game(config, world_profile))?,\n",
)
replace_once(
    main,
    "        &encode_default_spawn_position(&static_default_spawn_position())?,\n",
    "        &encode_default_spawn_position(&static_default_spawn_position(world_profile)?)?,\n",
)
replace_once(
    main,
    "        &encode_set_chunk_cache_center(STATIC_CHUNK_X, STATIC_CHUNK_Z),\n",
    "        &encode_set_chunk_cache_center(center.x, center.z),\n",
)
replace_once(
    main,
    "        &encode_player_position(&static_player_position())?,\n",
    "        &encode_player_position(&static_player_position(world_profile))?,\n",
)
replace_once(
    main,
    "fn static_join_game(config: &ServerConfig) -> JoinGame {\n",
    "fn static_join_game(config: &ServerConfig, world: &RomPackWorld) -> JoinGame {\n",
)
replace_once(
    main,
    "        levels: vec![STATIC_LEVEL.to_owned()],\n",
    "        levels: vec![world.dimension.clone()],\n",
)
replace_once(
    main,
    "            dimension_type_id: 0,\n"
    "            dimension: STATIC_LEVEL.to_owned(),\n",
    "            dimension_type_id: world.dimension_type_id,\n"
    "            dimension: world.dimension.clone(),\n",
)
replace_once(
    main,
    "            sea_level: STATIC_SEA_LEVEL,\n",
    "            sea_level: world.sea_level,\n",
)
replace_once(
    main,
    "fn static_default_spawn_position() -> DefaultSpawnPosition {\n"
    "    DefaultSpawnPosition {\n"
    "        position: GlobalPosition {\n"
    "            dimension: STATIC_LEVEL.to_owned(),\n"
    "            position: BlockPosition { x: 0, y: 64, z: 0 },\n"
    "        },\n"
    "        yaw: 0.0,\n"
    "        pitch: 0.0,\n"
    "    }\n"
    "}\n",
    "fn static_default_spawn_position(world: &RomPackWorld) -> Result<DefaultSpawnPosition> {\n"
    "    Ok(DefaultSpawnPosition {\n"
    "        position: GlobalPosition {\n"
    "            dimension: world.dimension.clone(),\n"
    "            position: BlockPosition {\n"
    "                x: world.spawn_x,\n"
    "                y: world\n"
    "                    .floor_y\n"
    "                    .checked_add(1)\n"
    "                    .context(\"default spawn y overflow\")?,\n"
    "                z: world.spawn_z,\n"
    "            },\n"
    "        },\n"
    "        yaw: 0.0,\n"
    "        pitch: 0.0,\n"
    "    })\n"
    "}\n",
)
replace_once(
    main,
    "fn static_player_position() -> PlayerPosition {\n"
    "    PlayerPosition {\n"
    "        teleport_id: STATIC_TELEPORT_ID,\n"
    "        change: PositionMoveRotation {\n"
    "            position: [0.5, 65.0, 0.5],\n"
    "            delta_movement: [0.0; 3],\n"
    "            yaw: 0.0,\n"
    "            pitch: 0.0,\n"
    "        },\n"
    "        relative_flags: 0,\n"
    "    }\n"
    "}\n",
    "fn static_player_position(world: &RomPackWorld) -> PlayerPosition {\n"
    "    PlayerPosition {\n"
    "        teleport_id: STATIC_TELEPORT_ID,\n"
    "        change: PositionMoveRotation {\n"
    "            position: play_runtime::player_spawn_position(world),\n"
    "            delta_movement: [0.0; 3],\n"
    "            yaw: 0.0,\n"
    "            pitch: 0.0,\n"
    "        },\n"
    "        relative_flags: 0,\n"
    "    }\n"
    "}\n",
)
# Exact-byte integration test expectations now use the generated world profile.
replace_once(
    main,
    "        let join_game = read_packet(&mut cursor).unwrap();\n",
    "        let world_profile = play_runtime::builtin_world_profile();\n"
    "        let join_game = read_packet(&mut cursor).unwrap();\n",
)
replace_once(
    main,
    "            encode_join_game(&static_join_game(&config)).unwrap()\n",
    "            encode_join_game(&static_join_game(&config, &world_profile)).unwrap()\n",
)
replace_once(
    main,
    "            encode_default_spawn_position(&static_default_spawn_position()).unwrap()\n",
    "            encode_default_spawn_position(\n"
    "                &static_default_spawn_position(&world_profile).unwrap(),\n"
    "            )\n"
    "            .unwrap()\n",
)
replace_once(
    main,
    "            encode_player_position(&static_player_position()).unwrap()\n",
    "            encode_player_position(&static_player_position(&world_profile)).unwrap()\n",
)
replace_once(
    main,
    "    #[test]\n    fn generated_registry_manifest_drives_configuration_payloads() {\n",
    "    #[test]\n"
    "    fn generated_play_metadata_drives_join_and_spawn_payloads() {\n"
    "        let config = ServerConfig::for_profile(Some(\"26.1.2\")).unwrap();\n"
    "        let mut world = play_runtime::builtin_world_profile();\n"
    "        world.dimension = \"minecraft:test_world\".to_owned();\n"
    "        world.dimension_type_id = 3;\n"
    "        world.sea_level = 70;\n"
    "        world.floor_y = 79;\n"
    "        world.spawn_x = 32;\n"
    "        world.spawn_z = -17;\n"
    "        let join = static_join_game(&config, &world);\n"
    "        assert_eq!(join.levels, [\"minecraft:test_world\"]);\n"
    "        assert_eq!(join.spawn_info.dimension_type_id, 3);\n"
    "        assert_eq!(join.spawn_info.dimension, \"minecraft:test_world\");\n"
    "        assert_eq!(join.spawn_info.sea_level, 70);\n"
    "        let spawn = static_default_spawn_position(&world).unwrap();\n"
    "        assert_eq!(spawn.position.dimension, \"minecraft:test_world\");\n"
    "        assert_eq!(spawn.position.position, BlockPosition { x: 32, y: 80, z: -17 });\n"
    "        assert_eq!(\n"
    "            static_player_position(&world).change.position,\n"
    "            [32.5, 81.0, -16.5]\n"
    "        );\n"
    "        assert_eq!(play_runtime::spawn_chunk(&world), ChunkPos { x: 2, z: -2 });\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn generated_registry_manifest_drives_configuration_payloads() {\n",
)

play = "crates/ferrum-server/src/play_runtime.rs"
replace_once(
    play,
    "use super::{\n"
    "    KEEP_ALIVE_INTERVAL, MAX_IGNORED_PLAY_PACKETS, STATIC_CHUNK_RADIUS, STATIC_CHUNK_X,\n"
    "    STATIC_CHUNK_Z, STATIC_FLOOR_Y, is_connection_eof, is_transient_read_timeout, version_26_1_2,\n"
    "    write_play_payload,\n"
    "};\n",
    "use super::{\n"
    "    KEEP_ALIVE_INTERVAL, MAX_IGNORED_PLAY_PACKETS, STATIC_CHUNK_RADIUS, is_connection_eof,\n"
    "    is_transient_read_timeout, version_26_1_2, write_play_payload,\n"
    "};\n",
)
replace_once(
    play,
    "const MIN_PLAYER_FEET_Y: f64 = STATIC_FLOOR_Y as f64 + 1.0;\n",
    "",
)
replace_once(
    play,
    "        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,\n"
    "        block_states: RomPackBlockStates {\n",
    "        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,\n"
    "        dimension: version_26_1_2::OVERWORLD_DIMENSION.to_owned(),\n"
    "        dimension_type_id: version_26_1_2::OVERWORLD_DIMENSION_TYPE_ID,\n"
    "        sea_level: version_26_1_2::OVERWORLD_SEA_LEVEL,\n"
    "        floor_y: version_26_1_2::FLAT_WORLD_FLOOR_Y,\n"
    "        spawn_x: version_26_1_2::FLAT_WORLD_SPAWN_X,\n"
    "        spawn_z: version_26_1_2::FLAT_WORLD_SPAWN_Z,\n"
    "        block_states: RomPackBlockStates {\n",
)
replace_once(
    play,
    "    pub(super) fn new(center: ChunkPos, profile: RomPackWorld) -> Result<Self> {\n"
    "        Ok(Self {\n"
    "            profile,\n"
    "            inner: Mutex::new(SharedWorldInner {\n"
    "                runtime: new_local_world_runtime(center, profile)?,\n",
    "    pub(super) fn new(center: ChunkPos, profile: RomPackWorld) -> Result<Self> {\n"
    "        let runtime = new_local_world_runtime(center, &profile)?;\n"
    "        Ok(Self {\n"
    "            profile,\n"
    "            inner: Mutex::new(SharedWorldInner {\n"
    "                runtime,\n",
)
replace_once(
    play,
    "        Self::new(\n"
    "            ChunkPos {\n"
    "                x: STATIC_CHUNK_X,\n"
    "                z: STATIC_CHUNK_Z,\n"
    "            },\n"
    "            builtin_world_profile(),\n"
    "        )\n",
    "        let profile = builtin_world_profile();\n"
    "        let center = spawn_chunk(&profile);\n"
    "        Self::new(center, profile)\n",
)
replace_once(
    play,
    "        ensure_chunks_loaded(inner.runtime.state_mut(), positions, self.profile)\n",
    "        ensure_chunks_loaded(inner.runtime.state_mut(), positions, &self.profile)\n",
)
replace_once(
    play,
    "    pub(super) const fn world_profile(&self) -> RomPackWorld {\n"
    "        self.profile\n"
    "    }\n",
    "    pub(super) fn world_profile(&self) -> &RomPackWorld {\n"
    "        &self.profile\n"
    "    }\n",
)
replace_once(
    play,
    "    let mut player = PlayerState::new([0.5, 65.0, 0.5], 0.0, 0.0, false, false)?;\n"
    "    let mut view = ChunkView::new(\n"
    "        ChunkPos {\n"
    "            x: STATIC_CHUNK_X,\n"
    "            z: STATIC_CHUNK_Z,\n"
    "        },\n"
    "        STATIC_CHUNK_RADIUS,\n"
    "    )?;\n",
    "    let world_profile = shared_world.world_profile();\n"
    "    let mut player = PlayerState::new(\n"
    "        player_spawn_position(world_profile),\n"
    "        0.0,\n"
    "        0.0,\n"
    "        false,\n"
    "        false,\n"
    "    )?;\n"
    "    let mut view = ChunkView::new(spawn_chunk(world_profile), STATIC_CHUNK_RADIUS)?;\n",
)
replace_once(
    play,
    "                    validate_movement_floor(movement)?;\n",
    "                    validate_movement_floor(movement, world_profile.floor_y)?;\n",
)
replace_once(
    play,
    "fn new_local_world_runtime(center: ChunkPos, profile: RomPackWorld) -> Result<LocalWorldRuntime> {\n"
    "    let mut store = ChunkStore::new();\n"
    "    seed_chunk_square(&mut store, center, STATIC_CHUNK_RADIUS, profile)?;\n",
    "fn new_local_world_runtime(\n"
    "    center: ChunkPos,\n"
    "    profile: &RomPackWorld,\n"
    ") -> Result<LocalWorldRuntime> {\n"
    "    let mut store = ChunkStore::new();\n"
    "    seed_chunk_square(&mut store, center, STATIC_CHUNK_RADIUS, profile)?;\n",
)
replace_once(
    play,
    "    profile: RomPackWorld,\n"
    ") -> Result<()> {\n",
    "    profile: &RomPackWorld,\n"
    ") -> Result<()> {\n",
)
replace_once(
    play,
    "    profile: RomPackWorld,\n"
    ") -> Result<()> {\n",
    "    profile: &RomPackWorld,\n"
    ") -> Result<()> {\n",
)
replace_once(
    play,
    "fn validate_movement_floor(movement: PlayerMovement) -> Result<()> {\n"
    "    let Some(next_position) = movement_position(movement) else {\n"
    "        return Ok(());\n"
    "    };\n"
    "    if next_position[1] < MIN_PLAYER_FEET_Y {\n"
    "        bail!(\n"
    "            \"player movement feet y {} is below flat-world floor {}\",\n"
    "            next_position[1],\n"
    "            MIN_PLAYER_FEET_Y\n"
    "        );\n"
    "    }\n"
    "    Ok(())\n"
    "}\n",
    "fn validate_movement_floor(movement: PlayerMovement, floor_y: i32) -> Result<()> {\n"
    "    let Some(next_position) = movement_position(movement) else {\n"
    "        return Ok(());\n"
    "    };\n"
    "    let minimum_feet_y = f64::from(floor_y) + 1.0;\n"
    "    if next_position[1] < minimum_feet_y {\n"
    "        bail!(\n"
    "            \"player movement feet y {} is below flat-world floor {}\",\n"
    "            next_position[1],\n"
    "            minimum_feet_y\n"
    "        );\n"
    "    }\n"
    "    Ok(())\n"
    "}\n",
)
replace_once(
    play,
    "fn flat_chunk(pos: ChunkPos, profile: RomPackWorld) -> Result<StaticChunk> {\n",
    "fn flat_chunk(pos: ChunkPos, profile: &RomPackWorld) -> Result<StaticChunk> {\n",
)
replace_once(
    play,
    "            floor_y: STATIC_FLOOR_Y,\n",
    "            floor_y: profile.floor_y,\n",
)
replace_once(
    play,
    "#[cfg(test)]\nmod tests {\n",
    "pub(super) fn spawn_chunk(profile: &RomPackWorld) -> ChunkPos {\n"
    "    ChunkPos {\n"
    "        x: profile.spawn_x.div_euclid(16),\n"
    "        z: profile.spawn_z.div_euclid(16),\n"
    "    }\n"
    "}\n\n"
    "pub(super) fn player_spawn_position(profile: &RomPackWorld) -> [f64; 3] {\n"
    "    [\n"
    "        f64::from(profile.spawn_x) + 0.5,\n"
    "        f64::from(profile.floor_y) + 2.0,\n"
    "        f64::from(profile.spawn_z) + 0.5,\n"
    "    ]\n"
    "}\n\n"
    "#[cfg(test)]\n"
    "mod tests {\n",
)
# Borrow temporary built-in profiles in tests.
text_path = Path(play)
text = text_path.read_text(encoding="utf-8")
text = text.replace(
    "new_local_world_runtime(ChunkPos { x: 0, z: 0 }, builtin_world_profile())",
    "new_local_world_runtime(ChunkPos { x: 0, z: 0 }, &builtin_world_profile())",
)
text_path.write_text(text, encoding="utf-8")
replace_once(
    play,
    "        validate_movement_floor(PlayerMovement::Position {\n"
    "            position: [0.5, MIN_PLAYER_FEET_Y, 0.5],\n",
    "        let floor_y = builtin_world_profile().floor_y;\n"
    "        let minimum_feet_y = f64::from(floor_y) + 1.0;\n"
    "        validate_movement_floor(PlayerMovement::Position {\n"
    "            position: [0.5, minimum_feet_y, 0.5],\n",
)
replace_once(
    play,
    "        })\n        .unwrap();\n        validate_movement_floor(PlayerMovement::StatusOnly {\n",
    "        }, floor_y)\n"
    "        .unwrap();\n"
    "        validate_movement_floor(PlayerMovement::StatusOnly {\n",
)
replace_once(
    play,
    "        })\n        .unwrap();\n\n        let error = validate_movement_floor(PlayerMovement::Position {\n"
    "            position: [0.5, MIN_PLAYER_FEET_Y - 0.01, 0.5],\n",
    "        }, floor_y)\n"
    "        .unwrap();\n\n"
    "        let error = validate_movement_floor(PlayerMovement::Position {\n"
    "            position: [0.5, minimum_feet_y - 0.01, 0.5],\n",
)
replace_once(
    play,
    "        })\n        .unwrap_err();\n        assert!(error.to_string().contains(\"below flat-world floor\"));\n",
    "        }, floor_y)\n"
    "        .unwrap_err();\n"
    "        assert!(error.to_string().contains(\"below flat-world floor\"));\n",
)

# Documentation and migration notes.
replace_once(
    "README.md",
    "- Packet IDs, world height, flat-world block-state IDs, biome ID, and Configuration registry payloads loaded from the generated schema-v3 `.rompack` during Bootstrap startup\n",
    "- Packet IDs, world height, dimension/bootstrap metadata, flat-world block-state IDs, biome ID, and Configuration registry payloads loaded from the generated schema-v4 `.rompack` during Bootstrap startup\n",
)
replace_once(
    "README.md",
    "writes an integrity-protected schema-v3 `.rompack`. Existing schema-v1/v2 packs must be regenerated with `generate --force`.\n",
    "writes an integrity-protected schema-v4 `.rompack`. Existing schema-v1/v2/v3 packs must be regenerated with `generate --force`.\n",
)
replace_once(
    "README.md",
    "- Runtime replacement of remaining gameplay constants with generated pack data\n",
    "- Runtime replacement of remaining server-policy and gameplay constants with explicit runtime configuration\n",
)
replace_once(
    "README.md",
    "1. Move remaining gameplay constants into generated packs\n",
    "1. Move remaining server-policy constants into explicit runtime configuration\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "10. Add the world data version, overworld section range, required flat-world block-state IDs, and plains biome ID.\n"
    "11. Write a deterministic schema-v3 `.rompack` with a container SHA-256 trailer and provenance metadata.\n"
    "12. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile`, Configuration registry payloads, and initial shared world from pack metadata.\n",
    "10. Add the world data version, overworld section range, required flat-world block-state IDs, and plains biome ID.\n"
    "11. Add the dimension ID, dimension-type ID, sea level, flat floor, and deterministic spawn coordinates consumed by Play bootstrap.\n"
    "12. Write a deterministic schema-v4 `.rompack` with a container SHA-256 trailer and provenance metadata.\n"
    "13. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile`, Configuration registry payloads, Join Game metadata, spawn packets, movement floor, and initial shared world from pack metadata.\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "Schema-v1 and schema-v2 packs are intentionally rejected after the packet-table and world-metadata migrations and must be regenerated.\n",
    "Schema-v1, schema-v2, and schema-v3 packs are intentionally rejected after the packet-table, world-metadata, and Play-bootstrap migrations and must be regenerated.\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "1. Move remaining gameplay constants into generated packs.\n",
    "1. Move remaining server-policy constants into explicit runtime configuration.\n",
)
