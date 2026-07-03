from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
source = path.read_text()

old_client = '''fn handle_client(stream: &mut TcpStream, config: &ServerConfig, state: &ServerState) -> Result<()> {
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
new_client = '''fn handle_client(stream: &mut TcpStream, config: &ServerConfig, state: &ServerState) -> Result<()> {
    let mut reader = stream.try_clone().context("cannot clone TCP stream reader")?;
    let writer = SharedWriter::new(
        stream
            .try_clone()
            .context("cannot clone TCP stream writer")?,
    );
    let _live_writer = LivePlayWriterRegistration::install(writer.clone())?;
    handle_connection_protocol_with_play_round_limit(&mut reader, writer, config, state, None)
}
'''
if old_client not in source:
    raise SystemExit("generated handle_client marker not found")
source = source.replace(old_client, new_client, 1)

marker = '''fn handle_client(stream: &mut TcpStream, config: &ServerConfig, state: &ServerState) -> Result<()> {
'''
helpers = '''std::thread_local! {
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

'''
if marker not in source:
    raise SystemExit("handle_client insertion marker not found")
path.write_text(source.replace(marker, helpers + marker, 1))
