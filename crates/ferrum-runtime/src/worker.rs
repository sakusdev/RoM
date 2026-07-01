use crate::{BoundedInputQueue, ConnectionId, QueueError};
use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
};
use thiserror::Error;

/// Build the bounded command boundary between connection workers and the
/// authoritative runtime owner.
///
/// The connector is cloned by network workers. The runtime half remains owned
/// by one authoritative thread and drains commands into a [`BoundedInputQueue`].
pub fn worker_channel<I, O>(
    command_capacity: NonZeroUsize,
) -> (WorkerConnector<I, O>, WorkerRuntime<I, O>) {
    let (commands, command_receiver) = sync_channel(command_capacity.get());
    (
        WorkerConnector { commands },
        WorkerRuntime {
            commands: command_receiver,
            outputs: BTreeMap::new(),
        },
    )
}

enum WorkerCommand<I, O> {
    Register {
        connection: ConnectionId,
        output: SyncSender<O>,
    },
    Input {
        connection: ConnectionId,
        payload: I,
    },
    Disconnect {
        connection: ConnectionId,
    },
}

/// Cloneable handle used by connection-accept and reader workers.
#[derive(Debug)]
pub struct WorkerConnector<I, O> {
    commands: SyncSender<WorkerCommand<I, O>>,
}

impl<I, O> Clone for WorkerConnector<I, O> {
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
        }
    }
}

impl<I, O> WorkerConnector<I, O> {
    /// Register one connection and create its independently bounded output
    /// receiver. This call never blocks.
    pub fn try_connect(
        &self,
        connection: ConnectionId,
        output_capacity: NonZeroUsize,
    ) -> Result<ConnectionWorker<I, O>, WorkerControlError> {
        let (output_sender, output) = sync_channel(output_capacity.get());
        match self.commands.try_send(WorkerCommand::Register {
            connection,
            output: output_sender,
        }) {
            Ok(()) => Ok(ConnectionWorker {
                connection,
                commands: self.commands.clone(),
                output,
            }),
            Err(TrySendError::Full(_)) => Err(WorkerControlError::Full),
            Err(TrySendError::Disconnected(_)) => Err(WorkerControlError::RuntimeDisconnected),
        }
    }
}

/// Per-connection endpoint shared by a reader worker and consumed by its writer
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkerControlError {
    #[error("worker command queue is full")]
    Full,
    #[error("authoritative worker runtime is disconnected")]
    RuntimeDisconnected,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum WorkerInputError<I> {
    #[error("worker command queue is full")]
    Full(I),
    #[error("authoritative worker runtime is disconnected")]
    RuntimeDisconnected(I),
}

impl<I> WorkerInputError<I> {
    pub fn into_payload(self) -> I {
        match self {
            Self::Full(payload) | Self::RuntimeDisconnected(payload) => payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkerReceiveError {
    #[error("connection output queue is empty")]
    Empty,
    #[error("authoritative worker runtime is disconnected")]
    RuntimeDisconnected,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum WorkerOutputError<O> {
    #[error("connection is not registered")]
    UnknownConnection {
        connection: ConnectionId,
        output: O,
    },
    #[error("connection output queue is full")]
    Full {
        connection: ConnectionId,
        output: O,
    },
    #[error("connection output worker is disconnected")]
    WorkerDisconnected {
        connection: ConnectionId,
        output: O,
    },
}

impl<O> WorkerOutputError<O> {
    #[must_use]
    pub const fn connection(&self) -> ConnectionId {
        match self {
            Self::UnknownConnection { connection, .. }
            | Self::Full { connection, .. }
            | Self::WorkerDisconnected { connection, .. } => *connection,
        }
    }

    pub fn into_output(self) -> O {
        match self {
            Self::UnknownConnection { output, .. }
            | Self::Full { output, .. }
            | Self::WorkerDisconnected { output, .. } => output,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerIngressReport {
    pub commands: usize,
    pub registrations: usize,
    pub replaced_connections: usize,
    pub accepted_inputs: usize,
    pub dropped_inputs: usize,
    pub orphaned_inputs: usize,
    pub disconnections: usize,
    pub removed_inputs: usize,
    pub command_channel_disconnected: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerBroadcastReport {
    pub delivered: usize,
    pub full: usize,
    pub disconnected: usize,
}

/// Authoritative half of the worker boundary.
///
/// It owns connection output senders and is intentionally not cloneable. A slow
/// output worker is isolated by its own bounded channel and cannot block output
/// delivery to another connection.
#[derive(Debug)]
pub struct WorkerRuntime<I, O> {
    commands: Receiver<WorkerCommand<I, O>>,
    outputs: BTreeMap<ConnectionId, SyncSender<O>>,
}

impl<I, O> WorkerRuntime<I, O> {
    /// Drain at most `max_commands` without blocking and route accepted inputs
    /// into the deterministic authoritative queue.
    ///
    /// Inputs received while that queue is full are counted as dropped. A
    /// sequence overflow remains a hard error. Commands from unregistered or
    /// already disconnected workers are counted as orphaned inputs.
    pub fn ingest_available(
        &mut self,
        inputs: &mut BoundedInputQueue<I>,
        max_commands: usize,
    ) -> Result<WorkerIngressReport, QueueError> {
        let mut report = WorkerIngressReport::default();
        for _ in 0..max_commands {
            let command = match self.commands.try_recv() {
                Ok(command) => command,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    report.command_channel_disconnected = true;
                    break;
                }
            };
            report.commands += 1;
            match command {
                WorkerCommand::Register { connection, output } => {
                    if self.outputs.insert(connection, output).is_some() {
                        report.replaced_connections += 1;
                        report.removed_inputs += inputs.remove_connection(connection);
                    }
                    report.registrations += 1;
                }
                WorkerCommand::Input {
                    connection,
                    payload,
                } => {
                    if !self.outputs.contains_key(&connection) {
                        report.orphaned_inputs += 1;
                        continue;
                    }
                    match inputs.push(connection, payload) {
                        Ok(_) => report.accepted_inputs += 1,
                        Err(QueueError::Full { .. }) => report.dropped_inputs += 1,
                        Err(error) => return Err(error),
                    }
                }
                WorkerCommand::Disconnect { connection } => {
                    if self.outputs.remove(&connection).is_some() {
                        report.disconnections += 1;
                    }
                    report.removed_inputs += inputs.remove_connection(connection);
                }
            }
        }
        Ok(report)
    }

    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.outputs.len()
    }

    #[must_use]
    pub fn contains_connection(&self, connection: ConnectionId) -> bool {
        self.outputs.contains_key(&connection)
    }

    /// Attempt one output delivery without blocking the authoritative runtime.
    pub fn try_send_output(
        &mut self,
        connection: ConnectionId,
        output: O,
    ) -> Result<(), WorkerOutputError<O>> {
        let Some(sender) = self.outputs.get(&connection).cloned() else {
            return Err(WorkerOutputError::UnknownConnection { connection, output });
        };
        match sender.try_send(output) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(output)) => {
                Err(WorkerOutputError::Full { connection, output })
            }
            Err(TrySendError::Disconnected(output)) => {
                self.outputs.remove(&connection);
                Err(WorkerOutputError::WorkerDisconnected { connection, output })
            }
        }
    }

    /// Broadcast without blocking. Full queues are counted and skipped, while
    /// disconnected output workers are removed after this pass.
    pub fn broadcast(&mut self, output: O) -> WorkerBroadcastReport
    where
        O: Clone,
    {
        let mut report = WorkerBroadcastReport::default();
        let connections = self.outputs.keys().copied().collect::<Vec<_>>();
        for connection in connections {
            let Some(sender) = self.outputs.get(&connection).cloned() else {
                continue;
            };
            match sender.try_send(output.clone()) {
                Ok(()) => report.delivered += 1,
                Err(TrySendError::Full(_)) => report.full += 1,
                Err(TrySendError::Disconnected(_)) => {
                    self.outputs.remove(&connection);
                    report.disconnected += 1;
                }
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn non_zero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn command_channel_is_bounded_and_non_blocking() {
        let (connector, mut runtime) = worker_channel::<&'static str, &'static str>(non_zero(1));
        let first = connector
            .try_connect(ConnectionId::new(1), non_zero(1))
            .unwrap();
        assert!(matches!(
            connector.try_connect(ConnectionId::new(2), non_zero(1)),
            Err(WorkerControlError::Full)
        ));

        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        let report = runtime.ingest_available(&mut inputs, 1).unwrap();
        assert_eq!(report.registrations, 1);
        assert_eq!(runtime.connection_count(), 1);
        assert_eq!(first.connection_id(), ConnectionId::new(1));

        assert!(
            connector
                .try_connect(ConnectionId::new(2), non_zero(1))
                .is_ok()
        );
    }

    #[test]
    fn inputs_reach_the_authoritative_queue_in_fair_connection_order() {
        let (connector, mut runtime) = worker_channel::<&'static str, ()>(non_zero(16));
        let second = connector
            .try_connect(ConnectionId::new(2), non_zero(1))
            .unwrap();
        let first = connector
            .try_connect(ConnectionId::new(1), non_zero(1))
            .unwrap();
        let mut inputs = BoundedInputQueue::try_new(16).unwrap();
        runtime.ingest_available(&mut inputs, 2).unwrap();

        second.try_send_input("2a").unwrap();
        second.try_send_input("2b").unwrap();
        first.try_send_input("1a").unwrap();
        first.try_send_input("1b").unwrap();
        let report = runtime.ingest_available(&mut inputs, 4).unwrap();
        assert_eq!(report.accepted_inputs, 4);

        let drained = inputs.drain_tick(4);
        let observed = drained
            .into_iter()
            .map(|event| {
                (
                    event.connection.get(),
                    event.sequence.get(),
                    event.payload,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed,
            [(1, 0, "1a"), (2, 0, "2a"), (1, 1, "1b"), (2, 1, "2b")]
        );
    }

    #[test]
    fn a_full_output_queue_does_not_block_another_connection() {
        let (connector, mut runtime) = worker_channel::<(), &'static str>(non_zero(8));
        let slow = connector
            .try_connect(ConnectionId::new(1), non_zero(1))
            .unwrap();
        let ready = connector
            .try_connect(ConnectionId::new(2), non_zero(1))
            .unwrap();
        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 2).unwrap();

        runtime
            .try_send_output(ConnectionId::new(1), "queued")
            .unwrap();
        assert_eq!(
            runtime
                .try_send_output(ConnectionId::new(1), "blocked")
                .unwrap_err(),
            WorkerOutputError::Full {
                connection: ConnectionId::new(1),
                output: "blocked",
            }
        );
        runtime
            .try_send_output(ConnectionId::new(2), "delivered")
            .unwrap();

        assert_eq!(ready.try_recv_output().unwrap(), "delivered");
        assert_eq!(slow.try_recv_output().unwrap(), "queued");
    }

    #[test]
    fn disconnect_removes_pending_input_and_output_registration() {
        let (connector, mut runtime) = worker_channel::<&'static str, ()>(non_zero(8));
        let worker = connector
            .try_connect(ConnectionId::new(7), non_zero(1))
            .unwrap();
        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();

        worker.try_send_input("pending").unwrap();
        worker.try_disconnect().unwrap();
        let report = runtime.ingest_available(&mut inputs, 2).unwrap();
        assert_eq!(report.accepted_inputs, 1);
        assert_eq!(report.disconnections, 1);
        assert_eq!(report.removed_inputs, 1);
        assert!(inputs.is_empty());
        assert!(!runtime.contains_connection(ConnectionId::new(7)));
    }

    #[test]
    fn broadcast_reports_full_queues_and_delivers_to_ready_workers() {
        let (connector, mut runtime) = worker_channel::<(), String>(non_zero(8));
        let slow = connector
            .try_connect(ConnectionId::new(1), non_zero(1))
            .unwrap();
        let ready = connector
            .try_connect(ConnectionId::new(2), non_zero(1))
            .unwrap();
        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 2).unwrap();
        runtime
            .try_send_output(ConnectionId::new(1), "already queued".to_owned())
            .unwrap();

        assert_eq!(
            runtime.broadcast("update".to_owned()),
            WorkerBroadcastReport {
                delivered: 1,
                full: 1,
                disconnected: 0,
            }
        );
        assert_eq!(ready.try_recv_output().unwrap(), "update");
        assert_eq!(slow.try_recv_output().unwrap(), "already queued");
    }
}
