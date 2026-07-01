from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


main = "crates/ferrum-server/src/main.rs"
replace_once(
    main,
    '''const STATIC_PLAYER_ID: i32 = 1;
const STATIC_TELEPORT_ID: i32 = 1;
const STATIC_CHUNK_RADIUS: i32 = 1;
const STATIC_SIMULATION_DISTANCE: i32 = 2;
const STATIC_CHUNK_BATCH_SIZE: i32 = 1;
const STATIC_WELCOME_MESSAGE: &str = "Ferrum native Rust world loaded";
const MAX_CONFIGURATION_AUXILIARY_PACKETS: usize = 16;
const MAX_IGNORED_PLAY_PACKETS: usize = 1_024;
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);
''',
    '''const STATIC_PLAYER_ID: i32 = 1;
const STATIC_TELEPORT_ID: i32 = 1;
const DEFAULT_CHUNK_RADIUS: i32 = 1;
const DEFAULT_SIMULATION_DISTANCE: i32 = 2;
const DEFAULT_WELCOME_MESSAGE: &str = "Ferrum native Rust world loaded";
const DEFAULT_KEEP_ALIVE_INTERVAL_SECONDS: u64 = 15;
const MAX_CONFIGURED_CHUNK_RADIUS: i32 = 8;
const MAX_CONFIGURED_SIMULATION_DISTANCE: i32 = 32;
const MAX_KEEP_ALIVE_INTERVAL_SECONDS: u64 = 300;
const MAX_WELCOME_MESSAGE_BYTES: usize = 256;
const MAX_CONFIGURATION_AUXILIARY_PACKETS: usize = 16;
const MAX_IGNORED_PLAY_PACKETS: usize = 1_024;
''',
)
replace_once(
    main,
    '''    sample_players: Vec<SamplePlayer>,
    packets: PacketIds,
    runtime_profile: Option<ProtocolProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SamplePlayer {
''',
    '''    sample_players: Vec<SamplePlayer>,
    play_policy: PlayPolicy,
    packets: PacketIds,
    runtime_profile: Option<ProtocolProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlayPolicy {
    chunk_radius: i32,
    simulation_distance: i32,
    welcome_message: String,
    keep_alive_interval_seconds: u64,
}

impl Default for PlayPolicy {
    fn default() -> Self {
        Self {
            chunk_radius: DEFAULT_CHUNK_RADIUS,
            simulation_distance: DEFAULT_SIMULATION_DISTANCE,
            welcome_message: DEFAULT_WELCOME_MESSAGE.to_owned(),
            keep_alive_interval_seconds: DEFAULT_KEEP_ALIVE_INTERVAL_SECONDS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SamplePlayer {
''',
)
replace_once(
    main,
    '''        Self::with_runtime(
            config.online_players,
            play_runtime::builtin_world_profile(),
            registry_payloads,
        )
''',
    '''        Self::with_runtime(
            config.online_players,
            play_runtime::builtin_world_profile(),
            registry_payloads,
            config.play_policy.clone(),
        )
''',
)
replace_once(
    main,
    '''    fn with_runtime(
        initial_online_players: i32,
        world: RomPackWorld,
        registry_payloads: Vec<Vec<u8>>,
    ) -> Result<Self> {
''',
    '''    fn with_runtime(
        initial_online_players: i32,
        world: RomPackWorld,
        registry_payloads: Vec<Vec<u8>>,
        play_policy: PlayPolicy,
    ) -> Result<Self> {
''',
)
replace_once(
    main,
    '''                play_runtime::SharedWorld::new(center, world)?
''',
    '''                play_runtime::SharedWorld::new_with_policy(center, world, play_policy)?
''',
)
replace_once(
    main,
    '''        world_profile,
        registry_payloads,
    )?);
''',
    '''        world_profile,
        registry_payloads,
        config.play_policy.clone(),
    )?);
''',
)
replace_once(
    main,
    '''            server_icon: None,
            sample_players: Vec::new(),
            packets: PacketIds::default(),
''',
    '''            server_icon: None,
            sample_players: Vec::new(),
            play_policy: PlayPolicy::default(),
            packets: PacketIds::default(),
''',
)
replace_once(
    main,
    '''                ("server", "allow_offline_login") => {
                    config.allow_offline_login = parse_bool(value, line_index + 1)?
                }
                ("configuration", "enabled") => {
''',
    '''                ("server", "allow_offline_login") => {
                    config.allow_offline_login = parse_bool(value, line_index + 1)?
                }
                ("play", "chunk_radius") => {
                    config.play_policy.chunk_radius = parse_i32(value, line_index + 1)?
                }
                ("play", "simulation_distance") => {
                    config.play_policy.simulation_distance = parse_i32(value, line_index + 1)?
                }
                ("play", "welcome_message") => {
                    config.play_policy.welcome_message = parse_string(value)
                }
                ("play", "keep_alive_interval_seconds") => {
                    config.play_policy.keep_alive_interval_seconds =
                        parse_u64(value, line_index + 1)?
                }
                ("configuration", "enabled") => {
''',
)
replace_once(
    main,
    '''        if config.online_players > config.max_players {
            bail!("online_players cannot exceed max_players");
        }
        if config.configuration_enabled && config.profile_name.is_none() {
''',
    '''        if config.online_players > config.max_players {
            bail!("online_players cannot exceed max_players");
        }
        if !(0..=MAX_CONFIGURED_CHUNK_RADIUS).contains(&config.play_policy.chunk_radius) {
            bail!(
                "play.chunk_radius must be between 0 and {MAX_CONFIGURED_CHUNK_RADIUS}"
            );
        }
        if !(0..=MAX_CONFIGURED_SIMULATION_DISTANCE)
            .contains(&config.play_policy.simulation_distance)
        {
            bail!(
                "play.simulation_distance must be between 0 and {MAX_CONFIGURED_SIMULATION_DISTANCE}"
            );
        }
        if !(1..=MAX_KEEP_ALIVE_INTERVAL_SECONDS)
            .contains(&config.play_policy.keep_alive_interval_seconds)
        {
            bail!(
                "play.keep_alive_interval_seconds must be between 1 and {MAX_KEEP_ALIVE_INTERVAL_SECONDS}"
            );
        }
        if config.play_policy.welcome_message.len() > MAX_WELCOME_MESSAGE_BYTES
            || config
                .play_policy
                .welcome_message
                .chars()
                .any(char::is_control)
        {
            bail!(
                "play.welcome_message must contain at most {MAX_WELCOME_MESSAGE_BYTES} bytes and no control characters"
            );
        }
        if config.configuration_enabled && config.profile_name.is_none() {
''',
)
replace_once(
    main,
    '''fn parse_bool(value: &str, line: usize) -> Result<bool> {
''',
    '''fn parse_u64(value: &str, line: usize) -> Result<u64> {
    value
        .parse()
        .with_context(|| format!("line {line} is not a valid u64: {value}"))
}

fn parse_bool(value: &str, line: usize) -> Result<bool> {
''',
)
replace_once(
    main,
    '''        &encode_chunk_batch_finished(STATIC_CHUNK_BATCH_SIZE)?,
    )?;
    write_play_payload(
        writer,
        profile,
        PacketKind::SystemChat,
        &encode_system_chat(STATIC_WELCOME_MESSAGE, false)?,
    )?;
''',
    '''        &encode_chunk_batch_finished(1)?,
    )?;
    if !config.play_policy.welcome_message.is_empty() {
        write_play_payload(
            writer,
            profile,
            PacketKind::SystemChat,
            &encode_system_chat(&config.play_policy.welcome_message, false)?,
        )?;
    }
''',
)
replace_once(
    main,
    '''        chunk_radius: STATIC_CHUNK_RADIUS,
        simulation_distance: STATIC_SIMULATION_DISTANCE,
''',
    '''        chunk_radius: config.play_policy.chunk_radius,
        simulation_distance: config.play_policy.simulation_distance,
''',
)
# Exact-byte tests still expect the default welcome message.
path = Path(main)
text = path.read_text(encoding="utf-8")
text = text.replace("STATIC_WELCOME_MESSAGE", "DEFAULT_WELCOME_MESSAGE")
path.write_text(text, encoding="utf-8")

replace_once(
    main,
    '''        assert_eq!(play_runtime::spawn_chunk(&world), ChunkPos { x: 2, z: -2 });
    }

    #[test]
    fn generated_registry_manifest_drives_configuration_payloads() {
''',
    '''        assert_eq!(play_runtime::spawn_chunk(&world), ChunkPos { x: 2, z: -2 });
    }

    #[test]
    fn configured_play_policy_drives_join_game_fields() {
        let mut config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        config.play_policy.chunk_radius = 4;
        config.play_policy.simulation_distance = 6;
        let world = play_runtime::builtin_world_profile();
        let join = static_join_game(&config, &world);
        assert_eq!(join.chunk_radius, 4);
        assert_eq!(join.simulation_distance, 6);
    }

    #[test]
    fn parses_and_validates_play_policy_configuration() {
        let config = ServerConfig::from_toml_like_with_base(
            r#"
            [server]
            profile = "26.1.2"

            [play]
            chunk_radius = 3
            simulation_distance = 7
            welcome_message = "Welcome to RoM"
            keep_alive_interval_seconds = 30
            "#,
            None,
        )
        .unwrap();
        assert_eq!(config.play_policy.chunk_radius, 3);
        assert_eq!(config.play_policy.simulation_distance, 7);
        assert_eq!(config.play_policy.welcome_message, "Welcome to RoM");
        assert_eq!(config.play_policy.keep_alive_interval_seconds, 30);

        for invalid in [
            "[play]\\nchunk_radius = 9",
            "[play]\\nsimulation_distance = 33",
            "[play]\\nkeep_alive_interval_seconds = 0",
        ] {
            assert!(ServerConfig::from_toml_like_with_base(invalid, None).is_err());
        }
    }

    #[test]
    fn generated_registry_manifest_drives_configuration_payloads() {
''',
)

play = "crates/ferrum-server/src/play_runtime.rs"
replace_once(
    play,
    '''use super::{
    KEEP_ALIVE_INTERVAL, MAX_IGNORED_PLAY_PACKETS, STATIC_CHUNK_RADIUS, is_connection_eof,
    is_transient_read_timeout, version_26_1_2, write_play_payload,
};
''',
    '''use super::{
    MAX_IGNORED_PLAY_PACKETS, PlayPolicy, is_connection_eof, is_transient_read_timeout,
    version_26_1_2, write_play_payload,
};
''',
)
replace_once(
    play,
    '''pub(super) struct SharedWorld {
    profile: RomPackWorld,
    inner: Mutex<SharedWorldInner>,
}
''',
    '''pub(super) struct SharedWorld {
    profile: RomPackWorld,
    play_policy: PlayPolicy,
    inner: Mutex<SharedWorldInner>,
}
''',
)
replace_once(
    play,
    '''impl SharedWorld {
    pub(super) fn new(center: ChunkPos, profile: RomPackWorld) -> Result<Self> {
        let runtime = new_local_world_runtime(center, &profile)?;
        Ok(Self {
            profile,
            inner: Mutex::new(SharedWorldInner {
                runtime,
                tick: Tick::ZERO,
                subscribers: BTreeMap::new(),
            }),
        })
    }
''',
    '''impl SharedWorld {
    pub(super) fn new(center: ChunkPos, profile: RomPackWorld) -> Result<Self> {
        Self::new_with_policy(center, profile, PlayPolicy::default())
    }

    pub(super) fn new_with_policy(
        center: ChunkPos,
        profile: RomPackWorld,
        play_policy: PlayPolicy,
    ) -> Result<Self> {
        let runtime = new_local_world_runtime_with_radius(
            center,
            &profile,
            play_policy.chunk_radius,
        )?;
        Ok(Self {
            profile,
            play_policy,
            inner: Mutex::new(SharedWorldInner {
                runtime,
                tick: Tick::ZERO,
                subscribers: BTreeMap::new(),
            }),
        })
    }
''',
)
replace_once(
    play,
    '''    pub(super) fn world_profile(&self) -> &RomPackWorld {
        &self.profile
    }

    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {
''',
    '''    pub(super) fn world_profile(&self) -> &RomPackWorld {
        &self.profile
    }

    pub(super) fn play_policy(&self) -> &PlayPolicy {
        &self.play_policy
    }

    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {
''',
)
replace_once(
    play,
    '''    let world_profile = shared_world.world_profile();
    let mut player =
        PlayerState::new(player_spawn_position(world_profile), 0.0, 0.0, false, false)?;
    let mut view = ChunkView::new(spawn_chunk(world_profile), STATIC_CHUNK_RADIUS)?;
''',
    '''    let world_profile = shared_world.world_profile();
    let play_policy = shared_world.play_policy();
    let mut player =
        PlayerState::new(player_spawn_position(world_profile), 0.0, 0.0, false, false)?;
    let mut view = ChunkView::new(spawn_chunk(world_profile), play_policy.chunk_radius)?;
''',
)
replace_once(
    play,
    '''    let tick_interval = usize::try_from(KEEP_ALIVE_INTERVAL.as_secs())
        .context("keep alive interval exceeds usize")?
        .checked_mul(CLIENT_TICKS_PER_SECOND)
        .context("keep alive tick interval overflow")?;
''',
    '''    let tick_interval = keep_alive_tick_interval(play_policy)?;
''',
)
replace_once(
    play,
    '''fn new_local_world_runtime(center: ChunkPos, profile: &RomPackWorld) -> Result<LocalWorldRuntime> {
    let mut store = ChunkStore::new();
    seed_chunk_square(&mut store, center, STATIC_CHUNK_RADIUS, profile)?;
    Ok(DeterministicRuntime::new(
''',
    '''fn keep_alive_tick_interval(play_policy: &PlayPolicy) -> Result<usize> {
    usize::try_from(play_policy.keep_alive_interval_seconds)
        .context("keep alive interval exceeds usize")?
        .checked_mul(CLIENT_TICKS_PER_SECOND)
        .context("keep alive tick interval overflow")
}

fn new_local_world_runtime(center: ChunkPos, profile: &RomPackWorld) -> Result<LocalWorldRuntime> {
    new_local_world_runtime_with_radius(center, profile, PlayPolicy::default().chunk_radius)
}

fn new_local_world_runtime_with_radius(
    center: ChunkPos,
    profile: &RomPackWorld,
    chunk_radius: i32,
) -> Result<LocalWorldRuntime> {
    let mut store = ChunkStore::new();
    seed_chunk_square(&mut store, center, chunk_radius, profile)?;
    Ok(DeterministicRuntime::new(
''',
)
replace_once(
    play,
    '''    #[test]
    fn generated_world_profile_drives_chunk_layout_and_block_states() {
''',
    '''    #[test]
    fn configured_play_policy_controls_loaded_radius_and_keep_alive_cadence() {
        let policy = PlayPolicy {
            chunk_radius: 2,
            keep_alive_interval_seconds: 3,
            ..PlayPolicy::default()
        };
        let profile = builtin_world_profile();
        let world = SharedWorld::new_with_policy(ChunkPos { x: 0, z: 0 }, profile, policy.clone())
            .unwrap();
        assert!(world.chunk_snapshot(ChunkPos { x: 2, z: 2 }).is_ok());
        assert!(world.chunk_snapshot(ChunkPos { x: 3, z: 0 }).is_err());
        assert_eq!(world.play_policy(), &policy);
        assert_eq!(keep_alive_tick_interval(&policy).unwrap(), 60);
    }

    #[test]
    fn generated_world_profile_drives_chunk_layout_and_block_states() {
''',
)

replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    '''previews_chat = false

[configuration]
''',
    '''previews_chat = false

[play]
chunk_radius = 1
simulation_distance = 2
welcome_message = "Ferrum native Rust world loaded"
keep_alive_interval_seconds = 15

[configuration]
''',
)
replace_once(
    "examples/server-26.1.2.toml",
    '''previews_chat = false

[configuration]
''',
    '''previews_chat = false

[play]
chunk_radius = 1
simulation_distance = 2
welcome_message = "Ferrum native Rust world loaded"
keep_alive_interval_seconds = 15

[configuration]
''',
)

replace_once(
    "README.md",
    '''```toml
bind = "127.0.0.1:25565"
online_mode = false
```

Connect a matching Minecraft Java Edition 26.1.2 client''',
    '''```toml
[server]
bind = "127.0.0.1:25565"
online_mode = false

[play]
chunk_radius = 1
simulation_distance = 2
welcome_message = "Ferrum native Rust world loaded"
keep_alive_interval_seconds = 15
```

`chunk_radius` is bounded to `0..=8`, `simulation_distance` to `0..=32`, and the Keep Alive interval to `1..=300` seconds. An empty welcome message disables the initial system-chat message.

Connect a matching Minecraft Java Edition 26.1.2 client''',
)
replace_once(
    "README.md",
    '''1. Move remaining server-policy constants into explicit runtime configuration
2. Wire dedicated network workers into the authoritative 20 TPS runtime
3. Add full block interaction and inventory validation
4. Add entities and entity tracking
5. Add persistent Anvil region loading and saving
6. Add Microsoft account authentication and encrypted online mode
7. Add additional Minecraft version profiles
''',
    '''1. Wire dedicated network workers into the authoritative 20 TPS runtime
2. Add full block interaction and inventory validation
3. Add entities and entity tracking
4. Add persistent Anvil region loading and saving
5. Add Microsoft account authentication and encrypted online mode
6. Add additional Minecraft version profiles
''',
)

roadmap = "docs/SERVER_ROADMAP.md"
replace_once(
    roadmap,
    '''- Deterministic 3×3 chunk views with cache-center updates, new chunk batches, and unloads
''',
    '''- Bounded configurable chunk views with cache-center updates, new chunk batches, and unloads
- Explicit `[play]` runtime policy for chunk radius, simulation distance, welcome chat, and Keep Alive cadence
''',
)
replace_once(
    roadmap,
    '''The server tracks each player's authoritative position and rotation, updates the chunk-cache center only after crossing a chunk boundary, sends newly visible chunks, unloads chunks that leave the 3×3 view, keeps the server-list online-player count synchronized with Play connections, and continues Keep Alive validation while processing movement.''',
    '''The server tracks each player's authoritative position and rotation, updates the chunk-cache center only after crossing a chunk boundary, sends newly visible chunks, unloads chunks that leave the configured bounded view, keeps the server-list online-player count synchronized with Play connections, and continues Keep Alive validation while processing movement at the configured cadence.''',
)
replace_once(
    roadmap,
    '''- Send a welcome system message and a graceful Play disconnect on bootstrap failure.
''',
    '''- Send an optional configured welcome system message and a graceful Play disconnect on bootstrap failure.
''',
)
replace_once(
    roadmap,
    '''- Keep a 3×3 visible chunk set centered on the player's current chunk.
''',
    '''- Keep a bounded `(2r+1)×(2r+1)` visible chunk set centered on the player's current chunk, with `r` selected by runtime policy and defaulting to 1.
''',
)
replace_once(
    roadmap,
    '''## M14 — Authoritative tick/runtime foundation
''',
    '''## Cross-cutting Play runtime policy

Status: complete for the current static-world runtime.

- Parse a dedicated `[play]` section from `server.toml`.
- Configure chunk radius, Join Game simulation distance, optional welcome chat, and Keep Alive interval.
- Use the same chunk-radius policy for initial world seeding and per-player chunk views.
- Bound chunk radius to `0..=8`, simulation distance to `0..=32`, Keep Alive interval to `1..=300` seconds, and welcome text to 256 bytes without control characters.
- Preserve the previous behavior as explicit defaults in Bootstrap and example configurations.

## M14 — Authoritative tick/runtime foundation
''',
)

replace_once(
    "docs/BOOTSTRAP.md",
    '''## Public server warning
''',
    '''## Runtime Play policy

Bootstrap writes an explicit bounded Play policy into new `server.toml` files:

```toml
[play]
chunk_radius = 1
simulation_distance = 2
welcome_message = "Ferrum native Rust world loaded"
keep_alive_interval_seconds = 15
```

The chunk radius controls both initial in-memory chunk seeding and each player's visible chunk view. Existing instance configurations remain valid because the native server supplies the same defaults when `[play]` is absent.

## Public server warning
''',
)
replace_once(
    "docs/BOOTSTRAP.md",
    '''1. Move remaining server-policy constants into explicit runtime configuration.
2. Add more independently testable extractors only when the server consumes their output.
3. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.
''',
    '''1. Add more independently testable extractors only when the server consumes their output.
2. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.
''',
)
