from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
source = path.read_text()

old_import = '''use ferrum_server::{
    authoritative_runtime::PlayInput,
    play_connection::{PlayReaderEndpoint, PlayWriterEndpoint, register_play_connection},
    shared_runtime::{SharedPlayRuntime, SharedPlayRuntimeConfig, spawn_shared_play_runtime},
};
'''
new_import = '''use ferrum_server::{
    authoritative_runtime::{PlayInput, PlayOutput},
    play_connection::{PlayReaderEndpoint, PlayWriterEndpoint, register_play_connection},
    play_writer::{PlayWriterDirective, PlayWriterWorker, spawn_play_writer},
    shared_runtime::{SharedPlayRuntime, SharedPlayRuntimeConfig, spawn_shared_play_runtime},
    shared_writer::SharedWriter,
};
'''
if old_import not in source:
    raise SystemExit("server import marker not found")
source = source.replace(old_import, new_import, 1)
source = source.replace(
    "const PLAY_OUTPUT_QUEUE_CAPACITY: usize = 256;\n",
    "const PLAY_OUTPUT_QUEUE_CAPACITY: usize = 256;\nconst PLAY_WRITER_WAIT_MILLIS: u64 = 50;\n",
    1,
)

old_creation = '''        Ok(OnlinePlayerGuard {
            state: self,
            connection_id,
            play_reader,
            _play_writer: play_writer,
        })
'''
new_creation = '''        Ok(OnlinePlayerGuard {
            state: self,
            connection_id,
            play_reader,
            play_writer: Some(play_writer),
        })
'''
if old_creation not in source:
    raise SystemExit("online player creation marker not found")
source = source.replace(old_creation, new_creation, 1)

old_guard = '''struct OnlinePlayerGuard<'a> {
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
'''
new_guard = '''struct OnlinePlayerGuard<'a> {
    state: &'a ServerState,
    connection_id: ConnectionId,
    play_reader: PlayReaderEndpoint,
    play_writer: Option<PlayWriterEndpoint>,
}

impl OnlinePlayerGuard<'_> {
    fn connection_id(&self) -> ConnectionId {
        self.connection_id
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
'''
if old_guard not in source:
    raise SystemExit("online player guard marker not found")
source = source.replace(old_guard, new_guard, 1)

old_client = '''fn handle_client(stream: &mut TcpStream, config: &ServerConfig, state: &ServerState) -> Result<()> {
    let mut reader = stream.try_clone().context("cannot clone TCP stream")?;
    handle_connection_protocol_with_play_round_limit(&mut reader, stream, config, state, None)
}
'''
new_client = '''fn handle_client(stream: &mut TcpStream, config: &ServerConfig, state: &ServerState) -> Result<()> {
    let mut reader = stream.try_clone().context("cannot clone TCP stream reader")?;
    let writer = SharedWriter::new(
        stream
            .try_clone()
            .context("cannot clone TCP stream writer")?,
    );
    handle_connection_protocol_with_play_round_limit(
        &mut reader,
        writer.clone(),
        config,
        state,
        None,
        Some(writer),
    )
}
'''
if old_client not in source:
    raise SystemExit("handle_client marker not found")
path.write_text(source.replace(old_client, new_client, 1))
