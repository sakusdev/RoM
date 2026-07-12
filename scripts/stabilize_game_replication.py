from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''        let player_uuid = GamePlayerUuid::from_bytes(*identity.uuid.as_bytes());
        self.game_runtime
            .connect_player(player_uuid, identity.username.clone(), transform)?;
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);''',
    '''        let player_uuid = GamePlayerUuid::from_bytes(*identity.uuid.as_bytes());
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);''',
    "defer player connected event",
)
text = replace_once(
    text,
    '''        let (play_reader, play_writer) = match endpoints {
            Ok(endpoints) => endpoints,
            Err(error) => {
                let _ = self.game_runtime.disconnect_player(player_uuid);
                return Err(error.into());
            }
        };''',
    '''        let (play_reader, play_writer) = endpoints?;''',
    "endpoint creation before game connect",
)
text = replace_once(
    text,
    '''        {
            let _ = play_reader.try_disconnect();
            let _ = self.game_runtime.disconnect_player(player_uuid);
            return Err(error.context("cannot register player for gameplay replication"));
        }
        self.online_players.fetch_add(1, Ordering::Relaxed);''',
    '''        {
            let _ = play_reader.try_disconnect();
            return Err(error.context("cannot register player for gameplay replication"));
        }
        if let Err(error) = self
            .game_runtime
            .connect_player(player_uuid, identity.username.clone(), transform)
        {
            let _ = self
                .game_replication
                .control()
                .unregister(player_uuid);
            let _ = play_reader.try_disconnect();
            return Err(error.into());
        }
        self.online_players.fetch_add(1, Ordering::Relaxed);''',
    "publish connect after replication registration",
)
path.write_text(text, encoding="utf-8")

path = Path("crates/ferrum-server/src/game_replication.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''    sync::mpsc::{
        Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
    },''',
    '''    sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel},''',
    "remove lossy shutdown import",
)
text = replace_once(
    text,
    '''    pub fn try_shutdown(&self) {
        match self.commands.try_send(ReplicationCommand::Shutdown) {
            Ok(())
            | Err(TrySendError::Full(ReplicationCommand::Shutdown))
            | Err(TrySendError::Disconnected(ReplicationCommand::Shutdown)) => {}
            Err(TrySendError::Full(ReplicationCommand::Register { .. }))
            | Err(TrySendError::Full(ReplicationCommand::Unregister { .. }))
            | Err(TrySendError::Disconnected(ReplicationCommand::Register { .. }))
            | Err(TrySendError::Disconnected(ReplicationCommand::Unregister { .. })) => {
                unreachable!("shutdown sends only Shutdown commands")
            }
        }
    }''',
    '''    pub fn request_shutdown(&self) {
        let _ = self.commands.send(ReplicationCommand::Shutdown);
    }''',
    "reliable shutdown command",
)
text = text.replace("self.control.try_shutdown();", "self.control.request_shutdown();")
text = replace_once(
    text,
    '''    fn ingest(
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) {
        workers.ingest_available(inputs, 64).unwrap();
    }''',
    '''    fn ingest(
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) {
        workers.ingest_available(inputs, 64).unwrap();
    }

    fn recv_output(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) -> PlayOutput {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            ingest(workers, inputs);
            match writer.try_recv_output() {
                Ok(output) => return output,
                Err(ferrum_runtime::WorkerReceiveError::Empty) => {
                    assert!(std::time::Instant::now() < deadline, "replication output timeout");
                    thread::sleep(Duration::from_millis(1));
                }
                Err(ferrum_runtime::WorkerReceiveError::RuntimeDisconnected) => {
                    panic!("replication runtime disconnected")
                }
            }
        }
    }''',
    "deterministic replication test receive",
)
old_first = '''        let steve = PlayerUuid::new(1);
        let alex = PlayerUuid::new(2);
        game.connect_player(steve, "Steve", spawn()).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(64).unwrap());'''
new_first = '''        let steve = PlayerUuid::new(1);
        let alex = PlayerUuid::new(2);

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(64).unwrap());'''
text = replace_once(text, old_first, new_first, "first test connection order prelude")
text = replace_once(
    text,
    '''        service.control().register(steve, steve_reader).unwrap();
        service.control().register(alex, alex_reader).unwrap();

        game.execute_command(&CommandSource::console(), "/say hello")''',
    '''        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().register(alex, alex_reader).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex joined the game"
        ));

        game.execute_command(&CommandSource::console(), "/say hello")''',
    "first test deterministic registration",
)
text = replace_once(
    text,
    '''        thread::sleep(Duration::from_millis(25));
        ingest(&mut workers, &mut inputs);

        assert!(matches!(
            steve_writer.try_recv_output().unwrap(),''',
    '''        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),''',
    "first test deterministic chat receive",
)
text = text.replace(
    "            steve_writer.try_recv_output().unwrap(),\n            PlayOutput::PlayerTeleport",
    "            recv_output(&steve_writer, &mut workers, &mut inputs),\n            PlayOutput::PlayerTeleport",
    1,
)
text = text.replace(
    "            alex_writer.try_recv_output().unwrap(),\n            PlayOutput::SystemChat",
    "            recv_output(&alex_writer, &mut workers, &mut inputs),\n            PlayOutput::SystemChat",
    1,
)
text = replace_once(
    text,
    '''        let steve = PlayerUuid::new(3);
        let alex = PlayerUuid::new(4);
        game.connect_player(steve, "Steve", spawn()).unwrap();

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());''',
    '''        let steve = PlayerUuid::new(3);
        let alex = PlayerUuid::new(4);

        let (connector, mut workers) = worker_channel(NonZeroUsize::new(32).unwrap());''',
    "second test connection order prelude",
)
text = replace_once(
    text,
    '''        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, steve_reader).unwrap();

        game.connect_player(alex, "Alex", spawn()).unwrap();
        thread::sleep(Duration::from_millis(20));
        ingest(&mut workers, &mut inputs);
        assert!(matches!(
            steve_writer.try_recv_output().unwrap(),''',
    '''        let (alex_reader, _alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(4),
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(32).unwrap();
        ingest(&mut workers, &mut inputs);
        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        service.control().register(alex, alex_reader).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),''',
    "second test deterministic registration",
)
text = replace_once(
    text,
    '''        game.disconnect_player(alex).unwrap();
        thread::sleep(Duration::from_millis(20));
        ingest(&mut workers, &mut inputs);
        assert!(matches!(
            steve_writer.try_recv_output().unwrap(),''',
    '''        service.control().unregister(alex).unwrap();
        game.disconnect_player(alex).unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),''',
    "second test deterministic disconnect",
)
path.write_text(text, encoding="utf-8")
