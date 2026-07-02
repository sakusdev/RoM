use anyhow::{Context, Result};
use ferrum_play::PlayerMovement;
use ferrum_runtime::{
    BoundedInputQueue, ConnectionId, FixedRateClock, Tick, WorkerIngressReport, WorkerRuntime,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayOutput {
    Packet(Vec<u8>),
    Disconnect(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnectionRuntimeState {
    pub client_ticks: u64,
    pub last_keep_alive_response: Option<i64>,
    pub last_movement: Option<PlayerMovement>,
    pub desired_chunks_per_tick: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuthoritativePollReport {
    pub ingress: WorkerIngressReport,
    pub executed_ticks: u32,
    pub dropped_ticks: u32,
    pub processed_inputs: usize,
}

#[derive(Debug)]
pub struct AuthoritativePlayRuntime {
    workers: WorkerRuntime<PlayInput, PlayOutput>,
    inputs: BoundedInputQueue<PlayInput>,
    clock: FixedRateClock,
    max_commands_per_poll: NonZeroUsize,
    max_inputs_per_tick: NonZeroUsize,
    connections: BTreeMap<ConnectionId, ConnectionRuntimeState>,
}

impl AuthoritativePlayRuntime {
    pub fn new(
        workers: WorkerRuntime<PlayInput, PlayOutput>,
        start: Instant,
        queue_capacity: NonZeroUsize,
        max_commands_per_poll: NonZeroUsize,
        max_inputs_per_tick: NonZeroUsize,
        max_catch_up: NonZeroU32,
    ) -> Result<Self> {
        Ok(Self {
            workers,
            inputs: BoundedInputQueue::new(queue_capacity),
            clock: FixedRateClock::server_clock(start, max_catch_up)
                .context("cannot initialize authoritative 20 TPS clock")?,
            max_commands_per_poll,
            max_inputs_per_tick,
            connections: BTreeMap::new(),
        })
    }

    pub fn poll(&mut self, now: Instant) -> Result<AuthoritativePollReport> {
        let ingress = self
            .workers
            .ingest_available(&mut self.inputs, self.max_commands_per_poll.get())
            .context("cannot ingest network-worker commands")?;
        let Some(batch) = self
            .clock
            .poll(now)
            .context("cannot advance authoritative 20 TPS clock")?
        else {
            return Ok(AuthoritativePollReport {
                ingress,
                ..AuthoritativePollReport::default()
            });
        };

        let mut processed_inputs = 0;
        for tick in batch.ticks() {
            processed_inputs += self.execute_tick(tick);
        }

        Ok(AuthoritativePollReport {
            ingress,
            executed_ticks: batch.count,
            dropped_ticks: batch.dropped,
            processed_inputs,
        })
    }

    fn execute_tick(&mut self, _tick: Tick) -> usize {
        let inputs = self.inputs.drain_tick(self.max_inputs_per_tick.get());
        let processed = inputs.len();
        for input in inputs {
            match input.payload {
                PlayInput::ClientTickEnd => {
                    let state = self.connections.entry(input.connection).or_default();
                    state.client_ticks = state.client_ticks.saturating_add(1);
                }
                PlayInput::KeepAliveResponse(id) => {
                    self.connections
                        .entry(input.connection)
                        .or_default()
                        .last_keep_alive_response = Some(id);
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
        processed
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
        let report = runtime
            .poll(start + Duration::from_millis(50))
            .unwrap();

        assert_eq!(report.executed_ticks, 1);
        assert_eq!(report.processed_inputs, 2);
        assert_eq!(report.ingress.registrations, 1);
        let state = runtime.connection_state(connection).unwrap();
        assert_eq!(state.client_ticks, 1);
        assert_eq!(state.last_keep_alive_response, Some(41));
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
        runtime
            .poll(start + Duration::from_millis(50))
            .unwrap();
        assert_eq!(runtime.connection_count(), 1);

        worker.try_send_input(PlayInput::Disconnected).unwrap();
        runtime
            .poll(start + Duration::from_millis(100))
            .unwrap();
        assert_eq!(runtime.connection_count(), 0);
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
        let report = runtime
            .poll(start + Duration::from_millis(10))
            .unwrap();

        assert_eq!(report.executed_ticks, 0);
        assert_eq!(report.ingress.accepted_inputs, 1);
        assert_eq!(runtime.pending_inputs(), 1);
        assert!(runtime.connection_state(connection).is_none());
    }
}
