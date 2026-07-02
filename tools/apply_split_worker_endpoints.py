from pathlib import Path

worker_path = Path("crates/ferrum-runtime/src/worker.rs")
lib_path = Path("crates/ferrum-runtime/src/lib.rs")

worker = worker_path.read_text()
if "pub struct ConnectionInput" in worker:
    raise SystemExit("split worker endpoints are already present")

connection_id_marker = '''    #[must_use]
    pub const fn connection_id(&self) -> ConnectionId {
        self.connection
    }

'''
split_method = connection_id_marker + '''    /// Split this registered connection into independently owned reader and
    /// writer endpoints. The input endpoint is cloneable for reader-side
    /// ownership, while the output receiver remains single-consumer.
    #[must_use]
    pub fn split(self) -> (ConnectionInput<I, O>, ConnectionOutput<O>) {
        let Self {
            connection,
            commands,
            output,
        } = self;
        (
            ConnectionInput {
                connection,
                commands,
            },
            ConnectionOutput { connection, output },
        )
    }

'''
if connection_id_marker not in worker:
    raise SystemExit("ConnectionWorker connection_id marker not found")
worker = worker.replace(connection_id_marker, split_method, 1)

error_marker = '''#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkerControlError {
'''
endpoint_types = '''/// Cloneable per-connection endpoint used by network reader workers.
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
}

/// Single-consumer per-connection endpoint owned by one network writer worker.
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
        match self.output.try_recv() {
            Ok(output) => Ok(output),
            Err(TryRecvError::Empty) => Err(WorkerReceiveError::Empty),
            Err(TryRecvError::Disconnected) => Err(WorkerReceiveError::RuntimeDisconnected),
        }
    }
}

'''
if error_marker not in worker:
    raise SystemExit("WorkerControlError marker not found")
worker = worker.replace(error_marker, endpoint_types + error_marker, 1)

test_marker = '''    #[test]
    fn broadcast_reports_full_queues_and_delivers_to_ready_workers() {
'''
split_test = '''    #[test]
    fn split_endpoints_support_independent_reader_and_writer_ownership() {
        let (connector, mut runtime) = worker_channel::<&'static str, &'static str>(non_zero(8));
        let worker = connector
            .try_connect(ConnectionId::new(11), non_zero(2))
            .unwrap();
        let (input, output) = worker.split();
        let cloned_input = input.clone();
        assert_eq!(input.connection_id(), ConnectionId::new(11));
        assert_eq!(output.connection_id(), ConnectionId::new(11));

        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        cloned_input.try_send_input("decoded packet").unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        assert_eq!(inputs.drain_tick(1)[0].payload, "decoded packet");

        runtime
            .try_send_output(ConnectionId::new(11), "encoded packet")
            .unwrap();
        assert_eq!(output.try_recv_output().unwrap(), "encoded packet");

        input.try_disconnect().unwrap();
        let report = runtime.ingest_available(&mut inputs, 1).unwrap();
        assert_eq!(report.disconnections, 1);
        assert!(!runtime.contains_connection(ConnectionId::new(11)));
    }

'''
if test_marker not in worker:
    raise SystemExit("broadcast test marker not found")
worker = worker.replace(test_marker, split_test + test_marker, 1)
worker_path.write_text(worker)

lib = lib_path.read_text()
export_marker = "    ConnectionWorker, WorkerBroadcastReport, WorkerConnector, WorkerControlError,\n"
export_replacement = (
    "    ConnectionInput, ConnectionOutput, ConnectionWorker, WorkerBroadcastReport, "
    "WorkerConnector,\n"
    "    WorkerControlError,\n"
)
if export_marker not in lib:
    raise SystemExit("worker export marker not found")
lib_path.write_text(lib.replace(export_marker, export_replacement, 1))
