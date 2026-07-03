from pathlib import Path
p = Path('crates/ferrum-server/src/main.rs')
s = p.read_text()
marker = '    #[test]\n    fn generated_play_metadata_drives_join_and_spawn_payloads() {\n'
tests = '''    #[test]
    fn live_writer_frames_authoritative_packets() {
        let mut writer = Vec::new();
        let directive = write_live_play_output(
            &mut writer,
            0x44,
            PlayOutput::Packet(vec![0x03, 0xaa]),
        )
        .unwrap();
        assert_eq!(directive, PlayWriterDirective::Continue);
        assert_eq!(read_packet(&mut Cursor::new(writer)).unwrap(), vec![0x03, 0xaa]);
    }

    #[test]
    fn live_writer_encodes_disconnect_and_stops() {
        let mut writer = Vec::new();
        let directive = write_live_play_output(
            &mut writer,
            0x44,
            PlayOutput::Disconnect("bye".to_owned()),
        )
        .unwrap();
        assert_eq!(directive, PlayWriterDirective::Stop);
        let packet = read_packet(&mut Cursor::new(writer)).unwrap();
        let mut reader = PacketReader::new(&packet);
        assert_eq!(reader.read_varint().unwrap(), 0x44);
        assert!(!reader.take_remaining().is_empty());
    }

'''
if marker not in s:
    raise SystemExit('main test marker not found')
p.write_text(s.replace(marker, tests + marker, 1))
