from pathlib import Path

worker_path = Path("crates/ferrum-runtime/src/worker.rs")
lib_path = Path("crates/ferrum-runtime/src/lib.rs")
worker = worker_path.read_text()
lib = lib_path.read_text()

old_import = """use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
};
"""
new_import = """use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::mpsc::{
        Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
    },
    time::Duration,
};
"""
if old_import not in worker:
    raise SystemExit("import marker missing")
worker = worker.replace(old_import, new_import, 1)

receive = """    pub fn try_recv_output(&self) -> Result<O, WorkerReceiveError> {
        match self.output.try_recv() {
            Ok(output) => Ok(output),
            Err(TryRecvError::Empty) => Err(WorkerReceiveError::Empty),
            Err(TryRecvError::Disconnected) => Err(WorkerReceiveError::RuntimeDisconnected),
        }
    }
"""
wait = """
    pub fn recv_output(&self) -> Result<O, WorkerWaitError> {
        recv_output(&self.output)
    }

    pub fn recv_output_timeout(&self, timeout: Duration) -> Result<O, WorkerWaitError> {
        recv_output_timeout(&self.output, timeout)
    }
"""

first = receive + """}

/// Cloneable per-connection endpoint used by network reader workers.
"""
if first not in worker:
    raise SystemExit("combined marker missing")
worker = worker.replace(
    first,
    receive + wait + """}

/// Cloneable per-connection endpoint used by network reader workers.
""",
    1,
)

second = receive + """}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkerControlError {
"""
helper = """fn recv_output<O>(output: &Receiver<O>) -> Result<O, WorkerWaitError> {
    output.recv().map_err(|_| WorkerWaitError::RuntimeDisconnected)
}

fn recv_output_timeout<O>(output: &Receiver<O>, timeout: Duration) -> Result<O, WorkerWaitError> {
    match output.recv_timeout(timeout) {
        Ok(output) => Ok(output),
        Err(RecvTimeoutError::Timeout) => Err(WorkerWaitError::Timeout),
        Err(RecvTimeoutError::Disconnected) => Err(WorkerWaitError::RuntimeDisconnected),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkerWaitError {
    #[error("timed out waiting for connection output")]
    Timeout,
    #[error("authoritative worker runtime is disconnected")]
    RuntimeDisconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkerControlError {
"""
if second not in worker:
    raise SystemExit("split marker missing")
worker = worker.replace(second, receive + wait + "}\n\n" + helper, 1)

test_marker = """    #[test]
    fn broadcast_reports_full_queues_and_delivers_to_ready_workers() {
"""
tests = """    #[test]
    fn writer_endpoint_waits_for_queued_output() {
        let (connector, mut runtime) = worker_channel::<(), &'static str>(non_zero(4));
        let connection = ConnectionId::new(19);
        let worker = connector.try_connect(connection, non_zero(2)).unwrap();
        let (_input, output) = worker.split();
        let mut inputs = BoundedInputQueue::try_new(2).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        runtime.try_send_output(connection, "ready").unwrap();
        assert_eq!(output.recv_output().unwrap(), "ready");
    }

    #[test]
    fn writer_endpoint_reports_timeout_and_disconnect_separately() {
        let (connector, mut runtime) = worker_channel::<(), ()>(non_zero(4));
        let worker = connector
            .try_connect(ConnectionId::new(20), non_zero(1))
            .unwrap();
        let (_input, output) = worker.split();
        let mut inputs = BoundedInputQueue::try_new(2).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        assert_eq!(
            output.recv_output_timeout(Duration::from_millis(1)).unwrap_err(),
            WorkerWaitError::Timeout
        );
        drop(runtime);
        assert_eq!(
            output.recv_output().unwrap_err(),
            WorkerWaitError::RuntimeDisconnected
        );
    }

"""
if test_marker not in worker:
    raise SystemExit("test marker missing")
worker = worker.replace(test_marker, tests + test_marker, 1)
worker_path.write_text(worker)

old_export = """    WorkerControlError, WorkerIngressReport, WorkerInputError, WorkerOutputError,
    WorkerReceiveError, WorkerRuntime, worker_channel,
"""
new_export = """    WorkerControlError, WorkerIngressReport, WorkerInputError, WorkerOutputError,
    WorkerReceiveError, WorkerRuntime, WorkerWaitError, worker_channel,
"""
if old_export not in lib:
    raise SystemExit("export marker missing")
lib_path.write_text(lib.replace(old_export, new_export, 1))
