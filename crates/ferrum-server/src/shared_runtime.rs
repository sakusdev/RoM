use crate::{
    authoritative_runtime::{
        AuthoritativePlayRuntime, AuthoritativePollReport, PlayInput, PlayOutput,
    },
    play_connection::PlayWorkerConnector,
};
use anyhow::{Context, Result, bail};
use ferrum_runtime::worker_channel;
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::mpsc::{SyncSender, TryRecvError, TrySendError, sync_channel},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct SharedPlayRuntimeConfig {
    pub command_capacity: NonZeroUsize,
    pub input_capacity: NonZeroUsize,
    pub max_commands_per_poll: NonZeroUsize,
    pub max_inputs_per_tick: NonZeroUsize,
    pub max_catch_up: NonZeroU32,
    pub poll_interval: Duration,
    pub keep_alive_interval_ticks: NonZeroU32,
}

impl Default for SharedPlayRuntimeConfig {
    fn default() -> Self {
        Self {
            command_capacity: non_zero_usize(1_024),
            input_capacity: non_zero_usize(4_096),
            max_commands_per_poll: non_zero_usize(1_024),
            max_inputs_per_tick: non_zero_usize(1_024),
            max_catch_up: NonZeroU32::new(4).expect("four is non-zero"),
            poll_interval: Duration::from_millis(1),
            keep_alive_interval_ticks: NonZeroU32::new(20 * 15)
                .expect("default Keep Alive interval is non-zero"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SharedPlayRuntimeExit {
    pub polls: u64,
    pub ingress_commands: u64,
    pub registrations: u64,
    pub disconnections: u64,
    pub accepted_inputs: u64,
    pub dropped_inputs: u64,
    pub orphaned_inputs: u64,
    pub removed_connections: u64,
    pub executed_ticks: u64,
    pub dropped_ticks: u64,
    pub processed_inputs: u64,
    pub sent_outputs: u64,
    pub full_output_queues: u64,
    pub disconnected_output_queues: u64,
    pub unknown_output_connections: u64,
}

impl SharedPlayRuntimeExit {
    fn record(&mut self, report: AuthoritativePollReport) {
        self.polls = self.polls.saturating_add(1);
        self.ingress_commands = self
            .ingress_commands
            .saturating_add(report.ingress.commands as u64);
        self.registrations = self
            .registrations
            .saturating_add(report.ingress.registrations as u64);
        self.disconnections = self
            .disconnections
            .saturating_add(report.ingress.disconnections as u64);
        self.accepted_inputs = self
            .accepted_inputs
            .saturating_add(report.ingress.accepted_inputs as u64);
        self.dropped_inputs = self
            .dropped_inputs
            .saturating_add(report.ingress.dropped_inputs as u64);
        self.orphaned_inputs = self
            .orphaned_inputs
            .saturating_add(report.ingress.orphaned_inputs as u64);
        self.removed_connections = self
            .removed_connections
            .saturating_add(report.removed_connections as u64);
        self.executed_ticks = self
            .executed_ticks
            .saturating_add(u64::from(report.executed_ticks));
        self.dropped_ticks = self
            .dropped_ticks
            .saturating_add(u64::from(report.dropped_ticks));
        self.processed_inputs = self
            .processed_inputs
            .saturating_add(report.processed_inputs as u64);
        self.sent_outputs = self.sent_outputs.saturating_add(report.sent_outputs as u64);
        self.full_output_queues = self
            .full_output_queues
            .saturating_add(report.full_output_queues as u64);
        self.disconnected_output_queues = self
            .disconnected_output_queues
            .saturating_add(report.disconnected_output_queues as u64);
        self.unknown_output_connections = self
            .unknown_output_connections
            .saturating_add(report.unknown_output_connections as u64);
    }
}

#[derive(Debug)]
pub struct SharedPlayRuntime {
    connector: PlayWorkerConnector,
    shutdown: Option<SyncSender<()>>,
    worker: Option<JoinHandle<Result<SharedPlayRuntimeExit>>>,
}

impl SharedPlayRuntime {
    #[must_use]
    pub fn connector(&self) -> PlayWorkerConnector {
        self.connector.clone()
    }

    pub fn shutdown(mut self) -> Result<SharedPlayRuntimeExit> {
        self.signal_shutdown();
        self.join_worker()
    }

    fn signal_shutdown(&mut self) {
        let Some(shutdown) = self.shutdown.take() else {
            return;
        };
        match shutdown.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
        }
    }

    fn join_worker(&mut self) -> Result<SharedPlayRuntimeExit> {
        let Some(worker) = self.worker.take() else {
            bail!("shared Play runtime worker was already joined");
        };
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("shared Play runtime worker panicked"))?
    }
}

impl Drop for SharedPlayRuntime {
    fn drop(&mut self) {
        self.signal_shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn spawn_shared_play_runtime(config: SharedPlayRuntimeConfig) -> Result<SharedPlayRuntime> {
    if config.poll_interval.is_zero() {
        bail!("shared Play runtime poll interval must be greater than zero");
    }

    let (connector, workers) = worker_channel::<PlayInput, PlayOutput>(config.command_capacity);
    let runtime = AuthoritativePlayRuntime::with_keep_alive_interval_ticks(
        workers,
        Instant::now(),
        config.input_capacity,
        config.max_commands_per_poll,
        config.max_inputs_per_tick,
        config.max_catch_up,
        u64::from(config.keep_alive_interval_ticks.get()),
    )?;
    let (shutdown, shutdown_receiver) = sync_channel(1);
    let poll_interval = config.poll_interval;
    let worker = thread::Builder::new()
        .name("rom-authoritative-play".to_owned())
        .spawn(move || run_shared_play_runtime(runtime, shutdown_receiver, poll_interval))
        .context("cannot spawn shared authoritative Play runtime")?;

    Ok(SharedPlayRuntime {
        connector,
        shutdown: Some(shutdown),
        worker: Some(worker),
    })
}

fn run_shared_play_runtime(
    mut runtime: AuthoritativePlayRuntime,
    shutdown: std::sync::mpsc::Receiver<()>,
    poll_interval: Duration,
) -> Result<SharedPlayRuntimeExit> {
    let mut exit = SharedPlayRuntimeExit::default();
    loop {
        exit.record(runtime.poll(Instant::now())?);
        match shutdown.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => {
                exit.record(runtime.poll(Instant::now())?);
                return Ok(exit);
            }
            Err(TryRecvError::Empty) => thread::sleep(poll_interval),
        }
    }
}

fn non_zero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("shared Play runtime defaults are non-zero")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{authoritative_runtime::PlayInput, play_connection::register_play_connection};
    use ferrum_protocol::PacketKind;
    use ferrum_runtime::ConnectionId;

    #[test]
    fn rejects_zero_poll_interval() {
        let config = SharedPlayRuntimeConfig {
            poll_interval: Duration::ZERO,
            ..SharedPlayRuntimeConfig::default()
        };
        assert!(
            spawn_shared_play_runtime(config)
                .unwrap_err()
                .to_string()
                .contains("poll interval")
        );
    }

    #[test]
    fn starts_and_stops_cleanly() {
        let service = spawn_shared_play_runtime(SharedPlayRuntimeConfig::default()).unwrap();
        let exit = service.shutdown().unwrap();
        assert!(exit.polls >= 2);
    }

    #[test]
    fn final_shutdown_poll_ingests_connection_commands() {
        let service = spawn_shared_play_runtime(SharedPlayRuntimeConfig::default()).unwrap();
        let connector = service.connector();
        let (reader, _writer) = register_play_connection(
            &connector,
            ConnectionId::new(41),
            NonZeroUsize::new(4).unwrap(),
        )
        .unwrap();
        reader
            .try_submit_packet(PacketKind::ClientTickEnd, &[])
            .unwrap();

        let exit = service.shutdown().unwrap();
        assert_eq!(exit.registrations, 1);
        assert_eq!(exit.accepted_inputs, 1);
        assert!(exit.ingress_commands >= 2);
    }

    #[test]
    fn shared_runtime_routes_semantic_keep_alive_outputs() {
        let service = spawn_shared_play_runtime(SharedPlayRuntimeConfig {
            keep_alive_interval_ticks: NonZeroU32::new(1).unwrap(),
            ..SharedPlayRuntimeConfig::default()
        })
        .unwrap();
        let connector = service.connector();
        let (reader, writer) = register_play_connection(
            &connector,
            ConnectionId::new(61),
            NonZeroUsize::new(4).unwrap(),
        )
        .unwrap();
        reader.try_submit_input(PlayInput::ClientTickEnd).unwrap();

        std::thread::sleep(Duration::from_millis(75));

        assert_eq!(
            writer.try_recv_output().unwrap(),
            PlayOutput::KeepAliveRequest(1)
        );
        let exit = service.shutdown().unwrap();
        assert!(exit.sent_outputs >= 1);
    }

    #[test]
    fn connector_disconnect_is_observed_before_exit() {
        let service = spawn_shared_play_runtime(SharedPlayRuntimeConfig::default()).unwrap();
        let connector = service.connector();
        let (reader, _writer) = register_play_connection(
            &connector,
            ConnectionId::new(52),
            NonZeroUsize::new(2).unwrap(),
        )
        .unwrap();
        reader.try_submit_input(PlayInput::ClientTickEnd).unwrap();
        reader.try_disconnect().unwrap();

        let exit = service.shutdown().unwrap();
        assert_eq!(exit.registrations, 1);
        assert_eq!(exit.disconnections, 1);
    }
}
