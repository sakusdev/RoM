use std::{
    fs::{self, File},
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use rom_game::{GameSnapshot, GameState};

use crate::game_runtime::SharedGameRuntime;

pub const VANILLA_TICK_INTERVAL: Duration = Duration::from_millis(50);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_CATCH_UP_TICKS: u32 = 4;

#[derive(Debug, Clone)]
pub struct GameServiceConfig {
    pub tick_interval: Duration,
    pub autosave_interval: Option<Duration>,
    pub snapshot_path: Option<PathBuf>,
    pub command_capacity: NonZeroUsize,
    pub poll_interval: Duration,
}

impl Default for GameServiceConfig {
    fn default() -> Self {
        Self {
            tick_interval: VANILLA_TICK_INTERVAL,
            autosave_interval: None,
            snapshot_path: None,
            command_capacity: NonZeroUsize::new(32).expect("32 is non-zero"),
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSaveReport {
    pub path: PathBuf,
    pub bytes: u64,
    pub game_time: u64,
    pub players: usize,
    pub entities: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameServiceExit {
    pub ticks: u64,
    pub dropped_ticks: u64,
    pub autosaves: u64,
    pub requested_saves: u64,
}

#[derive(Debug)]
enum GameServiceCommand {
    SaveNow {
        reply: SyncSender<Result<GameSaveReport, String>>,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct GameServiceControl {
    commands: SyncSender<GameServiceCommand>,
}

impl GameServiceControl {
    pub fn save_now(&self) -> Result<GameSaveReport> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(GameServiceCommand::SaveNow { reply })
            .context("game service is disconnected")?;
        response
            .recv()
            .context("game service dropped the save response")?
            .map_err(anyhow::Error::msg)
    }

    pub fn try_shutdown(&self) {
        match self.commands.try_send(GameServiceCommand::Shutdown) {
            Ok(())
            | Err(TrySendError::Full(GameServiceCommand::Shutdown))
            | Err(TrySendError::Disconnected(GameServiceCommand::Shutdown)) => {}
            Err(TrySendError::Full(GameServiceCommand::SaveNow { .. }))
            | Err(TrySendError::Disconnected(GameServiceCommand::SaveNow { .. })) => {
                unreachable!("shutdown sends only Shutdown commands")
            }
        }
    }
}

#[derive(Debug)]
pub struct GameService {
    control: GameServiceControl,
    worker: Option<JoinHandle<Result<GameServiceExit>>>,
}

impl GameService {
    #[must_use]
    pub fn control(&self) -> GameServiceControl {
        self.control.clone()
    }

    pub fn shutdown(mut self) -> Result<GameServiceExit> {
        self.control.try_shutdown();
        self.join_worker()
    }

    fn join_worker(&mut self) -> Result<GameServiceExit> {
        let worker = self
            .worker
            .take()
            .context("game service worker was already joined")?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("game service worker panicked"))?
    }
}

impl Drop for GameService {
    fn drop(&mut self) {
        self.control.try_shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn spawn_game_service(
    runtime: SharedGameRuntime,
    config: GameServiceConfig,
) -> Result<GameService> {
    validate_config(&config)?;
    let (commands, receiver) = sync_channel(config.command_capacity.get());
    let control = GameServiceControl { commands };
    let worker = thread::Builder::new()
        .name("rom-game-service".to_owned())
        .spawn(move || run_game_service(runtime, config, receiver))
        .context("cannot spawn global game service")?;
    Ok(GameService {
        control,
        worker: Some(worker),
    })
}

pub fn load_game_state(path: &Path, dimension: &str) -> Result<GameState> {
    if !path.exists() {
        return GameState::new(dimension).map_err(Into::into);
    }
    if !path.is_file() {
        bail!("game snapshot {} is not a file", path.display());
    }
    let json = fs::read_to_string(path)
        .with_context(|| format!("cannot read game snapshot {}", path.display()))?;
    let snapshot = GameSnapshot::from_json(&json)
        .with_context(|| format!("cannot decode game snapshot {}", path.display()))?;
    let mut state = snapshot
        .restore()
        .with_context(|| format!("cannot restore game snapshot {}", path.display()))?;
    if state.dimension() != dimension {
        bail!(
            "game snapshot dimension {} does not match configured dimension {dimension}",
            state.dimension()
        );
    }
    state.detach_all_connections();
    Ok(state)
}

pub fn save_game_state(runtime: &SharedGameRuntime, path: &Path) -> Result<GameSaveReport> {
    let snapshot = runtime.snapshot()?;
    let json = snapshot.to_json_pretty()?;
    write_atomic(path, json.as_bytes())?;
    Ok(GameSaveReport {
        path: path.to_path_buf(),
        bytes: u64::try_from(json.len()).context("snapshot size exceeds u64")?,
        game_time: snapshot.time.game_time,
        players: snapshot.players.len(),
        entities: snapshot.entities.len(),
    })
}

fn validate_config(config: &GameServiceConfig) -> Result<()> {
    if config.tick_interval.is_zero() {
        bail!("game tick interval must be greater than zero");
    }
    if config.poll_interval.is_zero() {
        bail!("game service poll interval must be greater than zero");
    }
    if config
        .autosave_interval
        .is_some_and(|interval| interval.is_zero())
    {
        bail!("game autosave interval must be greater than zero");
    }
    if config.autosave_interval.is_some() && config.snapshot_path.is_none() {
        bail!("game autosave requires a snapshot path");
    }
    Ok(())
}

fn run_game_service(
    runtime: SharedGameRuntime,
    config: GameServiceConfig,
    commands: Receiver<GameServiceCommand>,
) -> Result<GameServiceExit> {
    let mut exit = GameServiceExit::default();
    let mut next_tick = Instant::now() + config.tick_interval;
    let mut next_autosave = config
        .autosave_interval
        .map(|interval| Instant::now() + interval);

    loop {
        let now = Instant::now();
        let mut catch_up = 0_u32;
        while now >= next_tick && catch_up < MAX_CATCH_UP_TICKS {
            runtime.tick()?;
            exit.ticks = exit.ticks.saturating_add(1);
            catch_up += 1;
            next_tick += config.tick_interval;
        }
        if now >= next_tick {
            let skipped = ((now - next_tick).as_nanos() / config.tick_interval.as_nanos()) + 1;
            let skipped = u64::try_from(skipped).unwrap_or(u64::MAX);
            exit.dropped_ticks = exit.dropped_ticks.saturating_add(skipped);
            next_tick = now + config.tick_interval;
        }

        if next_autosave.is_some_and(|deadline| now >= deadline) {
            if let Some(path) = &config.snapshot_path {
                save_game_state(&runtime, path)?;
                exit.autosaves = exit.autosaves.saturating_add(1);
            }
            next_autosave = config.autosave_interval.map(|interval| now + interval);
        }

        let wake_at = next_autosave.map_or(next_tick, |autosave| autosave.min(next_tick));
        let wait = wake_at
            .saturating_duration_since(Instant::now())
            .min(config.poll_interval);
        match commands.recv_timeout(wait) {
            Ok(GameServiceCommand::SaveNow { reply }) => {
                exit.requested_saves = exit.requested_saves.saturating_add(1);
                let result = match &config.snapshot_path {
                    Some(path) => {
                        save_game_state(&runtime, path).map_err(|error| format!("{error:#}"))
                    }
                    None => Err("game snapshot path is not configured".to_owned()),
                };
                let _ = reply.send(result);
            }
            Ok(GameServiceCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                if let Some(path) = &config.snapshot_path {
                    save_game_state(&runtime, path)?;
                    exit.requested_saves = exit.requested_saves.saturating_add(1);
                }
                return Ok(exit);
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("cannot create snapshot directory {}", parent.display()))?;
    let file_name = path
        .file_name()
        .context("game snapshot path has no file name")?
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
            .with_context(|| format!("cannot replace game snapshot {}", path.display()))?;
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
    use rom_game::{PlayerUuid, Transform};

    fn temporary_directory(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rom-game-service-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn global_service_ticks_once_independent_of_connection_count() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        runtime
            .connect_player(PlayerUuid::new(1), "Steve", spawn())
            .unwrap();
        runtime
            .connect_player(PlayerUuid::new(2), "Alex", spawn())
            .unwrap();
        let service = spawn_game_service(
            runtime.clone(),
            GameServiceConfig {
                tick_interval: Duration::from_millis(2),
                poll_interval: Duration::from_millis(1),
                ..GameServiceConfig::default()
            },
        )
        .unwrap();
        thread::sleep(Duration::from_millis(20));
        let exit = service.shutdown().unwrap();
        let game_time = runtime.with_state(|state| state.time().game_time).unwrap();
        assert_eq!(game_time, exit.ticks);
        assert!(game_time >= 2);
    }

    #[test]
    fn saves_and_restores_disconnected_player_state() {
        let directory = temporary_directory("restore");
        let path = directory.join("game-state.json");
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(3);
        runtime.connect_player(uuid, "Notch", spawn()).unwrap();
        runtime.tick().unwrap();
        save_game_state(&runtime, &path).unwrap();

        let restored = load_game_state(&path, "minecraft:overworld").unwrap();
        assert_eq!(restored.time().game_time, 1);
        assert!(!restored.player(uuid).unwrap().connected);
        assert_eq!(restored.player(uuid).unwrap().entity_id, None);
    }

    #[test]
    fn requested_save_writes_snapshot_immediately() {
        let directory = temporary_directory("save-now");
        let path = directory.join("state.json");
        let runtime = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_service(
            runtime,
            GameServiceConfig {
                snapshot_path: Some(path.clone()),
                ..GameServiceConfig::default()
            },
        )
        .unwrap();
        let report = service.control().save_now().unwrap();
        assert_eq!(report.path, path);
        assert!(report.bytes > 0);
        service.shutdown().unwrap();
    }

    #[test]
    fn rejects_snapshot_dimension_mismatch() {
        let directory = temporary_directory("dimension");
        let path = directory.join("state.json");
        let runtime = SharedGameRuntime::vanilla_overworld();
        save_game_state(&runtime, &path).unwrap();
        assert!(load_game_state(&path, "minecraft:the_nether").is_err());
    }
}
