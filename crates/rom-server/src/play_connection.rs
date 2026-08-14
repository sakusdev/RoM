use crate::{
    authoritative_runtime::{PlayInput, PlayOutput},
    play_input::decode_play_input,
};
use rom_protocol::PacketKind;
use rom_runtime::{
    ConnectionId, ConnectionInput, ConnectionOutput, WorkerConnector, WorkerControlError,
    WorkerInputError,
};
use std::{error::Error, fmt, num::NonZeroUsize};

pub type PlayWorkerConnector = WorkerConnector<PlayInput, PlayOutput>;
pub type PlayWriterEndpoint = ConnectionOutput<PlayOutput>;

/// Register a Play connection with the shared worker hub and split ownership
/// between a cloneable reader endpoint and one single-consumer writer endpoint.
pub fn register_play_connection(
    connector: &PlayWorkerConnector,
    connection: ConnectionId,
    output_capacity: NonZeroUsize,
) -> Result<(PlayReaderEndpoint, PlayWriterEndpoint), WorkerControlError> {
    let worker = connector.try_connect(connection, output_capacity)?;
    let (input, output) = worker.split();
    Ok((PlayReaderEndpoint { input }, output))
}

#[derive(Debug, Clone)]
pub struct PlayReaderEndpoint {
    input: ConnectionInput<PlayInput, PlayOutput>,
}

impl PlayReaderEndpoint {
    #[must_use]
    pub const fn connection_id(&self) -> ConnectionId {
        self.input.connection_id()
    }

    /// Decode and submit one semantic serverbound Play packet without blocking.
    /// Unsupported packet kinds remain owned by the existing connection loop.
    pub fn try_submit_packet(
        &self,
        kind: PacketKind,
        payload: &[u8],
    ) -> Result<PlayPacketSubmission, PlayPacketSubmitError> {
        let Some(input) =
            decode_play_input(kind, payload).map_err(PlayPacketSubmitError::Decode)?
        else {
            return Ok(PlayPacketSubmission::Unsupported);
        };
        self.try_submit_input(input)?;
        Ok(PlayPacketSubmission::Submitted)
    }

    pub fn try_submit_input(&self, input: PlayInput) -> Result<(), PlayPacketSubmitError> {
        match self.input.try_send_input(input) {
            Ok(()) => Ok(()),
            Err(WorkerInputError::Full(input)) => Err(PlayPacketSubmitError::Full(input)),
            Err(WorkerInputError::RuntimeDisconnected(input)) => {
                Err(PlayPacketSubmitError::RuntimeDisconnected(input))
            }
        }
    }

    pub fn try_submit_output(&self, output: PlayOutput) -> Result<(), PlayOutputSubmitError> {
        match self.input.try_send_output(output) {
            Ok(()) => Ok(()),
            Err(WorkerInputError::Full(output)) => Err(PlayOutputSubmitError::Full(output)),
            Err(WorkerInputError::RuntimeDisconnected(output)) => {
                Err(PlayOutputSubmitError::RuntimeDisconnected(output))
            }
        }
    }

    pub fn try_disconnect(&self) -> Result<(), WorkerControlError> {
        self.input.try_disconnect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayPacketSubmission {
    Submitted,
    Unsupported,
}

#[derive(Debug)]
pub enum PlayPacketSubmitError {
    Decode(anyhow::Error),
    Full(PlayInput),
    RuntimeDisconnected(PlayInput),
}

impl PlayPacketSubmitError {
    pub fn into_input(self) -> Option<PlayInput> {
        match self {
            Self::Decode(_) => None,
            Self::Full(input) | Self::RuntimeDisconnected(input) => Some(input),
        }
    }
}

impl fmt::Display for PlayPacketSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(_) => formatter.write_str("cannot decode semantic Play input"),
            Self::Full(_) => formatter.write_str("worker command queue is full"),
            Self::RuntimeDisconnected(_) => {
                formatter.write_str("authoritative Play runtime is disconnected")
            }
        }
    }
}

impl Error for PlayPacketSubmitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error.as_ref()),
            Self::Full(_) | Self::RuntimeDisconnected(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum PlayOutputSubmitError {
    Full(PlayOutput),
    RuntimeDisconnected(PlayOutput),
}

impl PlayOutputSubmitError {
    pub fn into_output(self) -> PlayOutput {
        match self {
            Self::Full(output) | Self::RuntimeDisconnected(output) => output,
        }
    }
}

impl fmt::Display for PlayOutputSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full(_) => formatter.write_str("worker command queue is full"),
            Self::RuntimeDisconnected(_) => {
                formatter.write_str("authoritative Play runtime is disconnected")
            }
        }
    }
}

impl Error for PlayOutputSubmitError {}

#[cfg(test)]
mod tests {
    use super::*;
    use rom_runtime::{BoundedInputQueue, worker_channel};

    fn non_zero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn submits_decoded_packets_to_the_authoritative_input_queue() {
        let (connector, mut runtime) = worker_channel(non_zero(8));
        let connection = ConnectionId::new(17);
        let (reader, writer) =
            register_play_connection(&connector, connection, non_zero(4)).unwrap();
        assert_eq!(reader.connection_id(), connection);
        assert_eq!(writer.connection_id(), connection);

        let mut inputs = BoundedInputQueue::try_new(8).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        assert_eq!(
            reader
                .try_submit_packet(PacketKind::ClientTickEnd, &[])
                .unwrap(),
            PlayPacketSubmission::Submitted
        );
        runtime.ingest_available(&mut inputs, 1).unwrap();

        let drained = inputs.drain_tick(1);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].connection, connection);
        assert_eq!(drained[0].payload, PlayInput::ClientTickEnd);
    }

    #[test]
    fn unsupported_packets_stay_at_the_connection_boundary() {
        let (connector, mut runtime) = worker_channel(non_zero(4));
        let (reader, _writer) =
            register_play_connection(&connector, ConnectionId::new(2), non_zero(2)).unwrap();
        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();

        assert_eq!(
            reader
                .try_submit_packet(PacketKind::PlayerAction, &[1, 2, 3])
                .unwrap(),
            PlayPacketSubmission::Unsupported
        );
        assert_eq!(
            runtime.ingest_available(&mut inputs, 1).unwrap().commands,
            0
        );
        assert!(inputs.is_empty());
    }

    #[test]
    fn command_backpressure_returns_the_decoded_input() {
        let (connector, _runtime) = worker_channel(non_zero(1));
        let (reader, _writer) =
            register_play_connection(&connector, ConnectionId::new(4), non_zero(2)).unwrap();

        let error = reader
            .try_submit_packet(PacketKind::ClientTickEnd, &[])
            .unwrap_err();
        assert_eq!(error.into_input(), Some(PlayInput::ClientTickEnd));
    }

    #[test]
    fn malformed_payloads_are_reported_before_queue_submission() {
        let (connector, _runtime) = worker_channel(non_zero(4));
        let (reader, _writer) =
            register_play_connection(&connector, ConnectionId::new(5), non_zero(2)).unwrap();

        let error = reader
            .try_submit_packet(PacketKind::KeepAliveResponse, &[0])
            .unwrap_err();
        assert!(matches!(error, PlayPacketSubmitError::Decode(_)));
    }

    #[test]
    fn disconnect_command_removes_the_worker_registration() {
        let (connector, mut runtime) = worker_channel(non_zero(4));
        let connection = ConnectionId::new(8);
        let (reader, _writer) =
            register_play_connection(&connector, connection, non_zero(2)).unwrap();
        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        assert!(runtime.contains_connection(connection));

        reader.try_disconnect().unwrap();
        let report = runtime.ingest_available(&mut inputs, 1).unwrap();
        assert_eq!(report.disconnections, 1);
        assert!(!runtime.contains_connection(connection));
    }

    #[test]
    fn output_commands_reach_the_writer_endpoint() {
        let (connector, mut runtime) = worker_channel(non_zero(4));
        let (reader, writer) =
            register_play_connection(&connector, ConnectionId::new(9), non_zero(2)).unwrap();
        let mut inputs = BoundedInputQueue::try_new(4).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();

        reader
            .try_submit_output(PlayOutput::Packet(vec![1, 2, 3]))
            .unwrap();
        let report = runtime.ingest_available(&mut inputs, 1).unwrap();

        assert_eq!(report.accepted_outputs, 1);
        assert_eq!(
            writer.try_recv_output().unwrap(),
            PlayOutput::Packet(vec![1, 2, 3])
        );
    }
}
