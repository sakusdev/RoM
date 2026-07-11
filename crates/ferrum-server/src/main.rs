use anyhow::{Context, Result, bail};
use clap::Parser;
mod codec;
mod identity;
mod play_runtime;
#[cfg(test)]
use codec::read_varint_io;
use codec::{
    PacketReader, build_packet, read_packet, write_packet, write_string, write_varint_vec,
};
use ferrum_configuration::{
    KnownPack, KnownPackDecodeLimits, RegistryData, RegistryEntry, decode_client_information,
    decode_known_packs, encode_feature_flags, encode_known_packs, encode_registry_data,
    encode_tags,
};
use ferrum_game::{GameState, PlayerUuid as GamePlayerUuid, Transform};
use ferrum_nbt::{Tag, encode_anonymous};
use ferrum_play::{
    BlockPosition, CommonPlayerSpawnInfo, DefaultSpawnPosition, GlobalPosition, JoinGame,
    PlayerPosition, PositionMoveRotation, encode_chunk_batch_finished, encode_chunk_batch_start,
    encode_default_spawn_position, encode_join_game, encode_level_chunk_with_light,
    encode_play_disconnect, encode_player_position, encode_set_chunk_cache_center,
    encode_system_chat,
};
use ferrum_protocol::{HandshakeIntent, PacketKind, PacketTable, ProtocolProfile, ProtocolSession};
use ferrum_rompack::{RomPack, RomPackPacket, RomPackRegistry, RomPackWorld, read_rompack};
use ferrum_runtime::ConnectionId;
use ferrum_server::{
    authoritative_runtime::{PlayInput, PlayOutput},
    game_runtime::SharedGameRuntime,
    play_connection::{PlayReaderEndpoint, PlayWriterEndpoint, register_play_connection},
    play_writer::{PlayWriterDirective, PlayWriterWorker, spawn_play_writer},
    shared_runtime::{SharedPlayRuntime, SharedPlayRuntimeConfig, spawn_shared_play_runtime},
    shared_writer::SharedWriter,
};
use ferrum_version_26_1_2 as version_26_1_2;
use ferrum_world::{
    BiomeId, BlockStateId, ChunkStore,
    anvil::{
        AnvilChunkConversionProfile, AnvilDecodeLimits, RegionHeader, RegionPos,
        load_chunk_store_from_region_lenient,
    },
};
#[cfg(test)]
use ferrum_world::{BlockMutation, BlockPos, ChunkPos, StaticChunk};
use identity::{PlayerIdentity, offline_player_identity};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI32, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

const DEFAULT_BIND: &str = "127.0.0.1:25565";
const DEFAULT_VERSION_NAME: &str = "Minecraft Java Edition 26.*.*";
const DEFAULT_PROTOCOL: i32 = 0;
const DEFAULT_MOTD: &str = "Ferrum native Rust server";
const STATIC_PLAYER_ID: i32 = 1;
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
const PLAY_OUTPUT_QUEUE_CAPACITY: usize = 256;
const PLAY_WRITER_WAIT_MILLIS: u64 = 50;

#[derive(Debug, Parser)]
#[command(
    name = "ferrum-server",
    version,
    about = "Native Rust Minecraft-compatible server runtime"
)]
struct Cli {
    /// Path to the server configuration file.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,

    /// Locally generated and integrity-verified RoM version pack.
    #[arg(long, value_name = "PATH")]
    version_pack: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerConfig {
    profile_name: Option<String>,
    bind: String,
    version_name: String,
    protocol: i32,
    motd: String,
    max_players: i32,
    online_players: i32,
    login_disconnect_message: String,
    allow_offline_login: bool,
    configuration_enabled: bool,
    configuration_features: Vec<String>,
    online_mode: bool,
    hide_online_players: bool,
    enforces_secure_chat: bool,
    previews_chat: bool,
    server_icon: Option<String>,
    sample_players: Vec<SamplePlayer>,
    world: WorldConfig,
    play_policy: PlayPolicy,
    packets: PacketIds,
    runtime_profile: Option<ProtocolProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct WorldConfig {
    region_file: Option<PathBuf>,
    region_dir: Option<PathBuf>,
    region_x: Option<i32>,
    region_z: Option<i32>,
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
    name: String,
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PacketIds {
    handshake_serverbound: i32,
    status_request_serverbound: i32,
    status_response_clientbound: i32,
    ping_request_serverbound: i32,
    pong_response_clientbound: i32,
    login_start_serverbound: i32,
    login_disconnect_clientbound: i32,
    login_success_clientbound: i32,
    login_acknowledged_serverbound: Option<i32>,
    configuration_acknowledged_serverbound: Option<i32>,
    configuration_finish_clientbound: Option<i32>,
    configuration_feature_flags_clientbound: Option<i32>,
    configuration_tags_clientbound: Option<i32>,
    configuration_registry_data_clientbound: Option<i32>,
    play_player_action_serverbound: Option<i32>,
    play_use_item_on_serverbound: Option<i32>,
    play_block_update_clientbound: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Handshake {
    protocol: i32,
    server_address: String,
    server_port: u16,
    next_state: i32,
}

#[derive(Debug)]
struct ServerState {
    online_players: AtomicI32,
    next_connection_id: AtomicU64,
    world: play_runtime::SharedWorld,
    registry_payloads: Vec<Vec<u8>>,
    shared_play_runtime: SharedPlayRuntime,
    game_runtime: SharedGameRuntime,
}

impl ServerState {
    #[cfg(test)]
    fn new(config: &ServerConfig) -> Self {
        let registry_payloads =
            if config.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME) {
                builtin_26_1_2_registry_payloads().expect("built-in registry payloads must encode")
            } else {
                Vec::new()
            };
        Self::with_runtime(
            config.online_players,
            play_runtime::builtin_world_profile(),
            registry_payloads,
            config.play_policy.clone(),
            None,
        )
        .expect("built-in world profile must initialize")
    }

    fn with_runtime(
        initial_online_players: i32,
        world: RomPackWorld,
        registry_payloads: Vec<Vec<u8>>,
        play_policy: PlayPolicy,
        loaded_chunks: Option<ChunkStore>,
    ) -> Result<Self> {
        let center = play_runtime::spawn_chunk(&world);
        let game_runtime = SharedGameRuntime::new(GameState::new(world.dimension.clone())?);
        let shared_runtime_config = shared_play_runtime_config(&play_policy)?;
        let shared_world = match loaded_chunks {
            Some(store) => {
                play_runtime::SharedWorld::from_store_with_policy(store, world, play_policy)?
            }
            None => play_runtime::SharedWorld::new_with_policy(center, world, play_policy)?,
        };
        let shared_play_runtime = spawn_shared_play_runtime(shared_runtime_config)?;
        Ok(Self {
            online_players: AtomicI32::new(initial_online_players),
            next_connection_id: AtomicU64::new(1),
            world: shared_world,
            registry_payloads,
            shared_play_runtime,
            game_runtime,
        })
    }

    #[allow(dead_code)]
    fn with_loaded_world_runtime(
        initial_online_players: i32,
        world: RomPackWorld,
        store: ChunkStore,
        registry_payloads: Vec<Vec<u8>>,
        play_policy: PlayPolicy,
    ) -> Result<Self> {
        Self::with_runtime(
            initial_online_players,
            world,
            registry_payloads,
            play_policy,
            Some(store),
        )
    }

    fn online_players(&self) -> i32 {
        self.online_players.load(Ordering::Relaxed)
    }

    fn try_enter_play(
        &self,
        identity: &PlayerIdentity,
        transform: Transform,
    ) -> Result<OnlinePlayerGuard<'_>> {
        let player_uuid = GamePlayerUuid::from_bytes(*identity.uuid.as_bytes());
        self.game_runtime
            .connect_player(player_uuid, identity.username.clone(), transform)?;
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);
        let connection_id = ConnectionId::new(id);
        let endpoints = register_play_connection(
            &self.shared_play_runtime.connector(),
            connection_id,
            NonZeroUsize::new(PLAY_OUTPUT_QUEUE_CAPACITY)
                .expect("Play output queue capacity is non-zero"),
        );
        let (play_reader, play_writer) = match endpoints {
            Ok(endpoints) => endpoints,
            Err(error) => {
                let _ = self.game_runtime.disconnect_player(player_uuid);
                return Err(error);
            }
        };
        self.online_players.fetch_add(1, Ordering::Relaxed);
        Ok(OnlinePlayerGuard {
            state: self,
            connection_id,
            player_uuid,
            play_reader,
            play_writer: Some(play_writer),
        })
    }

    #[cfg(test)]
    fn enter_play(&self) -> OnlinePlayerGuard<'_> {
        let identity = offline_player_identity("TestPlayer");
        let transform = game_spawn_transform(self.world.world_profile())
            .expect("test spawn transform must be valid");
        self.try_enter_play(&identity, transform)
            .expect("test Play connection must register")
    }

    fn world(&self) -> &play_runtime::SharedWorld {
        &self.world
    }

    fn registry_payloads(&self) -> &[Vec<u8>] {
        &self.registry_payloads
    }
}

fn shared_play_runtime_config(play_policy: &PlayPolicy) -> Result<SharedPlayRuntimeConfig> {
    let keep_alive_interval_ticks = play_policy
        .keep_alive_interval_seconds
        .checked_mul(20)
        .context("shared runtime Keep Alive tick interval overflow")?;
    Ok(SharedPlayRuntimeConfig {
        keep_alive_interval_ticks: NonZeroU32::new(
            u32::try_from(keep_alive_interval_ticks)
                .context("shared runtime Keep Alive interval exceeds u32")?,
        )
        .context("shared runtime Keep Alive interval must be greater than zero")?,
        ..SharedPlayRuntimeConfig::default()
    })
}

#[derive(Debug)]
struct OnlinePlayerGuard<'a> {
    state: &'a ServerState,
    connection_id: ConnectionId,
    player_uuid: GamePlayerUuid,
    play_reader: PlayReaderEndpoint,
    play_writer: Option<PlayWriterEndpoint>,
}

impl OnlinePlayerGuard<'_> {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id
    }

    fn player_uuid(&self) -> GamePlayerUuid {
        self.player_uuid
    }

    fn play_reader(&self) -> &PlayReaderEndpoint {
        &self.play_reader
    }

    fn take_play_writer(&mut self) -> Result<PlayWriterEndpoint> {
        self.play_writer
            .take()
            .context("Play writer endpoint was already taken")
    }
}

impl Drop for OnlinePlayerGuard<'_> {
    fn drop(&mut self) {
        let _ = self.play_reader.try_disconnect();
        let _ = self.state.game_runtime.disconnect_player(self.player_uuid);
        self.state.online_players.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy)]
struct ServerContext<'a> {
    config: &'a ServerConfig,
    state: &'a ServerState,
}

#[derive(Debug, Clone, Copy)]
struct PlayWorldContext<'a> {
    shared_world: &'a play_runtime::SharedWorld,
    connection: ConnectionId,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    if !cli.config.is_file() {
        bail!(
            "server config {} does not exist or is not a file",
            cli.config.display()
        );
    }

    let config_path = cli
        .config
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", cli.config.display()))?;
    let mut config = ServerConfig::from_file(&config_path)
        .with_context(|| format!("cannot load {}", config_path.display()))?;
    let (runtime_profile, world_profile, registry_payloads) =
        if let Some(version_pack) = &cli.version_pack {
            let loaded = load_version_pack(version_pack, &config)?;
            (loaded.profile, loaded.world, loaded.registry_payloads)
        } else {
            let registry_payloads =
                if config.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME) {
                    builtin_26_1_2_registry_payloads()?
                } else {
                    Vec::new()
                };
            (
                config
                    .protocol_profile()
                    .context("cannot build configured protocol profile")?,
                play_runtime::builtin_world_profile(),
                registry_payloads,
            )
        };
    config.runtime_profile = Some(runtime_profile);
    let loaded_chunks = load_configured_world_chunks(&config.world, &world_profile)?;
    let state = Arc::new(ServerState::with_runtime(
        config.online_players,
        world_profile,
        registry_payloads,
        config.play_policy.clone(),
        loaded_chunks,
    )?);
    let listener = TcpListener::bind(&config.bind)
        .with_context(|| format!("cannot bind Minecraft status listener on {}", config.bind))?;
    println!(
        "ferrum-server listening on {} as {} protocol {}",
        config.bind, config.version_name, config.protocol
    );

    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                let config = config.clone();
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(error) = handle_client(&mut stream, &config, &state) {
                        eprintln!("connection closed: {error:#}");
                    }
                });
            }
            Err(error) => eprintln!("incoming connection failed: {error}"),
        }
    }
    Ok(())
}

struct LoadedVersionPack {
    profile: ProtocolProfile,
    world: RomPackWorld,
    registry_payloads: Vec<Vec<u8>>,
}

fn load_version_pack(path: &Path, config: &ServerConfig) -> Result<LoadedVersionPack> {
    if !path.is_file() {
        bail!(
            "version pack {} does not exist or is not a file",
            path.display()
        );
    }
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    let (pack, summary) =
        read_rompack(&canonical).with_context(|| format!("cannot load {}", canonical.display()))?;
    validate_builtin_26_1_2_pack(&pack)?;
    if config.profile_name.as_deref() != Some(version_26_1_2::PROFILE_NAME)
        || config.protocol != pack.metadata.protocol
        || config.version_name != version_26_1_2::VERSION_NAME
    {
        bail!("server configuration does not match the generated 26.1.2 version pack");
    }
    validate_world_profile(&pack.world)?;
    let profile =
        protocol_profile_from_packets(&config.version_name, pack.metadata.protocol, &pack.packets)?;
    let registry_payloads = registry_payloads_from_pack(&pack.registries)?;
    println!(
        "loaded RoM version pack {} (SHA-256 {}, {} packets, data version {}, {} sections, {} registries / {} entries)",
        canonical.display(),
        summary.sha256,
        summary.packet_count,
        pack.world.data_version,
        pack.world.overworld_section_count,
        summary.registry_count,
        summary.registry_entry_count
    );
    Ok(LoadedVersionPack {
        profile,
        world: pack.world,
        registry_payloads,
    })
}

fn registry_payloads_from_pack(registries: &[RomPackRegistry]) -> Result<Vec<Vec<u8>>> {
    let runtime_registries = registries
        .iter()
        .map(|registry| {
            RegistryData::new(
                registry.id.clone(),
                registry
                    .entries
                    .iter()
                    .cloned()
                    .map(|entry| RegistryEntry::new(entry, None))
                    .collect(),
            )
        })
        .collect();
    encode_registry_payloads(runtime_registries)
}

fn builtin_26_1_2_registry_payloads() -> Result<Vec<Vec<u8>>> {
    encode_registry_payloads(version_26_1_2::configuration_registries())
}

fn encode_registry_payloads(registries: Vec<RegistryData>) -> Result<Vec<Vec<u8>>> {
    registries
        .into_iter()
        .map(|registry| {
            let id = registry.id.clone();
            encode_registry_data(&registry)
                .with_context(|| format!("cannot encode generated registry {id}"))
        })
        .collect()
}

fn validate_world_profile(world: &RomPackWorld) -> Result<()> {
    let section_count = i32::try_from(world.overworld_section_count)
        .context("overworld section count exceeds i32")?;
    let min_block_y = world
        .overworld_min_section_y
        .checked_mul(16)
        .context("overworld minimum block y overflow")?;
    let max_block_y = world
        .overworld_min_section_y
        .checked_add(section_count)
        .and_then(|section| section.checked_mul(16))
        .and_then(|block| block.checked_sub(1))
        .context("overworld maximum block y overflow")?;
    let player_spawn_y = world
        .floor_y
        .checked_add(2)
        .context("generated player spawn overflow")?;
    if !(min_block_y..=max_block_y).contains(&player_spawn_y) {
        bail!(
            "generated world height {min_block_y}..={max_block_y} does not contain player spawn y {player_spawn_y}"
        );
    }
    Ok(())
}

fn load_configured_world_chunks(
    config: &WorldConfig,
    world: &RomPackWorld,
) -> Result<Option<ChunkStore>> {
    let profile = anvil_conversion_profile(world);
    if let Some(region_file) = &config.region_file {
        let region = configured_region_file_pos(config, region_file)?;
        let report = load_anvil_region_file(region_file, region, &profile)?;
        return Ok(Some(report.store));
    }
    if let Some(region_dir) = &config.region_dir {
        let store = load_anvil_region_directory(region_dir, &profile)?;
        return Ok(Some(store));
    }
    Ok(None)
}

fn configured_region_file_pos(config: &WorldConfig, region_file: &Path) -> Result<RegionPos> {
    match (config.region_x, config.region_z) {
        (Some(x), Some(z)) => Ok(RegionPos { x, z }),
        (None, None) => region_pos_from_file_name(region_file)?
            .with_context(|| format!("world.region_file {} must be named r.X.Z.mca when world.region_x and world.region_z are omitted", region_file.display())),
        _ => bail!("world.region_x and world.region_z must be set together"),
    }
}

fn anvil_conversion_profile(world: &RomPackWorld) -> AnvilChunkConversionProfile {
    let mut profile = AnvilChunkConversionProfile::new(
        BlockStateId::new(world.block_states.air),
        BiomeId::new(world.biomes.plains),
    )
    .with_block_state("minecraft:air", BlockStateId::new(world.block_states.air))
    .with_block_state(
        "minecraft:stone",
        BlockStateId::new(world.block_states.stone),
    )
    .with_block_state(
        "minecraft:grass_block",
        BlockStateId::new(world.block_states.grass),
    )
    .with_block_state("minecraft:dirt", BlockStateId::new(world.block_states.dirt))
    .with_block_state(
        "minecraft:bedrock",
        BlockStateId::new(world.block_states.bedrock),
    )
    .with_unknown_block_state(BlockStateId::new(world.block_states.stone))
    .with_unknown_biome(BiomeId::new(world.biomes.plains));

    for (index, biome) in builtin_biome_registry_entries(world)
        .into_iter()
        .enumerate()
    {
        profile = profile.with_biome(biome, BiomeId::new(index as u32));
    }
    profile
}

fn builtin_biome_registry_entries(world: &RomPackWorld) -> Vec<&'static str> {
    if world.data_version == version_26_1_2::WORLD_VERSION
        && world.biomes.plains == version_26_1_2::PLAINS_BIOME_ID
    {
        return version_26_1_2::SYNCHRONIZED_REGISTRIES
            .iter()
            .find(|registry| registry.id == "minecraft:worldgen/biome")
            .map(|registry| registry.entries.to_vec())
            .unwrap_or_default();
    }
    Vec::new()
}

fn load_anvil_region_directory(
    region_dir: &Path,
    profile: &AnvilChunkConversionProfile,
) -> Result<ChunkStore> {
    let mut store = ChunkStore::new();
    let mut loaded_regions = 0_usize;
    let mut skipped_regions = 0_usize;
    let entries = fs::read_dir(region_dir).with_context(|| {
        format!(
            "cannot read Anvil region directory {}",
            region_dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "cannot read an entry in Anvil region directory {}",
                region_dir.display()
            )
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(region) = region_pos_from_file_name(&path)? else {
            continue;
        };
        let region_store = match load_anvil_region_file(&path, region, profile) {
            Ok(region_store) => region_store,
            Err(error) => {
                skipped_regions += 1;
                eprintln!("skipped Anvil region {}: {error:#}", path.display());
                continue;
            }
        };
        for (_, chunk) in region_store.store.iter() {
            store.insert(chunk.clone());
        }
        loaded_regions += 1;
    }
    if loaded_regions == 0 {
        bail!(
            "Anvil region directory {} did not contain any r.X.Z.mca files",
            region_dir.display()
        );
    }
    println!(
        "loaded {loaded_regions} Anvil region file(s) from {}",
        region_dir.display()
    );
    if skipped_regions > 0 {
        eprintln!(
            "skipped {skipped_regions} Anvil region file(s) while loading {}",
            region_dir.display()
        );
    }
    Ok(store)
}

fn load_anvil_region_file(
    region_file: &Path,
    region: RegionPos,
    profile: &AnvilChunkConversionProfile,
) -> Result<ferrum_world::anvil::AnvilRegionLoadReport> {
    let mut file = fs::File::open(region_file)
        .with_context(|| format!("cannot open Anvil region file {}", region_file.display()))?;
    let header = RegionHeader::read(&mut file)
        .with_context(|| format!("cannot read Anvil region header {}", region_file.display()))?;
    let report = load_chunk_store_from_region_lenient(
        &mut file,
        &header,
        region,
        profile,
        AnvilDecodeLimits::default(),
    )
    .with_context(|| {
        format!(
            "cannot load Anvil region {} as ({}, {})",
            region_file.display(),
            region.x,
            region.z
        )
    })?;
    if !report.skipped_chunks.is_empty() {
        eprintln!(
            "skipped {} chunk(s) while loading Anvil region {}",
            report.skipped_chunks.len(),
            region_file.display()
        );
        for skipped in &report.skipped_chunks {
            eprintln!(
                "skipped chunk ({}, {}) in {}: {}",
                skipped.position.x,
                skipped.position.z,
                region_file.display(),
                skipped.error
            );
        }
    }
    println!(
        "loaded Anvil region {} as ({}, {}) with {} chunk(s)",
        region_file.display(),
        region.x,
        region.z,
        report.loaded_chunks
    );
    Ok(report)
}

fn region_pos_from_file_name(path: &Path) -> Result<Option<RegionPos>> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    let Some(stem) = file_name
        .strip_prefix("r.")
        .and_then(|name| name.strip_suffix(".mca"))
    else {
        return Ok(None);
    };
    let Some((x, z)) = stem.split_once('.') else {
        bail!("invalid Anvil region file name {}", path.display());
    };
    if z.contains('.') {
        bail!("invalid Anvil region file name {}", path.display());
    }
    Ok(Some(RegionPos {
        x: x.parse()
            .with_context(|| format!("invalid Anvil region X coordinate in {}", path.display()))?,
        z: z.parse()
            .with_context(|| format!("invalid Anvil region Z coordinate in {}", path.display()))?,
    }))
}

fn protocol_profile_from_packets(
    version_name: &str,
    protocol: i32,
    packets: &[RomPackPacket],
) -> Result<ProtocolProfile> {
    let expected: BTreeSet<_> = PacketKind::ALL.iter().copied().collect();
    let actual: BTreeSet<_> = packets.iter().map(|packet| packet.kind).collect();
    if actual != expected || packets.len() != expected.len() {
        bail!(
            "version pack packet kinds do not match the runtime: expected {}, got {}",
            expected.len(),
            packets.len()
        );
    }

    let mut table = PacketTable::new();
    for packet in packets {
        table.insert(packet.kind, packet.id)?;
    }
    ProtocolProfile::new(version_name, protocol, table)
        .context("cannot build protocol profile from the generated version pack")
}

fn validate_builtin_26_1_2_pack(pack: &RomPack) -> Result<()> {
    if pack.metadata.minecraft_version != version_26_1_2::PROFILE_NAME
        || pack.metadata.protocol != version_26_1_2::PROTOCOL_VERSION
    {
        bail!("version pack does not match the built-in Minecraft 26.1.2 profile");
    }
    if !pack
        .metadata
        .source
        .official_server_sha1
        .eq_ignore_ascii_case(version_26_1_2::OFFICIAL_SERVER_SHA1)
    {
        bail!("version pack official-source SHA-1 does not match the built-in profile");
    }
    if pack.metadata.patch_set != "builtin:26.1.2" {
        bail!("version pack patch-set identity does not match the built-in profile");
    }

    let expected: BTreeMap<_, _> = version_26_1_2::SYNCHRONIZED_REGISTRIES
        .iter()
        .map(|registry| (registry.id, registry.entries.to_vec()))
        .collect();
    let actual: BTreeMap<_, _> = pack
        .registries
        .iter()
        .map(|registry| {
            (
                registry.id.as_str(),
                registry
                    .entries
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    if actual != expected {
        bail!("version pack synchronized registries do not match the built-in profile");
    }
    Ok(())
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            profile_name: None,
            bind: DEFAULT_BIND.to_owned(),
            version_name: DEFAULT_VERSION_NAME.to_owned(),
            protocol: DEFAULT_PROTOCOL,
            motd: DEFAULT_MOTD.to_owned(),
            max_players: 20,
            online_players: 0,
            login_disconnect_message: "Ferrum native server currently implements status ping only"
                .to_owned(),
            allow_offline_login: false,
            configuration_enabled: false,
            configuration_features: vec!["minecraft:vanilla".to_owned()],
            online_mode: false,
            hide_online_players: false,
            enforces_secure_chat: false,
            previews_chat: false,
            server_icon: None,
            sample_players: Vec::new(),
            world: WorldConfig::default(),
            play_policy: PlayPolicy::default(),
            packets: PacketIds::default(),
            runtime_profile: None,
        }
    }
}

impl Default for PacketIds {
    fn default() -> Self {
        Self {
            handshake_serverbound: 0,
            status_request_serverbound: 0,
            status_response_clientbound: 0,
            ping_request_serverbound: 1,
            pong_response_clientbound: 1,
            login_start_serverbound: 0,
            login_disconnect_clientbound: 0,
            login_success_clientbound: 2,
            login_acknowledged_serverbound: None,
            configuration_acknowledged_serverbound: None,
            configuration_finish_clientbound: None,
            configuration_feature_flags_clientbound: None,
            configuration_tags_clientbound: None,
            configuration_registry_data_clientbound: None,
            play_player_action_serverbound: None,
            play_use_item_on_serverbound: None,
            play_block_update_clientbound: None,
        }
    }
}

impl ServerConfig {
    fn from_file(path: &PathBuf) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let base_dir = path.parent().map(PathBuf::from).unwrap_or_default();
        Self::from_toml_like_with_base(&text, Some(&base_dir))
    }

    fn from_toml_like_with_base(text: &str, base_dir: Option<&PathBuf>) -> Result<Self> {
        let profile_name = find_profile_name(text)?;
        let mut config = Self::for_profile(profile_name.as_deref())?;
        let mut section = String::new();
        for (line_index, raw_line) in text.lines().enumerate() {
            let line = raw_line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_owned();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                bail!("invalid config line {}: {}", line_index + 1, raw_line);
            };
            let key = key.trim();
            let value = value.trim();
            match (section.as_str(), key) {
                ("server", "profile") => {
                    let parsed = parse_string(value);
                    if config.profile_name.as_deref() != Some(parsed.as_str()) {
                        bail!("profile changed while parsing configuration")
                    }
                }
                ("server", "bind") => config.bind = parse_string(value),
                ("server", "version_name") if config.profile_name.is_some() => {
                    bail!("version_name cannot be overridden when server.profile is set")
                }
                ("server", "version_name") => config.version_name = parse_string(value),
                ("server", "protocol") if config.profile_name.is_some() => {
                    bail!("protocol cannot be overridden when server.profile is set")
                }
                ("server", "protocol") => config.protocol = parse_i32(value, line_index + 1)?,
                ("server", "motd") => config.motd = parse_string(value),
                ("server", "max_players") => config.max_players = parse_i32(value, line_index + 1)?,
                ("server", "online_players") => {
                    config.online_players = parse_i32(value, line_index + 1)?
                }
                ("server", "login_disconnect_message") => {
                    config.login_disconnect_message = parse_string(value)
                }
                ("server", "allow_offline_login") => {
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
                ("world", "region_file") => {
                    let path = parse_string(value);
                    config.world.region_file =
                        Some(resolve_config_path(&path, base_dir, line_index + 1)?);
                }
                ("world", "region_dir") => {
                    let path = parse_string(value);
                    config.world.region_dir =
                        Some(resolve_config_path(&path, base_dir, line_index + 1)?);
                }
                ("world", "region_x") => {
                    config.world.region_x = Some(parse_i32(value, line_index + 1)?)
                }
                ("world", "region_z") => {
                    config.world.region_z = Some(parse_i32(value, line_index + 1)?)
                }
                ("configuration", "enabled") => {
                    config.configuration_enabled = parse_bool(value, line_index + 1)?
                }
                ("configuration", "features") => {
                    config.configuration_features = parse_string_list(&parse_string(value))
                }
                ("server", "online_mode") => {
                    config.online_mode = parse_bool(value, line_index + 1)?
                }
                ("server", "hide_online_players") => {
                    config.hide_online_players = parse_bool(value, line_index + 1)?
                }
                ("server", "enforces_secure_chat") => {
                    config.enforces_secure_chat = parse_bool(value, line_index + 1)?
                }
                ("server", "previews_chat") => {
                    config.previews_chat = parse_bool(value, line_index + 1)?
                }
                ("server", "server_icon") => {
                    config.server_icon = Some(load_server_icon(
                        &parse_string(value),
                        base_dir,
                        line_index + 1,
                    )?)
                }
                ("server", "sample_players") => {
                    config.sample_players = parse_sample_players(&parse_string(value))?
                }
                ("protocol", _) if config.profile_name.is_some() => {
                    bail!("manual [protocol] packet IDs cannot be used with server.profile")
                }
                ("protocol", "handshake_serverbound") => {
                    config.packets.handshake_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "status_request_serverbound") => {
                    config.packets.status_request_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "status_response_clientbound") => {
                    config.packets.status_response_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "ping_request_serverbound") => {
                    config.packets.ping_request_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "pong_response_clientbound") => {
                    config.packets.pong_response_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_start_serverbound") => {
                    config.packets.login_start_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_disconnect_clientbound") => {
                    config.packets.login_disconnect_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_success_clientbound") => {
                    config.packets.login_success_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_acknowledged_serverbound") => {
                    config.packets.login_acknowledged_serverbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_acknowledged_serverbound") => {
                    config.packets.configuration_acknowledged_serverbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_finish_clientbound") => {
                    config.packets.configuration_finish_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_feature_flags_clientbound") => {
                    config.packets.configuration_feature_flags_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_tags_clientbound") => {
                    config.packets.configuration_tags_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_registry_data_clientbound") => {
                    config.packets.configuration_registry_data_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "play_player_action_serverbound") => {
                    config.packets.play_player_action_serverbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "play_use_item_on_serverbound") => {
                    config.packets.play_use_item_on_serverbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "play_block_update_clientbound") => {
                    config.packets.play_block_update_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                _ => bail!("unknown config key [{section}].{key}"),
            }
        }
        if config.max_players < 0 || config.online_players < 0 {
            bail!("player counts must be non-negative");
        }
        if config.online_players > config.max_players {
            bail!("online_players cannot exceed max_players");
        }
        if config.world.region_file.is_some() && config.world.region_dir.is_some() {
            bail!("world.region_file and world.region_dir cannot both be set");
        }
        if config.world.region_x.is_some() != config.world.region_z.is_some() {
            bail!("world.region_x and world.region_z must be set together");
        }
        if config.world.region_dir.is_some()
            && (config.world.region_x.is_some() || config.world.region_z.is_some())
        {
            bail!("world.region_x and world.region_z cannot be used with world.region_dir");
        }
        if config.world.region_file.is_none()
            && config.world.region_dir.is_none()
            && (config.world.region_x.is_some() || config.world.region_z.is_some())
        {
            bail!("world.region_x and world.region_z require world.region_file");
        }
        if !(0..=MAX_CONFIGURED_CHUNK_RADIUS).contains(&config.play_policy.chunk_radius) {
            bail!("play.chunk_radius must be between 0 and {MAX_CONFIGURED_CHUNK_RADIUS}");
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
            for (name, packet_id) in [
                (
                    "login_acknowledged_serverbound",
                    config.packets.login_acknowledged_serverbound,
                ),
                (
                    "configuration_acknowledged_serverbound",
                    config.packets.configuration_acknowledged_serverbound,
                ),
                (
                    "configuration_finish_clientbound",
                    config.packets.configuration_finish_clientbound,
                ),
            ] {
                if packet_id.is_none() {
                    bail!("configuration is enabled but [protocol].{name} is missing");
                }
            }
        }
        Ok(config)
    }

    fn for_profile(profile_name: Option<&str>) -> Result<Self> {
        let mut config = Self::default();
        match profile_name {
            None => Ok(config),
            Some(version_26_1_2::PROFILE_NAME) => {
                config.profile_name = Some(version_26_1_2::PROFILE_NAME.to_owned());
                config.version_name = version_26_1_2::VERSION_NAME.to_owned();
                config.protocol = version_26_1_2::PROTOCOL_VERSION;
                config.configuration_enabled = true;
                config.configuration_features = version_26_1_2::default_features();
                Ok(config)
            }
            Some(other) => bail!("unknown built-in server profile {other}"),
        }
    }

    #[cfg(test)]
    fn status_json(&self) -> String {
        self.status_json_with_online_players(self.online_players)
    }

    fn status_json_with_online_players(&self, online_players: i32) -> String {
        let mut root = Map::new();
        root.insert(
            "version".to_owned(),
            json!({
                "name": self.version_name,
                "protocol": self.protocol,
            }),
        );
        if !self.hide_online_players {
            let mut players = Map::new();
            players.insert("max".to_owned(), json!(self.max_players));
            players.insert("online".to_owned(), json!(online_players));
            if !self.sample_players.is_empty() {
                players.insert(
                    "sample".to_owned(),
                    Value::Array(
                        self.sample_players
                            .iter()
                            .map(|player| {
                                json!({
                                    "name": &player.name,
                                    "id": &player.id,
                                })
                            })
                            .collect(),
                    ),
                );
            }
            root.insert("players".to_owned(), Value::Object(players));
        }
        root.insert(
            "description".to_owned(),
            json!({
                "text": self.motd,
            }),
        );
        root.insert(
            "enforcesSecureChat".to_owned(),
            json!(self.enforces_secure_chat),
        );
        root.insert("previewsChat".to_owned(), json!(self.previews_chat));
        if let Some(favicon) = &self.server_icon {
            root.insert("favicon".to_owned(), json!(favicon));
        }
        Value::Object(root).to_string()
    }

    fn login_disconnect_json(&self) -> String {
        json!({
            "text": self.login_disconnect_message,
        })
        .to_string()
    }

    fn protocol_mismatch_json(&self, received: i32) -> String {
        json!({
            "text": format!(
                "Unsupported protocol {received}; this server requires {} (protocol {})",
                self.version_name, self.protocol
            ),
        })
        .to_string()
    }

    fn known_packs(&self) -> Vec<KnownPack> {
        match self.profile_name.as_deref() {
            Some(version_26_1_2::PROFILE_NAME) => version_26_1_2::known_packs(),
            _ => Vec::new(),
        }
    }

    fn protocol_profile(&self) -> Result<ProtocolProfile> {
        if let Some(profile) = &self.runtime_profile {
            return Ok(profile.clone());
        }
        if self.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME) {
            return version_26_1_2::protocol_profile().context("invalid 26.1.2 protocol profile");
        }

        let mut packets = PacketTable::new();
        packets.insert(PacketKind::Handshake, self.packets.handshake_serverbound)?;
        packets.insert(
            PacketKind::StatusRequest,
            self.packets.status_request_serverbound,
        )?;
        packets.insert(
            PacketKind::StatusResponse,
            self.packets.status_response_clientbound,
        )?;
        packets.insert(
            PacketKind::PingRequest,
            self.packets.ping_request_serverbound,
        )?;
        packets.insert(
            PacketKind::PongResponse,
            self.packets.pong_response_clientbound,
        )?;
        packets.insert(PacketKind::LoginStart, self.packets.login_start_serverbound)?;
        packets.insert(
            PacketKind::LoginDisconnect,
            self.packets.login_disconnect_clientbound,
        )?;
        packets.insert(
            PacketKind::LoginSuccess,
            self.packets.login_success_clientbound,
        )?;
        for (kind, id) in [
            (
                PacketKind::LoginAcknowledged,
                self.packets.login_acknowledged_serverbound,
            ),
            (
                PacketKind::ConfigurationAcknowledged,
                self.packets.configuration_acknowledged_serverbound,
            ),
            (
                PacketKind::FinishConfiguration,
                self.packets.configuration_finish_clientbound,
            ),
            (
                PacketKind::FeatureFlags,
                self.packets.configuration_feature_flags_clientbound,
            ),
            (
                PacketKind::UpdateTags,
                self.packets.configuration_tags_clientbound,
            ),
            (
                PacketKind::RegistryData,
                self.packets.configuration_registry_data_clientbound,
            ),
            (
                PacketKind::PlayerAction,
                self.packets.play_player_action_serverbound,
            ),
            (
                PacketKind::UseItemOn,
                self.packets.play_use_item_on_serverbound,
            ),
            (
                PacketKind::BlockUpdate,
                self.packets.play_block_update_clientbound,
            ),
        ] {
            if let Some(id) = id {
                packets.insert(kind, id)?;
            }
        }
        ProtocolProfile::new(self.version_name.clone(), self.protocol, packets)
            .context("invalid protocol profile")
    }
}

fn find_profile_name(text: &str) -> Result<Option<String>> {
    let mut section = String::new();
    let mut profile = None;
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            bail!("invalid config line {}: {}", line_index + 1, raw_line);
        };
        if section == "server" && key.trim() == "profile" {
            let value = parse_string(value.trim());
            if profile.replace(value).is_some() {
                bail!("server.profile may only be specified once")
            }
        }
    }
    Ok(profile)
}

fn parse_string(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn parse_string_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_i32(value: &str, line: usize) -> Result<i32> {
    value
        .parse()
        .with_context(|| format!("line {line} is not a valid i32: {value}"))
}

fn parse_u64(value: &str, line: usize) -> Result<u64> {
    value
        .parse()
        .with_context(|| format!("line {line} is not a valid u64: {value}"))
}

fn parse_bool(value: &str, line: usize) -> Result<bool> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => bail!("line {line} is not a valid bool: {other}"),
    }
}

fn parse_sample_players(value: &str) -> Result<Vec<SamplePlayer>> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let Some((name, id)) = entry.split_once(':') else {
                bail!("sample player entry must be name:uuid, got {entry}");
            };
            let name = name.trim();
            let id = id.trim();
            if name.is_empty() || id.is_empty() {
                bail!("sample player entry must include non-empty name and uuid");
            }
            Ok(SamplePlayer {
                name: name.to_owned(),
                id: id.to_owned(),
            })
        })
        .collect()
}

fn resolve_config_path(value: &str, base_dir: Option<&PathBuf>, line: usize) -> Result<PathBuf> {
    if value.trim().is_empty() {
        bail!("line {line}: path must not be empty");
    }
    let path = PathBuf::from(value);
    Ok(if path.is_absolute() {
        path
    } else if let Some(base_dir) = base_dir {
        base_dir.join(path)
    } else {
        path
    })
}

fn load_server_icon(value: &str, base_dir: Option<&PathBuf>, line: usize) -> Result<String> {
    if value.starts_with("data:image/png;base64,") {
        return Ok(value.to_owned());
    }
    let path = resolve_config_path(value, base_dir, line)?;
    let bytes =
        fs::read(&path).with_context(|| format!("line {line}: cannot read {}", path.display()))?;
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        bail!("line {line}: server_icon must point to a PNG file");
    }
    Ok(format!("data:image/png;base64,{}", base64_encode(&bytes)))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);

        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(third & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

std::thread_local! {
    static LIVE_PLAY_WRITER: std::cell::RefCell<Option<SharedWriter<TcpStream>>> =
        const { std::cell::RefCell::new(None) };
}

struct LivePlayWriterRegistration;

impl LivePlayWriterRegistration {
    fn install(writer: SharedWriter<TcpStream>) -> Result<Self> {
        LIVE_PLAY_WRITER.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_some() {
                bail!("live Play writer is already registered on this thread");
            }
            *slot = Some(writer);
            Ok(Self)
        })
    }
}

impl Drop for LivePlayWriterRegistration {
    fn drop(&mut self) {
        LIVE_PLAY_WRITER.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

fn take_live_play_writer() -> Option<SharedWriter<TcpStream>> {
    LIVE_PLAY_WRITER.with(|slot| slot.borrow_mut().take())
}

fn handle_client(stream: &mut TcpStream, config: &ServerConfig, state: &ServerState) -> Result<()> {
    let mut reader = stream
        .try_clone()
        .context("cannot clone TCP stream reader")?;
    let writer = SharedWriter::new(
        stream
            .try_clone()
            .context("cannot clone TCP stream writer")?,
    );
    let _live_writer = LivePlayWriterRegistration::install(writer.clone())?;
    handle_connection_protocol_with_play_round_limit(&mut reader, writer, config, state, None)
}

#[cfg(test)]
fn handle_connection_protocol<R: Read, W: Write>(
    reader: R,
    writer: W,
    config: &ServerConfig,
) -> Result<()> {
    let state = ServerState::new(config);
    handle_connection_protocol_with_play_round_limit(reader, writer, config, &state, Some(1))
}

#[cfg(test)]
fn handle_connection_protocol_with_state<R: Read, W: Write>(
    reader: R,
    writer: W,
    config: &ServerConfig,
    state: &ServerState,
) -> Result<()> {
    handle_connection_protocol_with_play_round_limit(reader, writer, config, state, Some(1))
}

fn handle_connection_protocol_with_play_round_limit<R: Read, W: Write>(
    mut reader: R,
    writer: W,
    config: &ServerConfig,
    state: &ServerState,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let profile = config.protocol_profile()?;
    let context = ServerContext { config, state };
    let handshake_packet = read_packet(&mut reader).context("cannot read handshake packet")?;
    let handshake = parse_handshake_packet(&handshake_packet, profile.packets())?;
    let intent = handshake.intent()?;
    let mut session = ProtocolSession::new();
    session.handshake(handshake.protocol, intent)?;
    match intent {
        HandshakeIntent::Status => {
            handle_status_protocol(reader, writer, context, &profile, &mut session)
        }
        HandshakeIntent::Login => handle_login_protocol(
            reader,
            writer,
            context,
            &handshake,
            &profile,
            &mut session,
            play_round_limit,
        ),
    }
}

impl Handshake {
    fn intent(&self) -> Result<HandshakeIntent> {
        match self.next_state {
            1 => Ok(HandshakeIntent::Status),
            2 => Ok(HandshakeIntent::Login),
            other => bail!("unsupported handshake next_state {other}"),
        }
    }
}

fn handle_status_protocol<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    context: ServerContext<'_>,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
) -> Result<()> {
    let expected_request_id = profile.packets().require(PacketKind::StatusRequest)?;
    let request_packet = read_packet(&mut reader).context("cannot read status request")?;
    let mut request_reader = PacketReader::new(&request_packet);
    let request_id = request_reader.read_varint()?;
    if request_id != expected_request_id {
        bail!("expected status request packet id {expected_request_id}, got {request_id}");
    }
    session.status_request()?;
    write_packet(
        &mut writer,
        &build_packet(
            profile.packets().require(PacketKind::StatusResponse)?,
            |body| {
                write_string(
                    body,
                    &context
                        .config
                        .status_json_with_online_players(context.state.online_players()),
                )
            },
        )?,
    )?;
    session.status_response_sent()?;

    match read_packet(&mut reader) {
        Ok(ping_packet) => {
            let expected_ping_id = profile.packets().require(PacketKind::PingRequest)?;
            let mut ping_reader = PacketReader::new(&ping_packet);
            let ping_id = ping_reader.read_varint()?;
            if ping_id != expected_ping_id {
                bail!("expected ping packet id {expected_ping_id}, got {ping_id}");
            }
            session.ping()?;
            let payload = ping_reader.read_i64()?;
            write_packet(
                &mut writer,
                &build_packet(
                    profile.packets().require(PacketKind::PongResponse)?,
                    |body| {
                        body.extend_from_slice(&payload.to_be_bytes());
                        Ok(())
                    },
                )?,
            )?;
            session.pong_sent()?;
        }
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {}
        Err(error) => return Err(error).context("cannot read ping packet"),
    }

    writer.flush()?;
    Ok(())
}

fn handle_login_protocol<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    context: ServerContext<'_>,
    handshake: &Handshake,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let config = context.config;
    if config.profile_name.is_some() && !profile.supports(handshake.protocol) {
        write_packet(
            &mut writer,
            &build_packet(
                profile.packets().require(PacketKind::LoginDisconnect)?,
                |body| write_string(body, &config.protocol_mismatch_json(handshake.protocol)),
            )?,
        )?;
        session.disconnect();
        writer.flush()?;
        return Ok(());
    }

    let expected_login_id = profile.packets().require(PacketKind::LoginStart)?;
    let login_packet = read_packet(&mut reader).context("cannot read login start packet")?;
    let mut login_reader = PacketReader::new(&login_packet);
    let packet_id = login_reader.read_varint()?;
    if packet_id != expected_login_id {
        bail!("expected login start packet id {expected_login_id}, got {packet_id}");
    }
    let username = login_reader.read_string()?;
    session.login_start(username.clone())?;
    let identity = offline_player_identity(&username);
    println!(
        "login attempt from {} ({}) online_mode={}",
        identity.username,
        identity.uuid.hyphenated(),
        config.online_mode
    );

    if config.allow_offline_login && !config.online_mode {
        write_packet(
            &mut writer,
            &build_packet(
                profile.packets().require(PacketKind::LoginSuccess)?,
                |body| {
                    body.extend_from_slice(identity.uuid.as_bytes());
                    write_string(body, &identity.username)?;
                    write_varint_vec(body, 0);
                    Ok(())
                },
            )?,
        )?;
        session.login_success_sent()?;
        if config.configuration_enabled {
            handle_configuration_protocol(
                &mut reader,
                &mut writer,
                context,
                &identity,
                profile,
                session,
                play_round_limit,
            )?;
        }
    } else {
        write_packet(
            &mut writer,
            &build_packet(
                profile.packets().require(PacketKind::LoginDisconnect)?,
                |body| write_string(body, &config.login_disconnect_json()),
            )?,
        )?;
        session.disconnect();
    }
    writer.flush()?;
    Ok(())
}

fn handle_configuration_protocol<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    context: ServerContext<'_>,
    identity: &PlayerIdentity,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let config = context.config;
    let login_acknowledged =
        read_packet(reader).context("cannot read login acknowledged packet")?;
    let mut login_acknowledged_reader = PacketReader::new(&login_acknowledged);
    let expected_login_acknowledged = profile.packets().require(PacketKind::LoginAcknowledged)?;
    let packet_id = login_acknowledged_reader.read_varint()?;
    if packet_id != expected_login_acknowledged {
        bail!(
            "expected login acknowledged packet id {expected_login_acknowledged}, got {packet_id}"
        );
    }
    session.login_acknowledged()?;

    let accepted_known_packs = negotiate_known_packs(reader, writer, config, profile)?;
    if config.profile_name.as_deref() == Some(version_26_1_2::PROFILE_NAME)
        && !version_26_1_2::accepts_vanilla_core_pack(&accepted_known_packs)
    {
        write_configuration_disconnect(
            writer,
            profile,
            "Minecraft 26.1.2 requires the bundled minecraft/core/26.1.2 data pack",
        )?;
        session.disconnect();
        writer.flush()?;
        return Ok(());
    }

    if let Some(packet_id) = profile.packets().id(PacketKind::FeatureFlags) {
        let body = encode_feature_flags(&config.configuration_features)?;
        write_packet(
            writer,
            &build_packet(packet_id, |output| {
                output.extend_from_slice(&body);
                Ok(())
            })?,
        )?;
    }
    send_registry_data(writer, context.state.registry_payloads(), profile)?;

    if let Some(packet_id) = profile.packets().id(PacketKind::UpdateTags) {
        let body = encode_tags(&[])?;
        write_packet(
            writer,
            &build_packet(packet_id, |output| {
                output.extend_from_slice(&body);
                Ok(())
            })?,
        )?;
    }

    write_packet(
        writer,
        &build_packet(
            profile.packets().require(PacketKind::FinishConfiguration)?,
            |_| Ok(()),
        )?,
    )?;
    session.finish_configuration_sent()?;
    writer.flush()?;

    let acknowledged =
        read_packet(reader).context("cannot read configuration acknowledged packet")?;
    let mut acknowledged_reader = PacketReader::new(&acknowledged);
    let expected_acknowledged = profile
        .packets()
        .require(PacketKind::ConfigurationAcknowledged)?;
    let packet_id = acknowledged_reader.read_varint()?;
    if packet_id != expected_acknowledged {
        bail!(
            "expected configuration acknowledged packet id {expected_acknowledged}, got {packet_id}"
        );
    }
    session.configuration_acknowledged()?;
    println!("configuration completed; connection entered Play state");
    handle_play_protocol(
        reader,
        writer,
        context,
        identity,
        profile,
        session,
        play_round_limit,
    )
}

fn handle_play_protocol<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    context: ServerContext<'_>,
    identity: &PlayerIdentity,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let config = context.config;
    if config.profile_name.as_deref() != Some(version_26_1_2::PROFILE_NAME) {
        return Ok(());
    }

    let transform = game_spawn_transform(context.state.world().world_profile())?;
    let mut online_player = context.state.try_enter_play(identity, transform)?;
    let writer_worker = match take_live_play_writer() {
        Some(live_writer) => Some(spawn_live_play_writer(
            online_player.take_play_writer()?,
            live_writer,
            profile,
        )?),
        None => None,
    };
    let play_reader = writer_worker.as_ref().map(|_| online_player.play_reader());
    let gameplay =
        play_runtime::GameplaySync::new(&context.state.game_runtime, online_player.player_uuid());
    let result = run_static_play_session_with_bridge(
        reader,
        writer,
        config,
        profile,
        session,
        PlayWorldContext {
            shared_world: context.state.world(),
            connection: online_player.connection_id(),
        },
        play_reader,
        Some(gameplay),
        play_round_limit,
    );
    let writer_result = shutdown_live_play_writer(writer_worker);
    if let Err(error) = result {
        if let Err(writer_error) = writer_result {
            eprintln!("Play writer shutdown also failed: {writer_error:#}");
        }
        let reason = format!("Ferrum closed the connection: {error}");
        if let Ok(payload) = encode_play_disconnect(&reason) {
            let _ = write_play_payload(writer, profile, PacketKind::PlayDisconnect, &payload);
            let _ = writer.flush();
        }
        session.disconnect();
        return Err(error);
    }
    writer_result?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct PlayOutputPacketIds {
    keep_alive_request: i32,
    disconnect: i32,
}

fn spawn_live_play_writer(
    endpoint: PlayWriterEndpoint,
    writer: SharedWriter<TcpStream>,
    profile: &ProtocolProfile,
) -> Result<PlayWriterWorker<SharedWriter<TcpStream>>> {
    let packet_ids = PlayOutputPacketIds {
        keep_alive_request: profile.packets().require(PacketKind::KeepAliveRequest)?,
        disconnect: profile.packets().require(PacketKind::PlayDisconnect)?,
    };
    spawn_play_writer(
        endpoint,
        writer,
        Duration::from_millis(PLAY_WRITER_WAIT_MILLIS),
        move |writer, output| write_live_play_output(writer, packet_ids, output),
    )
}

fn shutdown_live_play_writer(
    writer: Option<PlayWriterWorker<SharedWriter<TcpStream>>>,
) -> Result<()> {
    if let Some(writer) = writer {
        writer
            .shutdown()
            .context("cannot shut down live Play writer")?;
    }
    Ok(())
}

fn write_live_play_output<W: Write>(
    writer: &mut W,
    packet_ids: PlayOutputPacketIds,
    output: PlayOutput,
) -> Result<PlayWriterDirective> {
    let directive = match output {
        PlayOutput::Packet(packet) => {
            write_packet(writer, &packet)?;
            PlayWriterDirective::Continue
        }
        PlayOutput::KeepAliveRequest(id) => {
            write_packet(
                writer,
                &build_packet(packet_ids.keep_alive_request, |body| {
                    body.extend_from_slice(&id.to_be_bytes());
                    Ok(())
                })?,
            )?;
            PlayWriterDirective::Continue
        }
        PlayOutput::Disconnect(reason) => {
            let payload = encode_play_disconnect(&reason)?;
            write_packet(
                writer,
                &build_packet(packet_ids.disconnect, |body| {
                    body.extend_from_slice(&payload);
                    Ok(())
                })?,
            )?;
            PlayWriterDirective::Stop
        }
    };
    writer.flush()?;
    Ok(directive)
}

#[cfg(test)]
#[allow(dead_code)]
fn run_static_play_session<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &ServerConfig,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_round_limit: Option<usize>,
) -> Result<()> {
    run_static_play_session_with_bridge(
        reader,
        writer,
        config,
        profile,
        session,
        world,
        None,
        None,
        play_round_limit,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "transitional bridge preserves the finite legacy call boundary"
)]
fn run_static_play_session_with_bridge<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &ServerConfig,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_reader: Option<&PlayReaderEndpoint>,
    gameplay: Option<play_runtime::GameplaySync<'_>>,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let _world_subscription = world.shared_world.subscribe(world.connection)?;
    let world_profile = world.shared_world.world_profile();
    let center = play_runtime::spawn_chunk(world_profile);
    let chunk = world.shared_world.chunk_snapshot(center)?;

    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::PlayLogin,
        &encode_join_game(&static_join_game(config, world_profile))?,
        play_reader,
    )?;
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::DefaultSpawnPosition,
        &encode_default_spawn_position(&static_default_spawn_position(world_profile)?)?,
        play_reader,
    )?;
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::SetChunkCacheCenter,
        &encode_set_chunk_cache_center(center.x, center.z),
        play_reader,
    )?;
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::ChunkBatchStart,
        &encode_chunk_batch_start(),
        play_reader,
    )?;
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::LevelChunkWithLight,
        &encode_level_chunk_with_light(&chunk)?,
        play_reader,
    )?;
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::ChunkBatchFinished,
        &encode_chunk_batch_finished(1)?,
        play_reader,
    )?;
    if !config.play_policy.welcome_message.is_empty() {
        write_or_route_play_payload(
            writer,
            profile,
            PacketKind::SystemChat,
            &encode_system_chat(&config.play_policy.welcome_message, false)?,
            play_reader,
        )?;
    }
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::PlayerPosition,
        &encode_player_position(&static_player_position(world_profile))?,
        play_reader,
    )?;
    if play_reader.is_none() {
        writer.flush()?;
    }

    wait_for_play_bootstrap_acknowledgements_with_bridge(
        reader,
        profile,
        STATIC_TELEPORT_ID,
        play_reader,
    )?;
    run_keep_alive_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world,
        play_reader,
        gameplay,
        play_round_limit,
    )
}

fn write_or_route_play_payload<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    kind: PacketKind,
    payload: &[u8],
    play_reader: Option<&PlayReaderEndpoint>,
) -> Result<()> {
    if let Some(play_reader) = play_reader {
        let packet_id = profile.packets().require(kind)?;
        let packet = build_packet(packet_id, |body| {
            body.extend_from_slice(payload);
            Ok(())
        })?;
        play_reader
            .try_submit_output(PlayOutput::Packet(packet))
            .with_context(|| format!("cannot route {kind:?} packet to Play writer"))?;
        return Ok(());
    }
    write_play_payload(writer, profile, kind, payload)
}

fn static_join_game(config: &ServerConfig, world: &RomPackWorld) -> JoinGame {
    JoinGame {
        player_id: STATIC_PLAYER_ID,
        hardcore: false,
        levels: vec![world.dimension.clone()],
        max_players: config.max_players,
        chunk_radius: config.play_policy.chunk_radius,
        simulation_distance: config.play_policy.simulation_distance,
        reduced_debug_info: false,
        show_death_screen: true,
        limited_crafting: false,
        spawn_info: CommonPlayerSpawnInfo {
            dimension_type_id: world.dimension_type_id,
            dimension: world.dimension.clone(),
            seed: 0,
            game_mode: 0,
            previous_game_mode: -1,
            is_debug: false,
            is_flat: true,
            last_death_location: None,
            portal_cooldown: 0,
            sea_level: world.sea_level,
        },
        enforces_secure_chat: config.enforces_secure_chat,
    }
}

fn static_default_spawn_position(world: &RomPackWorld) -> Result<DefaultSpawnPosition> {
    Ok(DefaultSpawnPosition {
        position: GlobalPosition {
            dimension: world.dimension.clone(),
            position: BlockPosition {
                x: world.spawn_x,
                y: world
                    .floor_y
                    .checked_add(1)
                    .context("default spawn y overflow")?,
                z: world.spawn_z,
            },
        },
        yaw: 0.0,
        pitch: 0.0,
    })
}

fn game_spawn_transform(world: &RomPackWorld) -> Result<Transform> {
    Transform::new(play_runtime::player_spawn_position(world), 0.0, 0.0, false)
        .context("generated player spawn transform is invalid")
}

fn static_player_position(world: &RomPackWorld) -> PlayerPosition {
    PlayerPosition {
        teleport_id: STATIC_TELEPORT_ID,
        change: PositionMoveRotation {
            position: play_runtime::player_spawn_position(world),
            delta_movement: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
        },
        relative_flags: 0,
    }
}

fn write_play_payload<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    kind: PacketKind,
    payload: &[u8],
) -> Result<()> {
    write_packet(
        writer,
        &build_packet(profile.packets().require(kind)?, |body| {
            body.extend_from_slice(payload);
            Ok(())
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
fn wait_for_play_bootstrap_acknowledgements<R: Read>(
    reader: &mut R,
    profile: &ProtocolProfile,
    expected_teleport_id: i32,
) -> Result<()> {
    wait_for_play_bootstrap_acknowledgements_with_bridge(
        reader,
        profile,
        expected_teleport_id,
        None,
    )
}

fn wait_for_play_bootstrap_acknowledgements_with_bridge<R: Read>(
    reader: &mut R,
    profile: &ProtocolProfile,
    expected_teleport_id: i32,
    play_reader: Option<&PlayReaderEndpoint>,
) -> Result<()> {
    let teleport_packet_id = profile.packets().require(PacketKind::AcceptTeleportation)?;
    let chunk_batch_packet_id = profile.packets().require(PacketKind::ChunkBatchReceived)?;
    let mut teleport_acknowledged = false;
    let mut chunk_batch_acknowledged = false;

    for _ in 0..MAX_IGNORED_PLAY_PACKETS {
        let packet = read_packet(reader).context("cannot read Play bootstrap acknowledgement")?;
        let mut packet_reader = PacketReader::new(&packet);
        let packet_id = packet_reader.read_varint()?;

        if !teleport_acknowledged && play_runtime::is_movement_packet_id(profile, packet_id) {
            bail!("player movement received before teleport acknowledgement");
        }

        if packet_id == teleport_packet_id {
            let teleport_id = packet_reader.read_varint()?;
            if teleport_id != expected_teleport_id {
                bail!("expected teleport id {expected_teleport_id}, got {teleport_id}");
            }
            if !packet_reader.take_remaining().is_empty() {
                bail!("teleport acknowledgement contains trailing bytes");
            }
            teleport_acknowledged = true;
        } else if packet_id == chunk_batch_packet_id {
            let desired_chunks_per_tick = packet_reader.read_f32()?;
            if !desired_chunks_per_tick.is_finite() || desired_chunks_per_tick <= 0.0 {
                bail!(
                    "chunk batch acknowledgement contains invalid desired chunks per tick {desired_chunks_per_tick}"
                );
            }
            if !packet_reader.take_remaining().is_empty() {
                bail!("chunk batch acknowledgement contains trailing bytes");
            }
            if let Some(play_reader) = play_reader {
                play_reader
                    .try_submit_input(PlayInput::ChunkBatchReceived(desired_chunks_per_tick))
                    .context("cannot route Play bootstrap chunk acknowledgement")?;
            }
            chunk_batch_acknowledged = true;
        }

        if teleport_acknowledged && chunk_batch_acknowledged {
            return Ok(());
        }
    }

    bail!(
        "Play bootstrap acknowledgements were not received within the packet limit: teleport={teleport_acknowledged}, chunk_batch={chunk_batch_acknowledged}"
    )
}

#[cfg(test)]
#[allow(dead_code)]
fn run_keep_alive_loop<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_round_limit: Option<usize>,
) -> Result<()> {
    run_keep_alive_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world,
        None,
        None,
        play_round_limit,
    )
}

fn run_keep_alive_loop_with_bridge<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_reader: Option<&PlayReaderEndpoint>,
    gameplay: Option<play_runtime::GameplaySync<'_>>,
    play_round_limit: Option<usize>,
) -> Result<()> {
    play_runtime::run_play_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world.shared_world,
        world.connection,
        play_reader,
        gameplay,
        play_round_limit,
    )
}

#[cfg(test)]
fn wait_for_keep_alive_response<R: Read>(
    reader: &mut R,
    profile: &ProtocolProfile,
    expected_keep_alive_id: i64,
) -> Result<()> {
    let expected_packet_id = profile.packets().require(PacketKind::KeepAliveResponse)?;
    for _ in 0..MAX_IGNORED_PLAY_PACKETS {
        let packet = read_packet(reader).context("cannot read keep alive response")?;
        let mut packet_reader = PacketReader::new(&packet);
        let packet_id = packet_reader.read_varint()?;
        if packet_id != expected_packet_id {
            continue;
        }
        let keep_alive_id = packet_reader.read_i64()?;
        if keep_alive_id != expected_keep_alive_id {
            bail!("expected keep alive id {expected_keep_alive_id}, got {keep_alive_id}");
        }
        if !packet_reader.take_remaining().is_empty() {
            bail!("keep alive response contains trailing bytes");
        }
        return Ok(());
    }
    bail!("keep alive response was not received within the packet limit")
}

fn is_connection_eof(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|io_error| io_error.kind() == io::ErrorKind::UnexpectedEof)
    })
}

fn is_transient_read_timeout(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<io::Error>().is_some_and(|io_error| {
            matches!(
                io_error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            )
        })
    })
}

fn send_registry_data<W: Write>(
    writer: &mut W,
    registry_payloads: &[Vec<u8>],
    profile: &ProtocolProfile,
) -> Result<()> {
    if registry_payloads.is_empty() {
        return Ok(());
    }

    let packet_id = profile.packets().require(PacketKind::RegistryData)?;
    for body in registry_payloads {
        write_packet(
            writer,
            &build_packet(packet_id, |output| {
                output.extend_from_slice(body);
                Ok(())
            })?,
        )?;
    }
    Ok(())
}

fn write_configuration_disconnect<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    message: &str,
) -> Result<()> {
    let mut body = Vec::new();
    encode_anonymous(&mut body, &Tag::String(message.to_owned()))?;
    write_packet(
        writer,
        &build_packet(
            profile
                .packets()
                .require(PacketKind::ConfigurationDisconnect)?,
            |output| {
                output.extend_from_slice(&body);
                Ok(())
            },
        )?,
    )?;
    Ok(())
}

fn negotiate_known_packs<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &ServerConfig,
    profile: &ProtocolProfile,
) -> Result<Vec<KnownPack>> {
    let offered = config.known_packs();
    if offered.is_empty() {
        return Ok(Vec::new());
    }

    let packet_id = profile
        .packets()
        .require(PacketKind::SelectKnownPacksRequest)?;
    let body = encode_known_packs(&offered)?;
    write_packet(
        writer,
        &build_packet(packet_id, |output| {
            output.extend_from_slice(&body);
            Ok(())
        })?,
    )?;
    writer.flush()?;

    let expected_id = profile
        .packets()
        .require(PacketKind::SelectKnownPacksResponse)?;
    let client_information_id = profile
        .packets()
        .id(PacketKind::ConfigurationClientInformation);
    let mut auxiliary_packets = 0_usize;
    let accepted = loop {
        let response = read_packet(reader).context("cannot read Select Known Packs response")?;
        let mut response_reader = PacketReader::new(&response);
        let received_id = response_reader.read_varint()?;

        if received_id == expected_id {
            break decode_known_packs(
                response_reader.take_remaining(),
                KnownPackDecodeLimits::default(),
            )?;
        }

        if client_information_id == Some(received_id) {
            decode_client_information(response_reader.take_remaining())?;
            auxiliary_packets = auxiliary_packets
                .checked_add(1)
                .context("Configuration auxiliary packet count overflow")?;
            if auxiliary_packets > MAX_CONFIGURATION_AUXILIARY_PACKETS {
                bail!("Configuration auxiliary packet limit exceeded");
            }
            continue;
        }

        bail!("expected Select Known Packs packet id {expected_id}, got {received_id}");
    };
    let unique: BTreeSet<_> = accepted.iter().collect();
    if unique.len() != accepted.len() {
        bail!("Select Known Packs response contains duplicate entries");
    }
    if let Some(unknown) = accepted.iter().find(|pack| !offered.contains(pack)) {
        bail!(
            "client accepted an unoffered known pack {}/{}/{}",
            unknown.namespace,
            unknown.id,
            unknown.version
        );
    }
    Ok(accepted)
}

fn parse_handshake_packet(packet: &[u8], packets: &PacketTable) -> Result<Handshake> {
    let mut reader = PacketReader::new(packet);
    let expected_packet_id = packets.require(PacketKind::Handshake)?;
    let packet_id = reader.read_varint()?;
    if packet_id != expected_packet_id {
        bail!("expected handshake packet id {expected_packet_id}, got {packet_id}");
    }
    Ok(Handshake {
        protocol: reader.read_varint()?,
        server_address: reader.read_string()?,
        server_port: reader.read_u16()?,
        next_state: reader.read_varint()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_nbt::{NamedTag, TagType, encode_named};
    use ferrum_world::anvil::{HEADER_BYTES, REGION_EDGE_CHUNKS, SECTOR_BYTES};
    use std::io::Cursor;

    #[test]
    fn live_writer_frames_authoritative_packets() {
        let mut writer = Vec::new();
        let directive = write_live_play_output(
            &mut writer,
            PlayOutputPacketIds {
                keep_alive_request: 0x43,
                disconnect: 0x44,
            },
            PlayOutput::Packet(vec![0x03, 0xaa]),
        )
        .unwrap();
        assert_eq!(directive, PlayWriterDirective::Continue);
        assert_eq!(
            read_packet(&mut Cursor::new(writer)).unwrap(),
            vec![0x03, 0xaa]
        );
    }

    #[test]
    fn live_writer_encodes_semantic_keep_alive_request() {
        let mut writer = Vec::new();
        let directive = write_live_play_output(
            &mut writer,
            PlayOutputPacketIds {
                keep_alive_request: 0x43,
                disconnect: 0x44,
            },
            PlayOutput::KeepAliveRequest(73),
        )
        .unwrap();
        assert_eq!(directive, PlayWriterDirective::Continue);
        let packet = read_packet(&mut Cursor::new(writer)).unwrap();
        let mut reader = PacketReader::new(&packet);
        assert_eq!(reader.read_varint().unwrap(), 0x43);
        assert_eq!(reader.read_i64().unwrap(), 73);
        assert!(reader.take_remaining().is_empty());
    }

    #[test]
    fn live_writer_encodes_disconnect_and_stops() {
        let mut writer = Vec::new();
        let directive = write_live_play_output(
            &mut writer,
            PlayOutputPacketIds {
                keep_alive_request: 0x43,
                disconnect: 0x44,
            },
            PlayOutput::Disconnect("bye".to_owned()),
        )
        .unwrap();
        assert_eq!(directive, PlayWriterDirective::Stop);
        let packet = read_packet(&mut Cursor::new(writer)).unwrap();
        let mut reader = PacketReader::new(&packet);
        assert_eq!(reader.read_varint().unwrap(), 0x44);
        assert!(!reader.take_remaining().is_empty());
    }

    #[test]
    fn generated_play_metadata_drives_join_and_spawn_payloads() {
        let config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        let mut world = play_runtime::builtin_world_profile();
        world.dimension = "minecraft:test_world".to_owned();
        world.dimension_type_id = 3;
        world.sea_level = 70;
        world.floor_y = 79;
        world.spawn_x = 32;
        world.spawn_z = -17;
        let join = static_join_game(&config, &world);
        assert_eq!(join.levels, ["minecraft:test_world"]);
        assert_eq!(join.spawn_info.dimension_type_id, 3);
        assert_eq!(join.spawn_info.dimension, "minecraft:test_world");
        assert_eq!(join.spawn_info.sea_level, 70);
        let spawn = static_default_spawn_position(&world).unwrap();
        assert_eq!(spawn.position.dimension, "minecraft:test_world");
        assert_eq!(
            spawn.position.position,
            BlockPosition {
                x: 32,
                y: 80,
                z: -17
            }
        );
        assert_eq!(
            static_player_position(&world).change.position,
            [32.5, 81.0, -16.5]
        );
        assert_eq!(play_runtime::spawn_chunk(&world), ChunkPos { x: 2, z: -2 });
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
    fn server_state_can_start_from_loaded_world_store() {
        let world = play_runtime::builtin_world_profile();
        let loaded_position = BlockPos { x: 1, y: 65, z: 1 };
        let loaded_state = BlockStateId::new(world.block_states.stone);
        let mut chunk = StaticChunk::new(
            ChunkPos { x: 0, z: 0 },
            world.overworld_min_section_y,
            world.overworld_section_count,
            BlockStateId::new(world.block_states.air),
            BiomeId::new(world.biomes.plains),
        )
        .unwrap();
        chunk
            .apply_block_mutation(BlockMutation {
                position: loaded_position,
                state: loaded_state,
            })
            .unwrap();
        let mut store = ChunkStore::new();
        store.insert(chunk);

        let state = ServerState::with_loaded_world_runtime(
            0,
            world,
            store,
            Vec::new(),
            PlayPolicy::default(),
        )
        .unwrap();

        assert_eq!(
            state
                .world()
                .chunk_snapshot(ChunkPos { x: 0, z: 0 })
                .unwrap()
                .world_block(loaded_position)
                .unwrap(),
            loaded_state
        );
    }

    #[test]
    fn parses_world_region_file_configuration() {
        let base_dir = PathBuf::from("C:/ferrum/config");
        let config = ServerConfig::from_toml_like_with_base(
            r#"
            [world]
            region_file = "world/region/r.1.-1.mca"
            region_dir = "world/region"
            region_x = 1
            region_z = -1
            "#,
            Some(&base_dir),
        )
        .unwrap_err();
        assert!(
            config
                .to_string()
                .contains("world.region_file and world.region_dir cannot both be set")
        );

        let config = ServerConfig::from_toml_like_with_base(
            r#"
            [world]
            region_file = "world/region/r.1.-1.mca"
            region_x = 1
            region_z = -1
            "#,
            Some(&base_dir),
        )
        .unwrap();

        assert_eq!(
            config.world.region_file.as_deref(),
            Some(base_dir.join("world/region/r.1.-1.mca").as_path())
        );
        assert_eq!(config.world.region_dir, None);
        assert_eq!(config.world.region_x, Some(1));
        assert_eq!(config.world.region_z, Some(-1));

        let config = ServerConfig::from_toml_like_with_base(
            r#"
            [world]
            region_dir = "world/region"
            "#,
            Some(&base_dir),
        )
        .unwrap();
        assert_eq!(
            config.world.region_dir.as_deref(),
            Some(base_dir.join("world/region").as_path())
        );

        let inferred = ServerConfig::from_toml_like_with_base(
            r#"
            [world]
            region_file = "world/region/r.-2.3.mca"
            "#,
            Some(&base_dir),
        )
        .unwrap();
        assert_eq!(
            configured_region_file_pos(
                &inferred.world,
                inferred.world.region_file.as_deref().unwrap()
            )
            .unwrap(),
            RegionPos { x: -2, z: 3 }
        );

        for invalid in [
            "[world]\nregion_file = \"world/region/r.0.0.mca\"\nregion_x = 0",
            "[world]\nregion_dir = \"world/region\"\nregion_x = 0\nregion_z = 0",
            "[world]\nregion_x = 0\nregion_z = 0",
        ] {
            assert!(ServerConfig::from_toml_like_with_base(invalid, Some(&base_dir)).is_err());
        }
    }

    #[test]
    fn configured_anvil_region_can_seed_server_state() {
        let world = play_runtime::builtin_world_profile();
        let region = RegionPos { x: 1, z: -1 };
        let local_x = 1;
        let local_z = 2;
        let chunk_pos = ChunkPos { x: 33, z: -30 };
        let section_y = i8::try_from(world.overworld_min_section_y).unwrap();
        let region_bytes = test_region_with_chunk(
            local_x,
            local_z,
            2,
            1,
            3,
            &encode_named_root(&test_anvil_chunk_root(
                chunk_pos,
                section_y,
                "minecraft:diamond_block",
            )),
        );
        let path = temp_region_file("configured_anvil_region_can_seed_server_state.mca");
        fs::write(&path, region_bytes).unwrap();

        let world_config = WorldConfig {
            region_file: Some(path.clone()),
            region_dir: None,
            region_x: Some(region.x),
            region_z: Some(region.z),
        };
        let store = load_configured_world_chunks(&world_config, &world)
            .unwrap()
            .unwrap();
        let state = ServerState::with_loaded_world_runtime(
            0,
            world.clone(),
            store,
            Vec::new(),
            PlayPolicy::default(),
        )
        .unwrap();
        let position = BlockPos {
            x: chunk_pos.x * 16,
            y: i32::from(section_y) * 16,
            z: chunk_pos.z * 16,
        };

        assert_eq!(
            state
                .world()
                .chunk_snapshot(chunk_pos)
                .unwrap()
                .world_block(position)
                .unwrap(),
            BlockStateId::new(world.block_states.stone)
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn configured_anvil_region_file_infers_position_from_name() {
        let world = play_runtime::builtin_world_profile();
        let dir = temp_region_dir("configured_anvil_region_file_infers_position_from_name");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let local_x = 0;
        let local_z = 31;
        let chunk_pos = ChunkPos { x: 64, z: -33 };
        let section_y = i8::try_from(world.overworld_min_section_y).unwrap();
        let path = dir.join("r.2.-2.mca");
        fs::write(
            &path,
            test_region_with_chunk(
                local_x,
                local_z,
                2,
                1,
                3,
                &encode_named_root(&test_anvil_chunk_root(
                    chunk_pos,
                    section_y,
                    "minecraft:dirt",
                )),
            ),
        )
        .unwrap();

        let store = load_configured_world_chunks(
            &WorldConfig {
                region_file: Some(path),
                region_dir: None,
                region_x: None,
                region_z: None,
            },
            &world,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            store
                .world_block(BlockPos {
                    x: chunk_pos.x * 16,
                    y: i32::from(section_y) * 16,
                    z: chunk_pos.z * 16,
                })
                .unwrap(),
            BlockStateId::new(world.block_states.dirt)
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn configured_anvil_region_directory_loads_all_region_files() {
        let world = play_runtime::builtin_world_profile();
        let dir = temp_region_dir("configured_anvil_region_directory_loads_all_region_files");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let first_chunk = ChunkPos { x: 0, z: 0 };
        let second_chunk = ChunkPos { x: 33, z: -30 };
        let section_y = i8::try_from(world.overworld_min_section_y).unwrap();
        fs::write(
            dir.join("r.0.0.mca"),
            test_region_with_chunk(
                0,
                0,
                2,
                1,
                3,
                &encode_named_root(&test_anvil_chunk_root(
                    first_chunk,
                    section_y,
                    "minecraft:stone",
                )),
            ),
        )
        .unwrap();
        fs::write(
            dir.join("r.1.-1.mca"),
            test_region_with_chunk(
                1,
                2,
                2,
                1,
                3,
                &encode_named_root(&test_anvil_chunk_root(
                    second_chunk,
                    section_y,
                    "minecraft:dirt",
                )),
            ),
        )
        .unwrap();
        fs::write(dir.join("r.2.0.mca"), b"truncated").unwrap();
        fs::write(dir.join("notes.txt"), b"ignored").unwrap();

        let store = load_configured_world_chunks(
            &WorldConfig {
                region_file: None,
                region_dir: Some(dir.clone()),
                region_x: None,
                region_z: None,
            },
            &world,
        )
        .unwrap()
        .unwrap();

        assert_eq!(store.len(), 2);
        assert_eq!(
            store
                .world_block(BlockPos {
                    x: 0,
                    y: i32::from(section_y) * 16,
                    z: 0,
                })
                .unwrap(),
            BlockStateId::new(world.block_states.stone)
        );
        assert_eq!(
            store
                .world_block(BlockPos {
                    x: second_chunk.x * 16,
                    y: i32::from(section_y) * 16,
                    z: second_chunk.z * 16,
                })
                .unwrap(),
            BlockStateId::new(world.block_states.dirt)
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn anvil_conversion_profile_registers_builtin_biomes() {
        let world = play_runtime::builtin_world_profile();
        let profile = anvil_conversion_profile(&world);

        assert_eq!(
            profile.biomes.get("minecraft:badlands").copied(),
            Some(BiomeId::new(0))
        );
        assert_eq!(
            profile.biomes.get("minecraft:plains").copied(),
            Some(BiomeId::new(world.biomes.plains))
        );
        assert_eq!(
            profile.unknown_block_state,
            Some(BlockStateId::new(world.block_states.stone))
        );
        assert_eq!(
            profile.unknown_biome,
            Some(BiomeId::new(world.biomes.plains))
        );
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
            "[play]\nchunk_radius = 9",
            "[play]\nsimulation_distance = 33",
            "[play]\nkeep_alive_interval_seconds = 0",
        ] {
            assert!(ServerConfig::from_toml_like_with_base(invalid, None).is_err());
        }
    }

    #[test]
    fn play_policy_drives_shared_runtime_keep_alive_interval() {
        let mut policy = PlayPolicy {
            keep_alive_interval_seconds: 1,
            ..PlayPolicy::default()
        };
        assert_eq!(
            shared_play_runtime_config(&policy)
                .unwrap()
                .keep_alive_interval_ticks
                .get(),
            20
        );

        policy.keep_alive_interval_seconds = MAX_KEEP_ALIVE_INTERVAL_SECONDS;
        assert_eq!(
            shared_play_runtime_config(&policy)
                .unwrap()
                .keep_alive_interval_ticks
                .get(),
            6_000
        );
    }

    #[test]
    fn generated_registry_manifest_drives_configuration_payloads() {
        let registries = vec![RomPackRegistry {
            id: "minecraft:test_registry".to_owned(),
            entries: vec!["minecraft:alpha".to_owned(), "minecraft:beta".to_owned()],
        }];
        let payloads = registry_payloads_from_pack(&registries).unwrap();
        let expected = encode_registry_data(&RegistryData::new(
            "minecraft:test_registry",
            vec![
                RegistryEntry::new("minecraft:alpha", None),
                RegistryEntry::new("minecraft:beta", None),
            ],
        ))
        .unwrap();
        assert_eq!(payloads, vec![expected]);
    }

    #[test]
    fn parses_expected_config_argument() {
        let cli = Cli::try_parse_from(["ferrum-server", "--config", "server.toml"])
            .expect("expected CLI should parse");

        assert_eq!(cli.config, PathBuf::from("server.toml"));
    }

    #[test]
    fn requires_config_argument() {
        assert!(Cli::try_parse_from(["ferrum-server"]).is_err());
    }

    #[test]
    fn parses_server_config() {
        let config = ServerConfig::from_toml_like_with_base(
            r#"
            [server]
            bind = "127.0.0.1:25566"
            version_name = "Minecraft Java Edition 26.0.0"
            protocol = 2600
            motd = "Ferrum test"
            max_players = 5
            online_players = 1
            login_disconnect_message = "Not ready"
            allow_offline_login = false
            online_mode = false

            hide_online_players = false
            enforces_secure_chat = true
            previews_chat = false
            server_icon = "data:image/png;base64,iVBORw0KGgo="
            sample_players = "Steve:00000000-0000-0000-0000-000000000000;Alex:11111111-1111-1111-1111-111111111111"

            [configuration]
            enabled = true
            features = "minecraft:vanilla;minecraft:trade_rebalance"

            [protocol]
            handshake_serverbound = 0
            status_request_serverbound = 0
            status_response_clientbound = 0
            ping_request_serverbound = 1
            pong_response_clientbound = 1
            login_start_serverbound = 0
            login_disconnect_clientbound = 0
            login_success_clientbound = 2
            login_acknowledged_serverbound = 3
            configuration_acknowledged_serverbound = 4
            configuration_finish_clientbound = 5
            configuration_feature_flags_clientbound = 6
            configuration_tags_clientbound = 7
            configuration_registry_data_clientbound = 8
            "#,
            None,
        )
        .expect("config should parse");

        assert_eq!(config.bind, "127.0.0.1:25566");
        assert_eq!(config.protocol, 2600);
        assert_eq!(config.motd, "Ferrum test");
        assert_eq!(config.max_players, 5);
        assert_eq!(config.online_players, 1);
        assert_eq!(config.login_disconnect_message, "Not ready");
        assert!(!config.allow_offline_login);
        assert!(config.configuration_enabled);
        assert_eq!(
            config.configuration_features,
            ["minecraft:vanilla", "minecraft:trade_rebalance"]
        );
        assert!(!config.online_mode);
        assert!(config.enforces_secure_chat);
        assert!(!config.previews_chat);
        assert_eq!(
            config.server_icon.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        assert_eq!(config.sample_players.len(), 2);
        assert_eq!(config.sample_players[0].name, "Steve");
        assert_eq!(config.packets.ping_request_serverbound, 1);
        assert_eq!(config.packets.login_success_clientbound, 2);
    }

    #[test]
    fn parses_builtin_26_1_2_profile() {
        let config = ServerConfig::from_toml_like_with_base(
            r#"
            [server]
            profile = "26.1.2"
            bind = "127.0.0.1:25565"
            allow_offline_login = true
            online_mode = false
            "#,
            None,
        )
        .expect("built-in profile should parse");

        assert_eq!(config.profile_name.as_deref(), Some("26.1.2"));
        assert_eq!(config.version_name, version_26_1_2::VERSION_NAME);
        assert_eq!(config.protocol, 775);
        assert!(config.configuration_enabled);
        assert_eq!(config.configuration_features, ["minecraft:vanilla"]);

        let profile = config.protocol_profile().unwrap();
        assert_eq!(
            profile
                .packets()
                .require(PacketKind::SelectKnownPacksRequest)
                .unwrap(),
            0x0e
        );
    }

    #[test]
    fn builtin_profile_rejects_manual_packet_overrides() {
        let error = ServerConfig::from_toml_like_with_base(
            r#"
            [server]
            profile = "26.1.2"

            [protocol]
            login_success_clientbound = 99
            "#,
            None,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("manual [protocol] packet IDs cannot be used")
        );
    }

    #[test]
    fn status_json_includes_vanilla_server_list_metadata() {
        let config = ServerConfig {
            protocol: 2600,
            version_name: "Minecraft Java Edition 26.0.0".to_owned(),
            motd: "Ferrum status".to_owned(),
            online_players: 1,
            max_players: 10,
            enforces_secure_chat: true,
            previews_chat: false,
            server_icon: Some("data:image/png;base64,iVBORw0KGgo=".to_owned()),
            sample_players: vec![SamplePlayer {
                name: "Steve".to_owned(),
                id: "00000000-0000-0000-0000-000000000000".to_owned(),
            }],
            ..ServerConfig::default()
        };

        let status: Value =
            serde_json::from_str(&config.status_json()).expect("status should be valid JSON");
        assert_eq!(status["version"]["protocol"], 2600);
        assert_eq!(status["description"]["text"], "Ferrum status");
        assert_eq!(status["players"]["online"], 1);
        assert_eq!(status["players"]["sample"][0]["name"], "Steve");
        assert_eq!(status["enforcesSecureChat"], true);
        assert_eq!(status["previewsChat"], false);
        assert_eq!(status["favicon"], "data:image/png;base64,iVBORw0KGgo=");
    }

    #[test]
    fn status_json_can_use_live_online_player_count() {
        let config = ServerConfig {
            online_players: 0,
            max_players: 10,
            ..ServerConfig::default()
        };
        let state = ServerState::new(&config);

        {
            let _first_player = state.enter_play();
            let _second_player = state.enter_play();

            let mut input = Vec::new();
            write_packet(
                &mut input,
                &build_packet(0, |body| {
                    write_varint_vec(body, 2600);
                    write_string(body, "localhost")?;
                    body.extend_from_slice(&25565u16.to_be_bytes());
                    write_varint_vec(body, 1);
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();
            write_packet(&mut input, &build_packet(0, |_| Ok(())).unwrap()).unwrap();

            let mut output = Vec::new();
            handle_connection_protocol_with_state(Cursor::new(input), &mut output, &config, &state)
                .unwrap();
            let response = read_packet(&mut Cursor::new(output)).unwrap();
            let mut response_reader = PacketReader::new(&response);
            assert_eq!(response_reader.read_varint().unwrap(), 0);
            let status: Value = serde_json::from_str(&response_reader.read_string().unwrap())
                .expect("status should be valid JSON");
            assert_eq!(status["players"]["online"], 2);
        }

        let status: Value =
            serde_json::from_str(&config.status_json_with_online_players(state.online_players()))
                .expect("status should be valid JSON");
        assert_eq!(status["players"]["online"], 0);
    }

    #[test]
    fn can_hide_online_players_in_status_json() {
        let config = ServerConfig {
            hide_online_players: true,
            ..ServerConfig::default()
        };
        let status: Value =
            serde_json::from_str(&config.status_json()).expect("status should be valid JSON");
        assert!(status.get("players").is_none());
    }

    #[test]
    fn base64_encoder_matches_png_header_fixture() {
        assert_eq!(
            base64_encode(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            "iVBORw0KGgo="
        );
    }

    #[test]
    fn varint_round_trips_protocol_values() {
        for value in [0, 1, 127, 128, 255, 2_600, i32::MAX, -1] {
            let mut encoded = Vec::new();
            write_varint_vec(&mut encoded, value);
            let mut cursor = Cursor::new(encoded);
            assert_eq!(read_varint_io(&mut cursor).unwrap(), value);
        }
    }

    #[test]
    fn parses_handshake_packet() {
        let packet = build_packet(0, |body| {
            write_varint_vec(body, 2600);
            write_string(body, "localhost")?;
            body.extend_from_slice(&25565u16.to_be_bytes());
            write_varint_vec(body, 1);
            Ok(())
        })
        .unwrap();

        let profile = ServerConfig::default().protocol_profile().unwrap();

        assert_eq!(
            parse_handshake_packet(&packet, profile.packets()).unwrap(),
            Handshake {
                protocol: 2600,
                server_address: "localhost".to_owned(),
                server_port: 25565,
                next_state: 1,
            }
        );
    }

    #[test]
    fn answers_status_request_and_ping() {
        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(&mut input, &build_packet(0, |_| Ok(())).unwrap()).unwrap();
        write_packet(
            &mut input,
            &build_packet(1, |body| {
                body.extend_from_slice(&12345i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();

        let config = ServerConfig {
            protocol: 2600,
            version_name: "Minecraft Java Edition 26.0.0".to_owned(),
            motd: "Ferrum status".to_owned(),
            ..ServerConfig::default()
        };
        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();

        let mut cursor = Cursor::new(output);
        let response = read_packet(&mut cursor).unwrap();
        let mut response_reader = PacketReader::new(&response);
        assert_eq!(response_reader.read_varint().unwrap(), 0);
        let status = response_reader.read_string().unwrap();
        assert!(status.contains("Minecraft Java Edition 26.0.0"));
        assert!(status.contains("Ferrum status"));

        let pong = read_packet(&mut cursor).unwrap();
        let mut pong_reader = PacketReader::new(&pong);
        assert_eq!(pong_reader.read_varint().unwrap(), 1);
        assert_eq!(pong_reader.read_i64().unwrap(), 12345);
    }

    #[test]
    fn disconnects_login_attempts_with_configured_message() {
        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0, |body| write_string(body, "sakus")).expect("login start should build"),
        )
        .unwrap();

        let config = ServerConfig {
            login_disconnect_message: "Play login is not implemented yet".to_owned(),
            ..ServerConfig::default()
        };
        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();

        let mut cursor = Cursor::new(output);
        let disconnect = read_packet(&mut cursor).unwrap();
        let mut disconnect_reader = PacketReader::new(&disconnect);
        assert_eq!(disconnect_reader.read_varint().unwrap(), 0);
        let reason = disconnect_reader.read_string().unwrap();
        assert!(reason.contains("Play login is not implemented yet"));
    }

    #[test]
    fn builtin_profile_disconnects_mismatched_protocol_before_login_start() {
        let config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 774);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();

        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();
        let disconnect = read_packet(&mut Cursor::new(output)).unwrap();
        let mut reader = PacketReader::new(&disconnect);
        assert_eq!(reader.read_varint().unwrap(), 0x00);
        let reason = reader.read_string().unwrap();
        assert!(reason.contains("Unsupported protocol 774"));
        assert!(reason.contains("protocol 775"));
    }

    #[test]
    fn can_accept_offline_login_with_login_success() {
        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0, |body| write_string(body, "Steve")).expect("login start should build"),
        )
        .unwrap();

        let config = ServerConfig {
            allow_offline_login: true,
            ..ServerConfig::default()
        };
        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();

        let mut cursor = Cursor::new(output);
        let login_success = read_packet(&mut cursor).unwrap();
        let mut success_reader = PacketReader::new(&login_success);
        assert_eq!(success_reader.read_varint().unwrap(), 2);
        assert_eq!(
            success_reader.read_uuid_bytes().unwrap(),
            [
                0x56, 0x27, 0xdd, 0x98, 0xe6, 0xbe, 0x3c, 0x21, 0xb8, 0xa8, 0xe9, 0x23, 0x44, 0x18,
                0x36, 0x41
            ]
        );
        assert_eq!(success_reader.read_string().unwrap(), "Steve");
        assert_eq!(success_reader.read_varint().unwrap(), 0);
    }

    #[test]
    fn completes_the_configured_configuration_sequence() {
        let packets = PacketIds {
            login_acknowledged_serverbound: Some(3),
            configuration_acknowledged_serverbound: Some(4),
            configuration_finish_clientbound: Some(5),
            configuration_feature_flags_clientbound: Some(6),
            configuration_tags_clientbound: Some(7),
            ..PacketIds::default()
        };
        let config = ServerConfig {
            allow_offline_login: true,
            configuration_enabled: true,
            configuration_features: vec!["minecraft:vanilla".to_owned()],
            packets,
            ..ServerConfig::default()
        };

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0, |body| write_string(body, "Steve")).unwrap(),
        )
        .unwrap();
        write_packet(&mut input, &build_packet(3, |_| Ok(())).unwrap()).unwrap();
        write_packet(&mut input, &build_packet(4, |_| Ok(())).unwrap()).unwrap();

        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();
        let mut cursor = Cursor::new(output);

        let login_success = read_packet(&mut cursor).unwrap();
        assert_eq!(PacketReader::new(&login_success).read_varint().unwrap(), 2);

        let feature_flags = read_packet(&mut cursor).unwrap();
        let mut feature_reader = PacketReader::new(&feature_flags);
        assert_eq!(feature_reader.read_varint().unwrap(), 6);
        assert_eq!(feature_reader.read_varint().unwrap(), 1);
        assert_eq!(feature_reader.read_string().unwrap(), "minecraft:vanilla");

        let tags = read_packet(&mut cursor).unwrap();
        let mut tags_reader = PacketReader::new(&tags);
        assert_eq!(tags_reader.read_varint().unwrap(), 7);
        assert_eq!(tags_reader.read_varint().unwrap(), 0);

        let finish = read_packet(&mut cursor).unwrap();
        assert_eq!(PacketReader::new(&finish).read_varint().unwrap(), 5);
    }

    #[test]
    fn generated_packet_ids_drive_the_runtime_profile() {
        let built_in = version_26_1_2::protocol_profile().unwrap();
        let mut packets: Vec<_> = built_in
            .packets()
            .iter()
            .map(|(kind, id)| RomPackPacket { kind, id })
            .collect();
        packets
            .iter_mut()
            .find(|packet| packet.kind == PacketKind::SystemChat)
            .unwrap()
            .id = 0x7a;
        let profile = protocol_profile_from_packets(
            version_26_1_2::VERSION_NAME,
            version_26_1_2::PROTOCOL_VERSION,
            &packets,
        )
        .unwrap();
        assert_eq!(
            profile.packets().require(PacketKind::SystemChat).unwrap(),
            0x7a
        );
    }

    #[test]
    fn completes_builtin_26_1_2_known_pack_configuration_sequence() {
        let mut config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        config.allow_offline_login = true;

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 775);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_string(body, "Steve")?;
                body.extend_from_slice(&[0; 16]);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(&mut input, &build_packet(0x03, |_| Ok(())).unwrap()).unwrap();
        write_packet(
            &mut input,
            &build_packet(0x00, |body| {
                body.extend_from_slice(&[
                    0x05, b'e', b'n', b'_', b'u', b's', 0x02, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
                    0x00,
                ]);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let accepted = encode_known_packs(&version_26_1_2::known_packs()).unwrap();
        write_packet(
            &mut input,
            &build_packet(0x07, |body| {
                body.extend_from_slice(&accepted);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(&mut input, &build_packet(0x03, |_| Ok(())).unwrap()).unwrap();
        write_packet(
            &mut input,
            &build_packet(0x0b, |body| {
                body.extend_from_slice(&1.0_f32.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x00, |body| {
                write_varint_vec(body, STATIC_TELEPORT_ID);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x1c, |body| {
                body.extend_from_slice(&1_i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();

        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();
        let mut cursor = Cursor::new(output);

        let login_success = read_packet(&mut cursor).unwrap();
        assert_eq!(
            PacketReader::new(&login_success).read_varint().unwrap(),
            0x02
        );

        let known_packs = read_packet(&mut cursor).unwrap();
        let mut known_packs_reader = PacketReader::new(&known_packs);
        assert_eq!(known_packs_reader.read_varint().unwrap(), 0x0e);
        assert_eq!(
            decode_known_packs(
                known_packs_reader.take_remaining(),
                KnownPackDecodeLimits::default(),
            )
            .unwrap(),
            version_26_1_2::known_packs()
        );

        let features = read_packet(&mut cursor).unwrap();
        let mut features_reader = PacketReader::new(&features);
        assert_eq!(features_reader.read_varint().unwrap(), 0x0c);
        assert_eq!(features_reader.read_varint().unwrap(), 1);
        assert_eq!(features_reader.read_string().unwrap(), "minecraft:vanilla");

        for expected in version_26_1_2::SYNCHRONIZED_REGISTRIES {
            let packet = read_packet(&mut cursor).unwrap();
            let mut registry_reader = PacketReader::new(&packet);
            assert_eq!(registry_reader.read_varint().unwrap(), 0x07);
            assert_eq!(registry_reader.read_string().unwrap(), expected.id);
            assert_eq!(
                registry_reader.read_varint().unwrap(),
                i32::try_from(expected.entries.len()).unwrap()
            );
        }

        let tags = read_packet(&mut cursor).unwrap();
        let mut tags_reader = PacketReader::new(&tags);
        assert_eq!(tags_reader.read_varint().unwrap(), 0x0d);
        assert_eq!(tags_reader.read_varint().unwrap(), 0);

        let finish = read_packet(&mut cursor).unwrap();
        assert_eq!(PacketReader::new(&finish).read_varint().unwrap(), 0x03);

        let world_profile = play_runtime::builtin_world_profile();
        let join_game = read_packet(&mut cursor).unwrap();
        let mut join_game_reader = PacketReader::new(&join_game);
        assert_eq!(join_game_reader.read_varint().unwrap(), 0x31);
        assert_eq!(
            join_game_reader.take_remaining(),
            encode_join_game(&static_join_game(&config, &world_profile)).unwrap()
        );

        let default_spawn = read_packet(&mut cursor).unwrap();
        let mut default_spawn_reader = PacketReader::new(&default_spawn);
        assert_eq!(default_spawn_reader.read_varint().unwrap(), 0x61);
        assert_eq!(
            default_spawn_reader.take_remaining(),
            encode_default_spawn_position(&static_default_spawn_position(&world_profile).unwrap(),)
                .unwrap()
        );

        let chunk_center = read_packet(&mut cursor).unwrap();
        let mut chunk_center_reader = PacketReader::new(&chunk_center);
        assert_eq!(chunk_center_reader.read_varint().unwrap(), 0x5e);
        assert_eq!(chunk_center_reader.take_remaining(), &[0, 0]);

        let chunk_batch_start = read_packet(&mut cursor).unwrap();
        let mut chunk_batch_start_reader = PacketReader::new(&chunk_batch_start);
        assert_eq!(chunk_batch_start_reader.read_varint().unwrap(), 0x0c);
        assert!(chunk_batch_start_reader.take_remaining().is_empty());

        let level_chunk = read_packet(&mut cursor).unwrap();
        let mut level_chunk_reader = PacketReader::new(&level_chunk);
        assert_eq!(level_chunk_reader.read_varint().unwrap(), 0x2d);
        assert_eq!(
            level_chunk_reader.take_remaining(),
            encode_level_chunk_with_light(
                &play_runtime::SharedWorld::static_flat()
                    .chunk_snapshot(ChunkPos { x: 0, z: 0 })
                    .unwrap(),
            )
            .unwrap()
        );

        let chunk_batch_finished = read_packet(&mut cursor).unwrap();
        let mut chunk_batch_finished_reader = PacketReader::new(&chunk_batch_finished);
        assert_eq!(chunk_batch_finished_reader.read_varint().unwrap(), 0x0b);
        assert_eq!(chunk_batch_finished_reader.read_varint().unwrap(), 1);
        assert!(chunk_batch_finished_reader.take_remaining().is_empty());

        let system_chat = read_packet(&mut cursor).unwrap();
        let mut system_chat_reader = PacketReader::new(&system_chat);
        assert_eq!(system_chat_reader.read_varint().unwrap(), 0x79);
        assert_eq!(
            system_chat_reader.take_remaining(),
            encode_system_chat(DEFAULT_WELCOME_MESSAGE, false).unwrap()
        );

        let player_position = read_packet(&mut cursor).unwrap();
        let mut player_position_reader = PacketReader::new(&player_position);
        assert_eq!(player_position_reader.read_varint().unwrap(), 0x48);
        assert_eq!(
            player_position_reader.take_remaining(),
            encode_player_position(&static_player_position(&world_profile)).unwrap()
        );

        let keep_alive = read_packet(&mut cursor).unwrap();
        let mut keep_alive_reader = PacketReader::new(&keep_alive);
        assert_eq!(keep_alive_reader.read_varint().unwrap(), 0x2c);
        assert_eq!(keep_alive_reader.read_i64().unwrap(), 1);
        assert!(keep_alive_reader.take_remaining().is_empty());
        assert!(read_packet(&mut cursor).is_err());
    }

    #[test]
    fn builtin_profile_rejects_wrong_teleport_acknowledgement() {
        let config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        let profile = config.protocol_profile().unwrap();
        let mut framed = Vec::new();
        write_packet(
            &mut framed,
            &build_packet(0x0b, |body| {
                body.extend_from_slice(&1.0_f32.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut framed,
            &build_packet(0x00, |body| {
                write_varint_vec(body, STATIC_TELEPORT_ID + 1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();

        let error = wait_for_play_bootstrap_acknowledgements(
            &mut Cursor::new(framed),
            &profile,
            STATIC_TELEPORT_ID,
        )
        .unwrap_err();
        assert!(error.to_string().contains("expected teleport id 1, got 2"));
    }

    #[test]
    fn builtin_profile_rejects_invalid_chunk_batch_rate() {
        let config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        let profile = config.protocol_profile().unwrap();
        let mut framed = Vec::new();
        write_packet(
            &mut framed,
            &build_packet(0x0b, |body| {
                body.extend_from_slice(&f32::NAN.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();

        let error = wait_for_play_bootstrap_acknowledgements(
            &mut Cursor::new(framed),
            &profile,
            STATIC_TELEPORT_ID,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid desired chunks per tick")
        );
    }

    #[test]
    fn builtin_profile_rejects_wrong_keep_alive_response() {
        let config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        let profile = config.protocol_profile().unwrap();
        let packet = build_packet(0x1c, |body| {
            body.extend_from_slice(&2_i64.to_be_bytes());
            Ok(())
        })
        .unwrap();
        let mut framed = Vec::new();
        write_packet(&mut framed, &packet).unwrap();

        let error =
            wait_for_keep_alive_response(&mut Cursor::new(framed), &profile, 1).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("expected keep alive id 1, got 2")
        );
    }

    #[test]
    fn builtin_profile_disconnects_when_core_pack_is_declined() {
        let mut config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        config.allow_offline_login = true;

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 775);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_string(body, "Steve")?;
                body.extend_from_slice(&[0; 16]);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(&mut input, &build_packet(0x03, |_| Ok(())).unwrap()).unwrap();
        let declined = encode_known_packs(&[]).unwrap();
        write_packet(
            &mut input,
            &build_packet(0x07, |body| {
                body.extend_from_slice(&declined);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();

        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();
        let mut cursor = Cursor::new(output);

        assert_eq!(
            PacketReader::new(&read_packet(&mut cursor).unwrap())
                .read_varint()
                .unwrap(),
            0x02
        );
        assert_eq!(
            PacketReader::new(&read_packet(&mut cursor).unwrap())
                .read_varint()
                .unwrap(),
            0x0e
        );
        let disconnect = read_packet(&mut cursor).unwrap();
        let mut disconnect_reader = PacketReader::new(&disconnect);
        assert_eq!(disconnect_reader.read_varint().unwrap(), 0x02);
        assert_eq!(
            ferrum_nbt::decode_anonymous(disconnect_reader.take_remaining()).unwrap(),
            Tag::String(
                "Minecraft 26.1.2 requires the bundled minecraft/core/26.1.2 data pack".to_owned()
            )
        );
        assert!(read_packet(&mut cursor).is_err());
    }

    #[test]
    fn rejects_unoffered_known_pack_response() {
        let config = ServerConfig::for_profile(Some("26.1.2")).unwrap();
        let profile = config.protocol_profile().unwrap();
        let body = encode_known_packs(&[KnownPack::new("example", "unknown", "1")]).unwrap();
        let response = build_packet(0x07, |packet| {
            packet.extend_from_slice(&body);
            Ok(())
        })
        .unwrap();
        let mut framed = Vec::new();
        write_packet(&mut framed, &response).unwrap();
        let mut output = Vec::new();
        let error = negotiate_known_packs(&mut Cursor::new(framed), &mut output, &config, &profile)
            .unwrap_err();
        assert!(error.to_string().contains("unoffered known pack"));
    }

    #[test]
    fn supports_configured_packet_ids() {
        let packets = PacketIds {
            status_request_serverbound: 9,
            status_response_clientbound: 10,
            ping_request_serverbound: 11,
            pong_response_clientbound: 12,
            login_start_serverbound: 13,
            login_disconnect_clientbound: 14,
            login_success_clientbound: 15,
            ..PacketIds::default()
        };
        let config = ServerConfig {
            packets,
            login_disconnect_message: "custom packet table".to_owned(),
            ..ServerConfig::default()
        };

        let mut status_input = Vec::new();
        write_packet(
            &mut status_input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut status_input,
            &build_packet(9, |_| Ok(())).expect("custom status request should build"),
        )
        .unwrap();
        write_packet(
            &mut status_input,
            &build_packet(11, |body| {
                body.extend_from_slice(&99i64.to_be_bytes());
                Ok(())
            })
            .expect("custom ping should build"),
        )
        .unwrap();
        let mut status_output = Vec::new();
        handle_connection_protocol(Cursor::new(status_input), &mut status_output, &config).unwrap();
        let mut status_cursor = Cursor::new(status_output);
        let status_response = read_packet(&mut status_cursor).unwrap();
        assert_eq!(
            PacketReader::new(&status_response).read_varint().unwrap(),
            10
        );
        let pong_response = read_packet(&mut status_cursor).unwrap();
        assert_eq!(PacketReader::new(&pong_response).read_varint().unwrap(), 12);

        let mut login_input = Vec::new();
        write_packet(
            &mut login_input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut login_input,
            &build_packet(13, |body| write_string(body, "sakus"))
                .expect("custom login start should build"),
        )
        .unwrap();
        let mut login_output = Vec::new();
        handle_connection_protocol(Cursor::new(login_input), &mut login_output, &config).unwrap();
        let mut login_cursor = Cursor::new(login_output);
        let disconnect = read_packet(&mut login_cursor).unwrap();
        assert_eq!(PacketReader::new(&disconnect).read_varint().unwrap(), 14);
    }

    fn test_anvil_chunk_root(pos: ChunkPos, section_y: i8, block_state: &str) -> NamedTag {
        let mut root = BTreeMap::new();
        root.insert("DataVersion".to_owned(), Tag::Int(4444));
        root.insert("xPos".to_owned(), Tag::Int(pos.x));
        root.insert("zPos".to_owned(), Tag::Int(pos.z));
        root.insert(
            "sections".to_owned(),
            Tag::List {
                element_type: TagType::Compound,
                elements: vec![test_anvil_section(section_y, block_state)],
            },
        );
        NamedTag::new("", Tag::Compound(root))
    }

    fn test_anvil_section(section_y: i8, block_state: &str) -> Tag {
        let mut section = BTreeMap::new();
        section.insert("Y".to_owned(), Tag::Byte(section_y));
        section.insert(
            "block_states".to_owned(),
            Tag::Compound(test_block_states(block_state)),
        );
        section.insert("biomes".to_owned(), Tag::Compound(test_biomes()));
        Tag::Compound(section)
    }

    fn test_block_states(block_state: &str) -> BTreeMap<String, Tag> {
        let mut container = BTreeMap::new();
        let mut entry = BTreeMap::new();
        entry.insert("Name".to_owned(), Tag::String(block_state.to_owned()));
        container.insert(
            "palette".to_owned(),
            Tag::List {
                element_type: TagType::Compound,
                elements: vec![Tag::Compound(entry)],
            },
        );
        container
    }

    fn test_biomes() -> BTreeMap<String, Tag> {
        let mut container = BTreeMap::new();
        container.insert(
            "palette".to_owned(),
            Tag::List {
                element_type: TagType::String,
                elements: vec![Tag::String("minecraft:plains".to_owned())],
            },
        );
        container
    }

    fn encode_named_root(root: &NamedTag) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_named(&mut bytes, root).unwrap();
        bytes
    }

    fn test_region_with_chunk(
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
        sector_count: u8,
        compression: u8,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut region =
            vec![0_u8; (sector_offset as usize + sector_count as usize) * SECTOR_BYTES];
        write_region_location(
            &mut region[..HEADER_BYTES],
            local_x,
            local_z,
            sector_offset,
            sector_count,
        );
        let start = sector_offset as usize * SECTOR_BYTES;
        let declared_len = u32::try_from(payload.len() + 1).unwrap();
        region[start..start + 4].copy_from_slice(&declared_len.to_be_bytes());
        region[start + 4] = compression;
        region[start + 5..start + 5 + payload.len()].copy_from_slice(payload);
        region
    }

    fn write_region_location(
        header: &mut [u8],
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
        sector_count: u8,
    ) {
        let index = local_x + local_z * REGION_EDGE_CHUNKS;
        let bytes = sector_offset.to_be_bytes();
        header[index * 4..index * 4 + 3].copy_from_slice(&bytes[1..]);
        header[index * 4 + 3] = sector_count;
    }

    fn temp_region_file(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("ferrum-server-{}-{name}", std::process::id()));
        path
    }

    fn temp_region_dir(name: &str) -> PathBuf {
        temp_region_file(name)
    }
}
