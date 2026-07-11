from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use ferrum_game::{GameState, PlayerUuid as GamePlayerUuid, Transform};",
    "use ferrum_game::{CommandSource, GameState, PlayerUuid as GamePlayerUuid, Transform};",
    "game command import",
)
text = replace_once(
    text,
    "    game_runtime::SharedGameRuntime,\n",
    '''    game_runtime::SharedGameRuntime,
    game_service::{
        GameService, GameServiceConfig, GameServiceControl, GameServiceExit, load_game_state,
        spawn_game_service,
    },
''',
    "game service imports",
)
text = replace_once(
    text,
    "    io::{self, Read, Write},",
    "    io::{self, BufRead, ErrorKind, Read, Write},",
    "console IO imports",
)
text = replace_once(
    text,
    "        Arc,\n        atomic::{AtomicI32, AtomicU64, Ordering},",
    "        Arc,\n        atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering},",
    "shutdown atomic import",
)
text = replace_once(
    text,
    "const PLAY_WRITER_WAIT_MILLIS: u64 = 50;",
    "const PLAY_WRITER_WAIT_MILLIS: u64 = 50;\nconst MAX_AUTOSAVE_SECONDS: u64 = 24 * 60 * 60;\nconst ACCEPT_POLL_MILLIS: u64 = 10;",
    "service constants",
)
text = replace_once(
    text,
    '''    /// Locally generated and integrity-verified RoM version pack.
    #[arg(long, value_name = "PATH")]
    version_pack: Option<PathBuf>,
}''',
    '''    /// Locally generated and integrity-verified RoM version pack.
    #[arg(long, value_name = "PATH")]
    version_pack: Option<PathBuf>,

    /// Persistent gameplay snapshot. Defaults to game-state.json beside server.toml.
    #[arg(long, value_name = "PATH")]
    game_state: Option<PathBuf>,

    /// Autosave interval in seconds. Zero disables periodic saves; shutdown still saves.
    #[arg(long, default_value_t = 30)]
    autosave_seconds: u64,

    /// Disable the interactive server console on standard input.
    #[arg(long)]
    no_console: bool,
}''',
    "CLI persistence options",
)
text = replace_once(
    text,
    "    game_runtime: SharedGameRuntime,\n}",
    "    game_runtime: SharedGameRuntime,\n    game_service: GameService,\n    shutdown: Arc<AtomicBool>,\n}",
    "server service fields",
)
text = replace_once(
    text,
    '''            config.play_policy.clone(),
            None,
        )''',
    '''            config.play_policy.clone(),
            None,
            None,
            GameServiceConfig::default(),
        )''',
    "test state service defaults",
)
text = replace_once(
    text,
    '''        play_policy: PlayPolicy,
        loaded_chunks: Option<ChunkStore>,
    ) -> Result<Self> {
        let center = play_runtime::spawn_chunk(&world);
        let game_runtime = SharedGameRuntime::new(GameState::new(world.dimension.clone())?);''',
    '''        play_policy: PlayPolicy,
        loaded_chunks: Option<ChunkStore>,
        game_state: Option<GameState>,
        game_service_config: GameServiceConfig,
    ) -> Result<Self> {
        let center = play_runtime::spawn_chunk(&world);
        let game_state = match game_state {
            Some(state) => {
                if state.dimension() != world.dimension {
                    bail!(
                        "game state dimension {} does not match world dimension {}",
                        state.dimension(),
                        world.dimension
                    );
                }
                state
            }
            None => GameState::new(world.dimension.clone())?,
        };
        let game_runtime = SharedGameRuntime::new(game_state);
        let game_service = spawn_game_service(game_runtime.clone(), game_service_config)?;''',
    "runtime service construction",
)
text = replace_once(
    text,
    '''            shared_play_runtime,
            game_runtime,
        })''',
    '''            shared_play_runtime,
            game_runtime,
            game_service,
            shutdown: Arc::new(AtomicBool::new(false)),
        })''',
    "runtime service fields",
)
text = replace_once(
    text,
    '''            play_policy,
            Some(store),
        )''',
    '''            play_policy,
            Some(store),
            None,
            GameServiceConfig::default(),
        )''',
    "loaded world service defaults",
)
text = replace_once(
    text,
    '''    fn registry_payloads(&self) -> &[Vec<u8>] {
        &self.registry_payloads
    }
}''',
    '''    fn registry_payloads(&self) -> &[Vec<u8>] {
        &self.registry_payloads
    }

    fn game_control(&self) -> GameServiceControl {
        self.game_service.control()
    }

    fn shutdown_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown)
    }

    fn shutdown_requested(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    fn shutdown(self) -> Result<GameServiceExit> {
        self.game_service.shutdown()
    }
}''',
    "server shutdown methods",
)

old_run_setup = '''    let mut config = ServerConfig::from_file(&config_path)
        .with_context(|| format!("cannot load {}", config_path.display()))?;
    let (runtime_profile, world_profile, registry_payloads) ='''
new_run_setup = '''    let mut config = ServerConfig::from_file(&config_path)
        .with_context(|| format!("cannot load {}", config_path.display()))?;
    if cli.autosave_seconds > MAX_AUTOSAVE_SECONDS {
        bail!(
            "autosave interval {} exceeds maximum {MAX_AUTOSAVE_SECONDS}",
            cli.autosave_seconds
        );
    }
    let default_state_path = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("game-state.json");
    let game_state_path = cli.game_state.clone().unwrap_or(default_state_path);
    let (runtime_profile, world_profile, registry_payloads) ='''
text = replace_once(text, old_run_setup, new_run_setup, "run persistence setup")
text = replace_once(
    text,
    '''    config.runtime_profile = Some(runtime_profile);
    let loaded_chunks = load_configured_world_chunks(&config.world, &world_profile)?;
    let state = Arc::new(ServerState::with_runtime(
        config.online_players,
        world_profile,
        registry_payloads,
        config.play_policy.clone(),
        loaded_chunks,
    )?);''',
    '''    config.runtime_profile = Some(runtime_profile);
    let game_state = load_game_state(&game_state_path, &world_profile.dimension)?;
    let loaded_chunks = load_configured_world_chunks(&config.world, &world_profile)?;
    let game_service_config = GameServiceConfig {
        snapshot_path: Some(game_state_path.clone()),
        autosave_interval: (cli.autosave_seconds > 0)
            .then(|| Duration::from_secs(cli.autosave_seconds)),
        ..GameServiceConfig::default()
    };
    let state = Arc::new(ServerState::with_runtime(
        config.online_players,
        world_profile,
        registry_payloads,
        config.play_policy.clone(),
        loaded_chunks,
        Some(game_state),
        game_service_config,
    )?);''',
    "load persistent game state",
)
text = replace_once(
    text,
    '''    let listener = TcpListener::bind(&config.bind)
        .with_context(|| format!("cannot bind Minecraft status listener on {}", config.bind))?;
    println!(''',
    '''    let listener = TcpListener::bind(&config.bind)
        .with_context(|| format!("cannot bind Minecraft status listener on {}", config.bind))?;
    listener
        .set_nonblocking(true)
        .context("cannot configure non-blocking Minecraft listener")?;
    if !cli.no_console {
        spawn_server_console(
            state.game_runtime.clone(),
            state.game_control(),
            state.shutdown_signal(),
        )?;
    }
    println!(''',
    "nonblocking listener and console",
)
old_loop = '''    for incoming in listener.incoming() {
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
    Ok(())'''
new_loop = '''    let mut clients = Vec::new();
    while !state.shutdown_requested() {
        match listener.accept() {
            Ok((mut stream, _address)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                let config = config.clone();
                let state = Arc::clone(&state);
                clients.push(thread::spawn(move || {
                    if let Err(error) = handle_client(&mut stream, &config, &state) {
                        eprintln!("connection closed: {error:#}");
                    }
                }));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                reap_finished_clients(&mut clients);
                thread::sleep(Duration::from_millis(ACCEPT_POLL_MILLIS));
            }
            Err(error) => eprintln!("incoming connection failed: {error}"),
        }
    }
    drop(listener);
    for client in clients {
        let _ = client.join();
    }
    let state = Arc::try_unwrap(state)
        .map_err(|_| anyhow::anyhow!("server state is still referenced during shutdown"))?;
    let exit = state.shutdown()?;
    println!(
        "game service stopped after {} ticks ({} dropped), {} autosaves, {} requested/final saves",
        exit.ticks, exit.dropped_ticks, exit.autosaves, exit.requested_saves
    );
    Ok(())'''
text = replace_once(text, old_loop, new_loop, "server accept loop")

insert_before = "struct LoadedVersionPack {"
helpers = '''fn reap_finished_clients(clients: &mut Vec<thread::JoinHandle<()>>) {
    let mut index = 0;
    while index < clients.len() {
        if clients[index].is_finished() {
            let client = clients.swap_remove(index);
            let _ = client.join();
        } else {
            index += 1;
        }
    }
}

fn spawn_server_console(
    runtime: SharedGameRuntime,
    game_control: GameServiceControl,
    shutdown: Arc<AtomicBool>,
) -> Result<thread::JoinHandle<()>> {
    thread::Builder::new()
        .name("rom-server-console".to_owned())
        .spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = match line {
                    Ok(line) => line,
                    Err(error) => {
                        eprintln!("server console input failed: {error}");
                        break;
                    }
                };
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match runtime.execute_command(&CommandSource::console(), line) {
                    Ok(outcome) => {
                        println!("{}", outcome.feedback);
                        if outcome.save_requested {
                            match game_control.save_now() {
                                Ok(report) => println!(
                                    "saved gameplay state to {} ({} bytes, tick {}, {} players, {} entities)",
                                    report.path.display(),
                                    report.bytes,
                                    report.game_time,
                                    report.players,
                                    report.entities
                                ),
                                Err(error) => eprintln!("game save failed: {error:#}"),
                            }
                        }
                        if outcome.shutdown_requested {
                            shutdown.store(true, Ordering::Release);
                            break;
                        }
                    }
                    Err(error) => eprintln!("command failed: {error}"),
                }
            }
        })
        .context("cannot spawn server console")
}

'''
text = replace_once(text, insert_before, helpers + insert_before, "server process helpers")
path.write_text(text, encoding="utf-8")
