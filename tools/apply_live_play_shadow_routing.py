from pathlib import Path

main_path = Path("crates/ferrum-server/src/main.rs")
play_path = Path("crates/ferrum-server/src/play_runtime.rs")
main = main_path.read_text()
play = play_path.read_text()

main = main.replace(
    "use ferrum_runtime::ConnectionId;\n",
    '''use ferrum_runtime::ConnectionId;
use ferrum_server::{
    authoritative_runtime::PlayInput,
    play_connection::{
        PlayReaderEndpoint, PlayWriterEndpoint, register_play_connection,
    },
    shared_runtime::{
        SharedPlayRuntime, SharedPlayRuntimeConfig, spawn_shared_play_runtime,
    },
};
''',
    1,
)
main = main.replace(
    "    net::{TcpListener, TcpStream},\n    path::{Path, PathBuf},\n",
    "    net::{TcpListener, TcpStream},\n    num::NonZeroUsize,\n    path::{Path, PathBuf},\n",
    1,
)
main = main.replace(
    "const MAX_IGNORED_PLAY_PACKETS: usize = 1_024;\n",
    "const MAX_IGNORED_PLAY_PACKETS: usize = 1_024;\nconst PLAY_OUTPUT_QUEUE_CAPACITY: usize = 256;\n",
    1,
)

old_state = '''struct ServerState {
    online_players: AtomicI32,
    next_connection_id: AtomicU64,
    world: play_runtime::SharedWorld,
    registry_payloads: Vec<Vec<u8>>,
}
'''
new_state = '''struct ServerState {
    online_players: AtomicI32,
    next_connection_id: AtomicU64,
    world: play_runtime::SharedWorld,
    registry_payloads: Vec<Vec<u8>>,
    shared_play_runtime: SharedPlayRuntime,
}
'''
if old_state not in main:
    raise SystemExit("ServerState marker not found")
main = main.replace(old_state, new_state, 1)

old_constructor = '''    ) -> Result<Self> {
        Ok(Self {
            online_players: AtomicI32::new(initial_online_players),
            next_connection_id: AtomicU64::new(1),
            world: {
                let center = play_runtime::spawn_chunk(&world);
                play_runtime::SharedWorld::new_with_policy(center, world, play_policy)?
            },
            registry_payloads,
        })
    }
'''
new_constructor = '''    ) -> Result<Self> {
        let center = play_runtime::spawn_chunk(&world);
        let shared_world =
            play_runtime::SharedWorld::new_with_policy(center, world, play_policy)?;
        let shared_play_runtime =
            spawn_shared_play_runtime(SharedPlayRuntimeConfig::default())?;
        Ok(Self {
            online_players: AtomicI32::new(initial_online_players),
            next_connection_id: AtomicU64::new(1),
            world: shared_world,
            registry_payloads,
            shared_play_runtime,
        })
    }
'''
if old_constructor not in main:
    raise SystemExit("ServerState constructor marker not found")
main = main.replace(old_constructor, new_constructor, 1)

old_enter = '''    fn enter_play(&self) -> OnlinePlayerGuard<'_> {
        self.online_players.fetch_add(1, Ordering::Relaxed);
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);
        OnlinePlayerGuard {
            state: self,
            connection_id: ConnectionId::new(id),
        }
    }
'''
new_enter = '''    fn try_enter_play(&self) -> Result<OnlinePlayerGuard<'_>> {
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);
        let connection_id = ConnectionId::new(id);
        let (play_reader, play_writer) = register_play_connection(
            &self.shared_play_runtime.connector(),
            connection_id,
            NonZeroUsize::new(PLAY_OUTPUT_QUEUE_CAPACITY)
                .expect("Play output queue capacity is non-zero"),
        )?;
        self.online_players.fetch_add(1, Ordering::Relaxed);
        Ok(OnlinePlayerGuard {
            state: self,
            connection_id,
            play_reader,
            _play_writer: play_writer,
        })
    }

    #[cfg(test)]
    fn enter_play(&self) -> OnlinePlayerGuard<'_> {
        self.try_enter_play()
            .expect("test Play connection must register")
    }
'''
if old_enter not in main:
    raise SystemExit("enter_play marker not found")
main = main.replace(old_enter, new_enter, 1)

old_guard = '''struct OnlinePlayerGuard<'a> {
    state: &'a ServerState,
    connection_id: ConnectionId,
}

impl OnlinePlayerGuard<'_> {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id
    }
}

impl Drop for OnlinePlayerGuard<'_> {
    fn drop(&mut self) {
        self.state.online_players.fetch_sub(1, Ordering::Relaxed);
    }
}
'''
new_guard = '''struct OnlinePlayerGuard<'a> {
    state: &'a ServerState,
    connection_id: ConnectionId,
    play_reader: PlayReaderEndpoint,
    _play_writer: PlayWriterEndpoint,
}

impl OnlinePlayerGuard<'_> {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id
    }

    fn play_reader(&self) -> &PlayReaderEndpoint {
        &self.play_reader
    }
}

impl Drop for OnlinePlayerGuard<'_> {
    fn drop(&mut self) {
        let _ = self.play_reader.try_disconnect();
        self.state.online_players.fetch_sub(1, Ordering::Relaxed);
    }
}
'''
if old_guard not in main:
    raise SystemExit("OnlinePlayerGuard marker not found")
main = main.replace(old_guard, new_guard, 1)

old_live_call = '''    let online_player = context.state.enter_play();
    let result = run_static_play_session(
        reader,
        writer,
        config,
        profile,
        session,
        PlayWorldContext {
            shared_world: context.state.world(),
            connection: online_player.connection_id(),
        },
        play_round_limit,
    );
'''
new_live_call = '''    let online_player = context.state.try_enter_play()?;
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
        Some(online_player.play_reader()),
        play_round_limit,
    );
'''
if old_live_call not in main:
    raise SystemExit("live Play call marker not found")
main = main.replace(old_live_call, new_live_call, 1)

static_signature = '''fn run_static_play_session<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &ServerConfig,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_round_limit: Option<usize>,
) -> Result<()> {
'''
static_replacement = '''fn run_static_play_session<R: Read, W: Write>(
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
        play_round_limit,
    )
}

fn run_static_play_session_with_bridge<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &ServerConfig,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_reader: Option<&PlayReaderEndpoint>,
    play_round_limit: Option<usize>,
) -> Result<()> {
'''
if static_signature not in main:
    raise SystemExit("run_static_play_session signature not found")
main = main.replace(static_signature, static_replacement, 1)

old_static_tail = '''    wait_for_play_bootstrap_acknowledgements(reader, profile, STATIC_TELEPORT_ID)?;
    run_keep_alive_loop(reader, writer, profile, session, world, play_round_limit)
}
'''
new_static_tail = '''    wait_for_play_bootstrap_acknowledgements_with_bridge(
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
        play_round_limit,
    )
}
'''
if old_static_tail not in main:
    raise SystemExit("run_static_play_session tail not found")
main = main.replace(old_static_tail, new_static_tail, 1)

bootstrap_signature = '''fn wait_for_play_bootstrap_acknowledgements<R: Read>(
    reader: &mut R,
    profile: &ProtocolProfile,
    expected_teleport_id: i32,
) -> Result<()> {
'''
bootstrap_replacement = '''fn wait_for_play_bootstrap_acknowledgements<R: Read>(
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
'''
if bootstrap_signature not in main:
    raise SystemExit("bootstrap acknowledgement signature not found")
main = main.replace(bootstrap_signature, bootstrap_replacement, 1)

old_chunk_ack = '''            if !packet_reader.take_remaining().is_empty() {
                bail!("chunk batch acknowledgement contains trailing bytes");
            }
            chunk_batch_acknowledged = true;
'''
new_chunk_ack = '''            if !packet_reader.take_remaining().is_empty() {
                bail!("chunk batch acknowledgement contains trailing bytes");
            }
            if let Some(play_reader) = play_reader {
                play_reader
                    .try_submit_input(PlayInput::ChunkBatchReceived(desired_chunks_per_tick))
                    .context("cannot route Play bootstrap chunk acknowledgement")?;
            }
            chunk_batch_acknowledged = true;
'''
if old_chunk_ack not in main:
    raise SystemExit("bootstrap chunk acknowledgement marker not found")
main = main.replace(old_chunk_ack, new_chunk_ack, 1)

keep_signature = '''fn run_keep_alive_loop<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_round_limit: Option<usize>,
) -> Result<()> {
    play_runtime::run_play_loop(
        reader,
        writer,
        profile,
        session,
        world.shared_world,
        world.connection,
        play_round_limit,
    )
}
'''
keep_replacement = '''fn run_keep_alive_loop<R: Read, W: Write>(
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
        play_round_limit,
    )
}
'''
if keep_signature not in main:
    raise SystemExit("run_keep_alive_loop marker not found")
main = main.replace(keep_signature, keep_replacement, 1)

play = play.replace(
    "use ferrum_server::{authoritative_runtime::PlayInput, play_input::decode_play_input};\n",
    '''use ferrum_server::{
    authoritative_runtime::PlayInput, play_connection::PlayReaderEndpoint,
    play_input::decode_play_input,
};
''',
    1,
)

play_signature = '''pub(super) fn run_play_loop<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    shared_world: &SharedWorld,
    connection: ConnectionId,
    play_round_limit: Option<usize>,
) -> Result<()> {
'''
play_replacement = '''pub(super) fn run_play_loop<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    shared_world: &SharedWorld,
    connection: ConnectionId,
    play_round_limit: Option<usize>,
) -> Result<()> {
    run_play_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        shared_world,
        connection,
        None,
        play_round_limit,
    )
}

pub(super) fn run_play_loop_with_bridge<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    shared_world: &SharedWorld,
    connection: ConnectionId,
    play_reader: Option<&PlayReaderEndpoint>,
    play_round_limit: Option<usize>,
) -> Result<()> {
'''
if play_signature not in play:
    raise SystemExit("run_play_loop signature not found")
play = play.replace(play_signature, play_replacement, 1)

old_decode = '''                    let input = decode_play_input(kind, packet_reader.take_remaining())?
                        .context("resolved migrated Play packet did not decode")?;
                    match input {
'''
new_decode = '''                    let input = decode_play_input(kind, packet_reader.take_remaining())?
                        .context("resolved migrated Play packet did not decode")?;
                    if let Some(play_reader) = play_reader {
                        play_reader
                            .try_submit_input(input.clone())
                            .context("cannot route decoded Play input")?;
                    }
                    match input {
'''
if old_decode not in play:
    raise SystemExit("decoded Play input marker not found")
play = play.replace(old_decode, new_decode, 1)

main_path.write_text(main)
play_path.write_text(play)
