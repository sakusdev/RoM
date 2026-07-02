from pathlib import Path

worker_path = Path("crates/ferrum-runtime/src/worker.rs")
worker = worker_path.read_text()
old = '''/// Per-connection endpoint shared by a reader worker and consumed by its writer
/// worker. Input and disconnect commands are non-blocking; output reception is
/// also exposed as a non-blocking operation.
#[derive(Debug)]
pub struct ConnectionWorker<I, O> {
    connection: ConnectionId,
    commands: SyncSender<WorkerCommand<I, O>>,
    output: Receiver<O>,
}

impl<I, O> ConnectionWorker<I, O> {
    #[must_use]
    pub const fn connection_id(&self) -> ConnectionId {
        self.connection
    }

    pub fn try_send_input(&self, payload: I) -> Result<(), WorkerInputError<I>> {
        match self.commands.try_send(WorkerCommand::Input {
            connection: self.connection,
            payload,
        }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(WorkerCommand::Input { payload, .. })) => {
                Err(WorkerInputError::Full(payload))
            }
            Err(TrySendError::Disconnected(WorkerCommand::Input { payload, .. })) => {
                Err(WorkerInputError::RuntimeDisconnected(payload))
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                unreachable!("try_send_input only submits input commands")
            }
        }
    }

    pub fn try_disconnect(&self) -> Result<(), WorkerControlError> {
        match self.commands.try_send(WorkerCommand::Disconnect {
            connection: self.connection,
        }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(WorkerControlError::Full),
            Err(TrySendError::Disconnected(_)) => Err(WorkerControlError::RuntimeDisconnected),
        }
    }

    pub fn try_recv_output(&self) -> Result<O, WorkerReceiveError> {
        match self.output.try_recv() {
            Ok(output) => Ok(output),
            Err(TryRecvError::Empty) => Err(WorkerReceiveError::Empty),
            Err(TryRecvError::Disconnected) => Err(WorkerReceiveError::RuntimeDisconnected),
        }
    }
}
'''
new = '''/// Per-connection endpoint returned during registration.
///
/// It can be used directly for finite single-threaded integrations or split into
/// independently movable reader and writer endpoints for live TCP workers.
#[derive(Debug)]
pub struct ConnectionWorker<I, O> {
    connection: ConnectionId,
    commands: SyncSender<WorkerCommand<I, O>>,
    output: Receiver<O>,
}

impl<I, O> ConnectionWorker<I, O> {
    #[must_use]
    pub const fn connection_id(&self) -> ConnectionId {
        self.connection
    }

    #[must_use]
    pub fn split(self) -> (ConnectionInput<I, O>, ConnectionOutput<O>) {
        (
            ConnectionInput {
                connection: self.connection,
                commands: self.commands,
            },
            ConnectionOutput {
                connection: self.connection,
                output: self.output,
            },
        )
    }

    pub fn try_send_input(&self, payload: I) -> Result<(), WorkerInputError<I>> {
        try_send_input(&self.commands, self.connection, payload)
    }

    pub fn try_disconnect(&self) -> Result<(), WorkerControlError> {
        try_disconnect(&self.commands, self.connection)
    }

    pub fn try_recv_output(&self) -> Result<O, WorkerReceiveError> {
        try_recv_output(&self.output)
    }
}

/// Cloneable endpoint owned by a connection reader worker.
#[derive(Debug)]
pub struct ConnectionInput<I, O> {
    connection: ConnectionId,
    commands: SyncSender<WorkerCommand<I, O>>,
}

impl<I, O> Clone for ConnectionInput<I, O> {
    fn clone(&self) -> Self {
        Self {
            connection: self.connection,
            commands: self.commands.clone(),
        }
    }
}

impl<I, O> ConnectionInput<I, O> {
    #[must_use]
    pub const fn connection_id(&self) -> ConnectionId {
        self.connection
    }

    pub fn try_send_input(&self, payload: I) -> Result<(), WorkerInputError<I>> {
        try_send_input(&self.commands, self.connection, payload)
    }

    pub fn try_disconnect(&self) -> Result<(), WorkerControlError> {
        try_disconnect(&self.commands, self.connection)
    }
}

/// Output endpoint moved to one dedicated connection writer worker.
#[derive(Debug)]
pub struct ConnectionOutput<O> {
    connection: ConnectionId,
    output: Receiver<O>,
}

impl<O> ConnectionOutput<O> {
    #[must_use]
    pub const fn connection_id(&self) -> ConnectionId {
        self.connection
    }

    pub fn try_recv_output(&self) -> Result<O, WorkerReceiveError> {
        try_recv_output(&self.output)
    }
}

fn try_send_input<I, O>(
    commands: &SyncSender<WorkerCommand<I, O>>,
    connection: ConnectionId,
    payload: I,
) -> Result<(), WorkerInputError<I>> {
    match commands.try_send(WorkerCommand::Input {
        connection,
        payload,
    }) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(WorkerCommand::Input { payload, .. })) => {
            Err(WorkerInputError::Full(payload))
        }
        Err(TrySendError::Disconnected(WorkerCommand::Input { payload, .. })) => {
            Err(WorkerInputError::RuntimeDisconnected(payload))
        }
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            unreachable!("try_send_input only submits input commands")
        }
    }
}

fn try_disconnect<I, O>(
    commands: &SyncSender<WorkerCommand<I, O>>,
    connection: ConnectionId,
) -> Result<(), WorkerControlError> {
    match commands.try_send(WorkerCommand::Disconnect { connection }) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err(WorkerControlError::Full),
        Err(TrySendError::Disconnected(_)) => Err(WorkerControlError::RuntimeDisconnected),
    }
}

fn try_recv_output<O>(output: &Receiver<O>) -> Result<O, WorkerReceiveError> {
    match output.try_recv() {
        Ok(output) => Ok(output),
        Err(TryRecvError::Empty) => Err(WorkerReceiveError::Empty),
        Err(TryRecvError::Disconnected) => Err(WorkerReceiveError::RuntimeDisconnected),
    }
}
'''
if old not in worker:
    raise SystemExit("connection worker block did not match")
worker = worker.replace(old, new)
marker = '''    #[test]
    fn broadcast_reports_full_queues_and_delivers_to_ready_workers() {'''
test = '''    #[test]
    fn split_endpoints_support_independent_reader_and_writer_ownership() {
        let (connector, mut runtime) = worker_channel::<&'static str, &'static str>(non_zero(8));
        let connection = ConnectionId::new(11);
        let worker = connector.try_connect(connection, non_zero(2)).unwrap();
        let (reader, writer) = worker.split();
        assert_eq!(reader.connection_id(), connection);
        assert_eq!(writer.connection_id(), connection);

        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        reader.try_send_input("decoded packet").unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        assert_eq!(inputs.drain_tick(1)[0].payload, "decoded packet");

        runtime.try_send_output(connection, "encoded packet").unwrap();
        assert_eq!(writer.try_recv_output().unwrap(), "encoded packet");

        reader.try_disconnect().unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        assert!(!runtime.contains_connection(connection));
    }

'''
if marker not in worker:
    raise SystemExit("test insertion marker did not match")
worker = worker.replace(marker, test + marker)
worker_path.write_text(worker)

lib_path = Path("crates/ferrum-runtime/src/lib.rs")
lib = lib_path.read_text()
old_export = '''    ConnectionWorker, WorkerBroadcastReport, WorkerConnector, WorkerControlError,
    WorkerIngressReport, WorkerInputError, WorkerOutputError, WorkerReceiveError, WorkerRuntime,
    worker_channel,
'''
new_export = '''    ConnectionInput, ConnectionOutput, ConnectionWorker, WorkerBroadcastReport, WorkerConnector,
    WorkerControlError, WorkerIngressReport, WorkerInputError, WorkerOutputError,
    WorkerReceiveError, WorkerRuntime, worker_channel,
'''
if old_export not in lib:
    raise SystemExit("runtime export block did not match")
lib_path.write_text(lib.replace(old_export, new_export))
