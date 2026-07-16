from pathlib import Path
import re


def load(path: str) -> str:
    return Path(path).read_text()


def save(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, repl: str, label: str) -> str:
    new, count = re.subn(pattern, repl, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return new

# Update replication tests and add regressions.
path = "crates/ferrum-server/src/game_replication.rs"
text = load(path)
# Activate every unit-test registration. Production activation remains after Play bootstrap.
text, count = re.subn(
    r"(?m)^(\s*)service\.control\(\)\.register\((\w+), (\w+)\)\.unwrap\(\);$",
    r"\1service.control().register(\2, \3).unwrap();\n\1service.control().activate(\2).unwrap();",
    text,
)
if count < 1:
    raise SystemExit("replication tests: no register calls updated")
# Add focused regression tests before the test module closes.
text = replace_once(
    text,
    '''        service.shutdown().unwrap();
    }
}''',
    '''        service.shutdown().unwrap();
    }

    #[test]
    fn registration_is_silent_until_explicit_activation() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, GameReplicationConfig::default()).unwrap();
        let steve = PlayerUuid::new(601);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(601),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        thread::sleep(Duration::from_millis(10));
        ingest(&mut workers, &mut inputs);
        assert!(writer.try_recv_output().is_err());
        service.control().activate(steve).unwrap();
        assert!(matches!(
            recv_raw_output(&writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        service.shutdown().unwrap();
    }

    #[test]
    fn repeated_spawn_snapshot_is_idempotent() {
        let game = SharedGameRuntime::vanilla_overworld();
        let steve = PlayerUuid::new(602);
        game.connect_player(steve, "Steve", spawn()).unwrap();
        let snapshot = player_snapshot(&game, steve).unwrap().unwrap();
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());
        let (reader, _writer) = register_play_connection(
            &connector,
            ConnectionId::new(602),
            NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        let mut connection = ReplicationConnection::new(reader, 16);
        connection.activate().unwrap();
        let mut exit = GameReplicationExit::default();
        queue_player_spawn(&mut connection, snapshot.clone(), &entity_config(), &mut exit).unwrap();
        let pending = connection.pending.len();
        queue_player_spawn(&mut connection, snapshot, &entity_config(), &mut exit).unwrap();
        assert_eq!(connection.pending.len(), pending);
        assert_eq!(connection.entities.len(), 1);
    }

    #[test]
    fn output_overflow_disconnects_instead_of_corrupting_tracking_state() {
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(8).unwrap());
        let (reader, _writer) = register_play_connection(
            &connector,
            ConnectionId::new(603),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(8).unwrap();
        ingest(&mut workers, &mut inputs);
        let mut connection = ReplicationConnection::new(reader, 1);
        connection.activate().unwrap();
        let mut exit = GameReplicationExit::default();
        assert!(connection.queue(PlayOutput::SystemChat {
            message: "first".to_owned(),
            overlay: false,
        }, &mut exit));
        assert!(!connection.queue(PlayOutput::SystemChat {
            message: "second".to_owned(),
            overlay: false,
        }, &mut exit));
        assert!(!connection.healthy);
        assert!(connection.pending.is_empty());
        assert!(connection.entities.is_empty());
        assert_eq!(exit.dropped_outputs, 1);
    }
}''',
    "replication regression tests",
)
save(path, text)
