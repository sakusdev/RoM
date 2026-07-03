from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
source = path.read_text()
marker = '''#[cfg(test)]
#[allow(dead_code)]
fn run_static_play_session<R: Read, W: Write>(
'''
helpers = '''fn spawn_live_play_writer(
    endpoint: PlayWriterEndpoint,
    writer: SharedWriter<TcpStream>,
    profile: &ProtocolProfile,
) -> Result<PlayWriterWorker<SharedWriter<TcpStream>>> {
    let disconnect_packet_id = profile.packets().require(PacketKind::PlayDisconnect)?;
    spawn_play_writer(
        endpoint,
        writer,
        Duration::from_millis(PLAY_WRITER_WAIT_MILLIS),
        move |writer, output| write_live_play_output(writer, disconnect_packet_id, output),
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
    disconnect_packet_id: i32,
    output: PlayOutput,
) -> Result<PlayWriterDirective> {
    let directive = match output {
        PlayOutput::Packet(packet) => {
            write_packet(writer, &packet)?;
            PlayWriterDirective::Continue
        }
        PlayOutput::Disconnect(reason) => {
            let payload = encode_play_disconnect(&reason)?;
            write_packet(
                writer,
                &build_packet(disconnect_packet_id, |body| {
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

'''
if marker not in source:
    raise SystemExit("writer helper insertion marker not found")
path.write_text(source.replace(marker, helpers + marker, 1))
