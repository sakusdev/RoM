use std::{
    fs::{self, File},
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use ferrum_rompack::RomPackWorld;
use ferrum_world::{ChunkStore, WorldSnapshot};

use crate::play_runtime::SharedWorld;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone)]
pub(super) struct WorldServiceConfig {
    pub autosave_interval: Option<Duration>,
    pub snapshot_path: Option<PathBuf>,
    pub command_capacity: NonZeroUsize,
    pub poll_interval: Duration,
}

impl WorldServiceConfig {
    pub(super) fn new(snapshot_path: PathBuf, autosave_interval: Option<Duration>) -> Self {
        Self {
            autosave_interval,
            snapshot_path: Some(snapshot_path),
            command_capacity: NonZeroUsize::new(16).expect("16 is non-zero"),
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

impl Default for WorldServiceConfig {
    fn default() -> Self {
        Self {
            autosave_interval: None,
            snapshot_path: None,
            command_capacity: NonZeroUsize::new(16).expect("16 is non-zero"),
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorldSaveReport {
    pub path: PathBuf,
    pub bytes: u64,
    pub chunks: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct WorldServiceExit {
    pub autosaves: u64,
    pub requested_saves: u64,
}

#[derive(Debug)]
enum WorldServiceCommand {
    SaveNow {
        reply: SyncSender<Result<WorldSaveReport, String>>,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub(super) struct WorldServiceControl {
    commands: SyncSender<WorldServiceCommand>,
}

impl WorldServiceControl {
    pub(super) fn save_now(&self) -> Result<WorldSaveReport> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(WorldServiceCommand::SaveNow { reply })
            .context("world service is disconnected")?;
        response
            .recv()
            .context("world service dropped the save response")?
            .map_err(anyhow::Error::msg)
    }

    fn try_shutdown(&self) {
        match self.commands.try_send(WorldServiceCommand::Shutdown) {
            Ok(())
            | Err(TrySendError::Full(WorldServiceCommand::Shutdown))
            | Err(TrySendError::Disconnected(WorldServiceCommand::Shutdown)) => {}
            Err(TrySendError::Full(WorldServiceCommand::SaveNow { .. }))
            | Err(TrySendError::Disconnected(WorldServiceCommand::SaveNow { .. })) => {
                unreachable!("shutdown sends only Shutdown commands")
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct WorldService {
    control: WorldServiceControl,
    worker: Option<JoinHandle<Result<WorldServiceExit>>>,
}

impl WorldService {
    #[must_use]
    pub(super) fn control(&self) -> WorldServiceControl {
        self.control.clone()
    }

    pub(super) fn shutdown(mut self) -> Result<WorldServiceExit> {
        self.control.try_shutdown();
        let worker = self
            .worker
            .take()
            .context("world service worker was already joined")?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("world service worker panicked"))?
    }
}

impl Drop for WorldService {
    fn drop(&mut self) {
        self.control.try_shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(super) fn spawn_world_service(
    world: Arc<SharedWorld>,
    config: WorldServiceConfig,
) -> Result<WorldService> {
    validate_config(&config)?;
    let (commands, receiver) = sync_channel(config.command_capacity.get());
    let control = WorldServiceControl { commands };
    let worker = thread::Builder::new()
        .name("rom-world-service".to_owned())
        .spawn(move || run_world_service(world, config, receiver))
        .context("cannot spawn world persistence service")?;
    Ok(WorldService {
        control,
        worker: Some(worker),
    })
}

pub(super) fn load_world_state(path: &Path, profile: &RomPackWorld) -> Result<Option<ChunkStore>> {
    if !path.exists() {
        return Ok(None);
    }
    if !path.is_file() {
        bail!("world snapshot {} is not a file", path.display());
    }
    let json = fs::read_to_string(path)
        .with_context(|| format!("cannot read world snapshot {}", path.display()))?;
    let snapshot = WorldSnapshot::from_json(&json)
        .with_context(|| format!("cannot decode world snapshot {}", path.display()))?;
    let store = ChunkStore::restore(snapshot)
        .with_context(|| format!("cannot restore world snapshot {}", path.display()))?;
    validate_store_profile(&store, profile)?;
    Ok(Some(store))
}

pub(super) fn save_world_state(world: &SharedWorld, path: &Path) -> Result<WorldSaveReport> {
    let store = world.store_snapshot()?;
    let chunks = store.len();
    let json = store.snapshot().to_json_pretty()?;
    write_atomic(path, json.as_bytes())?;
    Ok(WorldSaveReport {
        path: path.to_path_buf(),
        bytes: u64::try_from(json.len()).context("world snapshot size exceeds u64")?,
        chunks,
    })
}

fn validate_config(config: &WorldServiceConfig) -> Result<()> {
    if config
        .snapshot_path
        .as_ref()
        .is_some_and(|path| path.as_os_str().is_empty())
    {
        bail!("world snapshot path cannot be empty");
    }
    if config.poll_interval.is_zero() {
        bail!("world service poll interval must be greater than zero");
    }
    if config
        .autosave_interval
        .is_some_and(|interval| interval.is_zero())
    {
        bail!("world autosave interval must be greater than zero");
    }
    if config.autosave_interval.is_some() && config.snapshot_path.is_none() {
        bail!("world autosave requires a snapshot path");
    }
    Ok(())
}

fn validate_store_profile(store: &ChunkStore, profile: &RomPackWorld) -> Result<()> {
    if store.is_empty() {
        bail!("world snapshot contains no chunks");
    }
    for (position, chunk) in store.iter() {
        if chunk.min_section_y() != profile.overworld_min_section_y {
            bail!(
                "world snapshot chunk ({}, {}) starts at section {}, expected {}",
                position.x,
                position.z,
                chunk.min_section_y(),
                profile.overworld_min_section_y
            );
        }
        if chunk.sections().len() != profile.overworld_section_count {
            bail!(
                "world snapshot chunk ({}, {}) has {} sections, expected {}",
                position.x,
                position.z,
                chunk.sections().len(),
                profile.overworld_section_count
            );
        }
    }
    Ok(())
}

fn run_world_service(
    world: Arc<SharedWorld>,
    config: WorldServiceConfig,
    commands: Receiver<WorldServiceCommand>,
) -> Result<WorldServiceExit> {
    let mut exit = WorldServiceExit::default();
    let mut next_autosave = config
        .autosave_interval
        .map(|interval| Instant::now() + interval);

    loop {
        let wait = next_autosave
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .unwrap_or(config.poll_interval)
            .min(config.poll_interval);
        match commands.recv_timeout(wait) {
            Ok(WorldServiceCommand::SaveNow { reply }) => {
                exit.requested_saves = exit.requested_saves.saturating_add(1);
                let result = match &config.snapshot_path {
                    Some(path) => {
                        save_world_state(&world, path).map_err(|error| format!("{error:#}"))
                    }
                    None => Err("world snapshot path is not configured".to_owned()),
                };
                let _ = reply.send(result);
            }
            Ok(WorldServiceCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                if let Some(path) = &config.snapshot_path {
                    save_world_state(&world, path)?;
                    exit.requested_saves = exit.requested_saves.saturating_add(1);
                }
                return Ok(exit);
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        let now = Instant::now();
        if next_autosave.is_some_and(|deadline| now >= deadline) {
            if let Some(path) = &config.snapshot_path {
                save_world_state(&world, path)?;
                exit.autosaves = exit.autosaves.saturating_add(1);
            }
            next_autosave = config.autosave_interval.map(|interval| now + interval);
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "cannot create world snapshot directory {}",
            parent.display()
        )
    })?;
    let file_name = path
        .file_name()
        .context("world snapshot path has no file name")?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.tmp"));
    let mut file = File::create(&temporary)
        .with_context(|| format!("cannot create temporary snapshot {}", temporary.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("cannot write temporary snapshot {}", temporary.display()))?;
    file.sync_all()
        .with_context(|| format!("cannot sync temporary snapshot {}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("cannot replace world snapshot {}", path.display()))?;
    }
    fs::rename(&temporary, path).with_context(|| {
        format!(
            "cannot move temporary snapshot {} to {}",
            temporary.display(),
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play_runtime::{SharedWorld, builtin_world_profile, spawn_chunk};
    use ferrum_runtime::ConnectionId;
    use ferrum_world::{BlockMutation, BlockPos, BlockStateId, WorldEvent};

    fn temporary_directory(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rom-world-service-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn saves_and_loads_modified_world() {
        let profile = builtin_world_profile();
        let world = SharedWorld::new(spawn_chunk(&profile), profile.clone()).unwrap();
        world
            .apply_event(
                ConnectionId::new(1),
                WorldEvent::BlockMutation(BlockMutation {
                    position: BlockPos { x: 1, y: 65, z: 1 },
                    state: BlockStateId::new(profile.block_states.stone),
                }),
            )
            .unwrap();
        let directory = temporary_directory("round-trip");
        let path = directory.join("world-state.json");
        save_world_state(&world, &path).unwrap();
        let restored = load_world_state(&path, &profile).unwrap().unwrap();
        assert_eq!(
            restored
                .world_block(BlockPos { x: 1, y: 65, z: 1 })
                .unwrap(),
            BlockStateId::new(profile.block_states.stone)
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_snapshot_with_wrong_section_profile() {
        let profile = builtin_world_profile();
        let world = SharedWorld::new(spawn_chunk(&profile), profile.clone()).unwrap();
        let directory = temporary_directory("profile");
        let path = directory.join("world-state.json");
        save_world_state(&world, &path).unwrap();
        let mut wrong = profile.clone();
        wrong.overworld_section_count += 1;
        assert!(load_world_state(&path, &wrong).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
