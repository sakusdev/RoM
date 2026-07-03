from pathlib import Path

path = Path('crates/ferrum-server/src/main.rs')
source = path.read_text()

spawn_marker = '''fn spawn_live_play_writer(
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
'''
spawn_replacement = '''#[derive(Debug, Clone, Copy)]
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
'''
if spawn_marker not in source:
    raise SystemExit('spawn_live_play_writer marker not found')
source = source.replace(spawn_marker, spawn_replacement, 1)

old_writer = '''fn write_live_play_output<W: Write>(
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
'''
new_writer = '''fn write_live_play_output<W: Write>(
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
'''
if old_writer not in source:
    raise SystemExit('write_live_play_output marker not found')
source = source.replace(old_writer, new_writer, 1)

old_packet_test = '''        let directive =
            write_live_play_output(&mut writer, 0x44, PlayOutput::Packet(vec![0x03, 0xaa]))
                .unwrap();
'''
new_packet_test = '''        let directive = write_live_play_output(
            &mut writer,
            PlayOutputPacketIds {
                keep_alive_request: 0x43,
                disconnect: 0x44,
            },
            PlayOutput::Packet(vec![0x03, 0xaa]),
        )
        .unwrap();
'''
if old_packet_test not in source:
    raise SystemExit('packet output test marker not found')
source = source.replace(old_packet_test, new_packet_test, 1)

old_disconnect_test = '''        let directive =
            write_live_play_output(&mut writer, 0x44, PlayOutput::Disconnect("bye".to_owned()))
                .unwrap();
'''
new_disconnect_test = '''        let directive = write_live_play_output(
            &mut writer,
            PlayOutputPacketIds {
                keep_alive_request: 0x43,
                disconnect: 0x44,
            },
            PlayOutput::Disconnect("bye".to_owned()),
        )
        .unwrap();
'''
if old_disconnect_test not in source:
    raise SystemExit('disconnect output test marker not found')
source = source.replace(old_disconnect_test, new_disconnect_test, 1)

test_marker = '''    #[test]
    fn live_writer_encodes_disconnect_and_stops() {
'''
keep_alive_test = '''    #[test]
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

'''
if test_marker not in source:
    raise SystemExit('keep alive test insertion marker not found')
path.write_text(source.replace(test_marker, keep_alive_test + test_marker, 1))
