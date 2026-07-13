use anyhow::{Context, Result};
use ferrum_game::{ItemStack, Transform};
use ferrum_play::PlayerMovement;
use ferrum_runtime::{
    BoundedInputQueue, ConnectionId, FixedRateClock, Tick, WorkerIngressReport, WorkerOutputError,
    WorkerRuntime,
};
use std::{
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroUsize},
    time::Instant,
};

#[derive(Debug, Clone, PartialEq)]
pub enum PlayInput {
    ClientTickEnd,
    KeepAliveResponse(i64),
    Movement(PlayerMovement),
    ChunkBatchReceived(f32),
    Disconnected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayOutput {
    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
    /// Request a protocol-aware Keep Alive packet with this identifier.
    KeepAliveRequest(i64),
    /// Request a protocol-aware Play disconnect with this reason.
    Disconnect(String),
    /// Send a protocol-aware system chat component.
    SystemChat { message: String, overlay: bool },
    /// Teleport this connection using a connection-local teleport identifier.
    PlayerTeleport {
        teleport_id: i32,
        transform: Transform,
    },
    /// Synchronize one authoritative player inventory slot.
    SetPlayerInventory {
        slot: usize,
        stack: Option<ItemStack>,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnectionRuntimeState {
    pub client_ticks: u64,
    pub last_keep_alive_response: Option<i64>,
    pub last_keep_alive_request: Option<i64>,
    pub keep_alive_pending: bool,
    pub last_movement: Option<PlayerMovement>,
    pub desired_chunks_per_tick: Option<f32>,
    ticks_since_keep_alive_request: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuthoritativePollReport {
    pub ingress: WorkerIngressReport,
    pub removed_connections: usize,
    pub executed_ticks: u32,
    pub dropped_ticks: u32,
    pub processed_inputs: usize,
    pub sent_outputs: usize,
    pub full_output_queues: usize,
    pub disconnected_output_queues: usize,
    pub unknown_output_connections: usize,
}

#[derive(Debug)]
pub struct AuthoritativePlayRuntime {
    workers: WorkerRuntime<PlayInput, PlayOutput>,
    inputs: BoundedInputQueue<PlayInput>,
    clock: FixedRateClock,
    max_commands_per_poll: NonZeroUsize,
    max_inputs_per_tick: NonZeroUsize,
    connections: BTreeMap<ConnectionId, ConnectionRuntimeState>,
    keep_alive_interval_ticks: u64,
    next_keep_alive_id: i64,
}

impl AuthoritativePlayRuntime {
    const DEFAULT_KEEP_ALIVE_INTERVAL_TICKS: u64 = 20 * 15;

    pub fn new(
        workers: WorkerRuntime<PlayInput, PlayOutput>,
        start: Instant,
        queue_capacity: NonZeroUsize,
        max_commands_per_poll: NonZeroUsize,
        max_inputs_per_tick: NonZeroUsize,
        max_catch_up: NonZeroU32,
    ) -> Result<Self> {
        Self::with_keep_alive_interval_ticks(
            workers,
            start,
            queue_capacity,
            max_commands_per_poll,
            max_inputs_per_tick,
            max_catch_up,
            Self::DEFAULT_KEEP_ALIVE_INTERVAL_TICKS,
        )
    }

    pub fn with_keep_alive_interval_ticks(
        workers: WorkerRuntime<PlayInput, PlayOutput>,
        start: Instant,
        queue_capacity: NonZeroUsize,
        max_commands_per_poll: NonZeroUsize,
        max_inputs_per_tick: NonZeroUsize,
        max_catch_up: NonZeroU32,
        keep_alive_interval_ticks: u64,
    ) -> Result<Self> {
        if keep_alive_interval_ticks == 0 {
            anyhow::bail!("authoritative keep alive interval must be greater than zero");
        }
        Ok(Self {
            workers,
            inputs: BoundedInputQueue::new(queue_capacity),
            clock: FixedRateClock::server_clock(start, max_catch_up)
                .context("cannot initialize authoritative 20 TPS clock")?,
            max_commands_per_poll,
            max_inputs_per_tick,
            connections: BTreeMap::new(),
            keep_alive_interval_ticks,
            next_keep_alive_id: 1,
        })
    }

    pub fn poll(&mut self, now: Instant) -> Result<AuthoritativePollReport> {
        let ingress = self
            .workers
            .ingest_available(&mut self.inputs, self.max_commands_per_poll.get())
            .context("cannot ingest network-worker commands")?;
        let removed_connections = self.remove_unregistered_connection_state();
        let Some(batch) = self
            .clock
            .poll(now)
            .context("cannot advance authoritative 20 TPS clock")?
        else {
            return Ok(AuthoritativePollReport {
                ingress,
                removed_connections,
                ..AuthoritativePollReport::default()
            });
        };

        let mut tick_report = AuthoritativePollReport::default();
        for tick in batch.ticks() {
            let report = self.execute_tick(tick);
            tick_report.processed_inputs += report.processed_inputs;
            tick_report.sent_outputs += report.sent_outputs;
            tick_report.full_output_queues += report.full_output_queues;
            tick_report.disconnected_output_queues += report.disconnected_output_queues;
            tick_report.unknown_output_connections += report.unknown_output_connections;
        }

        Ok(AuthoritativePollReport {
            ingress,
            removed_connections,
            executed_ticks: batch.count,
            dropped_ticks: batch.dropped,
            processed_inputs: tick_report.processed_inputs,
            sent_outputs: tick_report.sent_outputs,
            full_output_queues: tick_report.full_output_queues,
            disconnected_output_queues: tick_report.disconnected_output_queues,
            unknown_output_connections: tick_report.unknown_output_connections,
        })
    }

    fn remove_unregistered_connection_state(&mut self) -> usize {
        let previous = self.connections.len();
        let workers = &self.workers;
        self.connections
            .retain(|connection, _| workers.contains_connection(*connection));
        previous - self.connections.len()
    }

    fn execute_tick(&mut self, _tick: Tick) -> AuthoritativePollReport {
        let inputs = self.inputs.drain_tick(self.max_inputs_per_tick.get());
        let mut report = AuthoritativePollReport {
            processed_inputs: inputs.len(),
            ..AuthoritativePollReport::default()
        };
        for input in inputs {
            match input.payload {
                PlayInput::ClientTickEnd => {
                    let state = self.connections.entry(input.connection).or_default();
                    state.client_ticks = state.client_ticks.saturating_add(1);
                    state.ticks_since_keep_alive_request =
                        state.ticks_since_keep_alive_request.saturating_add(1);
                    if !state.keep_alive_pending
                        && state.ticks_since_keep_alive_request >= self.keep_alive_interval_ticks
                    {
                        let id = self.next_keep_alive_id;
                        self.next_keep_alive_id = self.next_keep_alive_id.saturating_add(1);
                        state.last_keep_alive_request = Some(id);
                        state.keep_alive_pending = true;
                        state.ticks_since_keep_alive_request = 0;
                        self.record_output_result(
                            input.connection,
                            PlayOutput::KeepAliveRequest(id),
                            &mut report,
                        );
                    }
                }
                PlayInput::KeepAliveResponse(id) => {
                    let state = self.connections.entry(input.connection).or_default();
                    let expected = state.last_keep_alive_request;
                    state.last_keep_alive_response = Some(id);
                    if expected == Some(id) {
                        state.keep_alive_pending = false;
                    } else if state.keep_alive_pending {
                        self.record_output_result(
                            input.connection,
                            PlayOutput::Disconnect(format!(
                                "expected keep alive id {}, got {id}",
                                expected.unwrap_or_default()
                            )),
                            &mut report,
                        );
                    }
                }
                PlayInput::Movement(movement) => {
                    self.connections
                        .entry(input.connection)
                        .or_default()
                        .last_movement = Some(movement);
                }
                PlayInput::ChunkBatchReceived(desired) => {
                    self.connections
                        .entry(input.connection)
                        .or_default()
                        .desired_chunks_per_tick = Some(desired);
                }
                PlayInput::Disconnected => {
                    self.connections.remove(&input.connection);
                }
            }
        }
        report
    }

    fn record_output_result(
        &mut self,
        connection: ConnectionId,
        output: PlayOutput,
        report: &mut AuthoritativePollReport,
    ) {
        match self.workers.try_send_output(connection, output) {
            Ok(()) => report.sent_outputs += 1,
            Err(WorkerOutputError::Full { .. }) => report.full_output_queues += 1,
            Err(WorkerOutputError::WorkerDisconnected { .. }) => {
                report.disconnected_output_queues += 1;
            }
            Err(WorkerOutputError::UnknownConnection { .. }) => {
                report.unknown_output_connections += 1;
            }
        }
    }

    #[must_use]
    pub fn connection_state(&self, connection: ConnectionId) -> Option<&ConnectionRuntimeState> {
        self.connections.get(&connection)
    }

    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    #[must_use]
    pub fn pending_inputs(&self) -> usize {
        self.inputs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_runtime::worker_channel;
    use std::time::Duration;

    fn non_zero_usize(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    fn non_zero_u32(value: u32) -> NonZeroU32 {
        NonZeroU32::new(value).unwrap()
    }

    fn runtime_with_keep_alive_interval(
        workers: WorkerRuntime<PlayInput, PlayOutput>,
        start: Instant,
        keep_alive_interval_ticks: u64,
    ) -> AuthoritativePlayRuntime {
        AuthoritativePlayRuntime::with_keep_alive_interval_ticks(
            workers,
            start,
            non_zero_usize(32),
            non_zero_usize(32),
            non_zero_usize(8),
            non_zero_u32(4),
            keep_alive_interval_ticks,
        )
        .unwrap()
    }

    #[test]
    fn ingests_worker_input_on_authoritative_ticks() {
        let start = Instant::now();
        let (connector, workers) = worker_channel(non_zero_usize(16));
        let connection = ConnectionId::new(7);
        let worker = connector
            .try_connect(connection, non_zero_usize(4))
            .unwrap();
        worker.try_send_input(PlayInput::ClientTickEnd).unwrap();
        worker
            .try_send_input(PlayInput::KeepAliveResponse(41))
            .unwrap();

        let mut runtime = AuthoritativePlayRuntime::new(
            workers,
            start,
            non_zero_usize(32),
            non_zero_usize(32),
            non_zero_usize(8),
            non_zero_u32(4),
        )
        .unwrap();
        let report = runtime.poll(start + Duration::from_millis(50)).unwrap();

        assert_eq!(report.executed_ticks, 1);
        assert_eq!(report.processed_inputs, 2);
        assert_eq!(report.ingress.registrations, 1);
        let state = runtime.connection_state(connection).unwrap();
        assert_eq!(state.client_ticks, 1);
        assert_eq!(state.last_keep_alive_response, Some(41));
    }

    #[test]
    fn emits_semantic_keep_alive_requests_on_authoritative_ticks() {
        let start = Instant::now();
        let (connector, workers) = worker_channel(non_zero_usize(16));
        let connection = ConnectionId::new(17);
        let worker = connector
            .try_connect(connection, non_zero_usize(4))
            .unwrap();
        worker.try_send_input(PlayInput::ClientTickEnd).unwrap();

        let mut runtime = runtime_with_keep_alive_interval(workers, start, 1);
        let report = runtime.poll(start + Duration::from_millis(50)).unwrap();

        assert_eq!(report.executed_ticks, 1);
        assert_eq!(report.processed_inputs, 1);
        assert_eq!(report.sent_outputs, 1);
        assert_eq!(
            worker.try_recv_output().unwrap(),
            PlayOutput::KeepAliveRequest(1)
        );
        let state = runtime.connection_state(connection).unwrap();
        assert_eq!(state.last_keep_alive_request, Some(1));
        assert!(state.keep_alive_pending);
    }

    #[test]
    fn disconnects_mismatched_authoritative_keep_alive_responses() {
        let start = Instant::now();
        let (connector, workers) = worker_channel(non_zero_usize(16));
        let connection = ConnectionId::new(18);
        let worker = connector
            .try_connect(connection, non_zero_usize(4))
            .unwrap();
        worker.try_send_input(PlayInput::ClientTickEnd).unwrap();
        worker
            .try_send_input(PlayInput::KeepAliveResponse(99))
            .unwrap();

        let mut runtime = runtime_with_keep_alive_interval(workers, start, 1);
        let report = runtime.poll(start + Duration::from_millis(50)).unwrap();

        assert_eq!(report.processed_inputs, 2);
        assert_eq!(report.sent_outputs, 2);
        assert_eq!(
            worker.try_recv_output().unwrap(),
            PlayOutput::KeepAliveRequest(1)
        );
        assert_eq!(
            worker.try_recv_output().unwrap(),
            PlayOutput::Disconnect("expected keep alive id 1, got 99".to_owned())
        );
        let state = runtime.connection_state(connection).unwrap();
        assert_eq!(state.last_keep_alive_request, Some(1));
        assert_eq!(state.last_keep_alive_response, Some(99));
        assert!(state.keep_alive_pending);
    }

    #[test]
    fn disconnect_input_removes_connection_state() {
        let start = Instant::now();
        let (connector, workers) = worker_channel(non_zero_usize(16));
        let connection = ConnectionId::new(9);
        let worker = connector
            .try_connect(connection, non_zero_usize(4))
            .unwrap();
        worker.try_send_input(PlayInput::ClientTickEnd).unwrap();

        let mut runtime = AuthoritativePlayRuntime::new(
            workers,
            start,
            non_zero_usize(32),
            non_zero_usize(32),
            non_zero_usize(8),
            non_zero_u32(4),
        )
        .unwrap();
        runtime.poll(start + Duration::from_millis(50)).unwrap();
        assert_eq!(runtime.connection_count(), 1);

        worker.try_send_input(PlayInput::Disconnected).unwrap();
        runtime.poll(start + Duration::from_millis(100)).unwrap();
        assert_eq!(runtime.connection_count(), 0);
    }

    #[test]
    fn worker_disconnect_removes_state_before_the_next_tick() {
        let start = Instant::now();
        let (connector, workers) = worker_channel(non_zero_usize(16));
        let connection = ConnectionId::new(12);
        let worker = connector
            .try_connect(connection, non_zero_usize(4))
            .unwrap();
        worker.try_send_input(PlayInput::ClientTickEnd).unwrap();

        let mut runtime = AuthoritativePlayRuntime::new(
            workers,
            start,
            non_zero_usize(32),
            non_zero_usize(32),
            non_zero_usize(8),
            non_zero_u32(4),
        )
        .unwrap();
        runtime.poll(start + Duration::from_millis(50)).unwrap();
        assert_eq!(runtime.connection_count(), 1);

        worker.try_disconnect().unwrap();
        let report = runtime.poll(start + Duration::from_millis(60)).unwrap();
        assert_eq!(report.ingress.disconnections, 1);
        assert_eq!(report.removed_connections, 1);
        assert_eq!(report.executed_ticks, 0);
        assert!(runtime.connection_state(connection).is_none());
    }

    #[test]
    fn poll_before_deadline_only_ingests_commands() {
        let start = Instant::now();
        let (connector, workers) = worker_channel(non_zero_usize(8));
        let connection = ConnectionId::new(3);
        let worker = connector
            .try_connect(connection, non_zero_usize(2))
            .unwrap();
        worker.try_send_input(PlayInput::ClientTickEnd).unwrap();

        let mut runtime = AuthoritativePlayRuntime::new(
            workers,
            start,
            non_zero_usize(8),
            non_zero_usize(8),
            non_zero_usize(8),
            non_zero_u32(2),
        )
        .unwrap();
        let report = runtime.poll(start + Duration::from_millis(10)).unwrap();

        assert_eq!(report.executed_ticks, 0);
        assert_eq!(report.ingress.accepted_inputs, 1);
        assert_eq!(runtime.pending_inputs(), 1);
        assert!(runtime.connection_state(connection).is_none());
    }
}
