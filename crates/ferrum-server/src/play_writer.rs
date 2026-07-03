use crate::{
    authoritative_runtime::PlayOutput,
    play_connection::PlayWriterEndpoint,
};
use anyhow::{Context, Result, bail};
use ferrum_runtime::WorkerWaitError;
use std::{
    sync::mpsc::{SyncSender, TryRecvError, TrySendError, sync_channel},
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayWriterDirective {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayWriterExitReason {
    Shutdown,
    RuntimeDisconnected,
    HandlerRequestedStop,
}

#[derive(Debug)]
pub struct PlayWriterExit<W> {
    writer: W,
    outputs: u64,
    reason: PlayWriterExitReason,
}

impl<W> PlayWriterExit<W> {
    #[must_use]
    pub const fn outputs(&self) -> u64 {
        self.outputs
    }

    #[must_use]
    pub const fn reason(&self) -> PlayWriterExitReason {
        self.reason
    }

    #[must_use]
    pub fn writer(&self) -> &W {
        &self.writer
    }

    #[must_use]
    pub fn into_writer(self) -> W {
        self.writer
    }
}

#[derive(Debug)]
pub struct PlayWriterWorker<W> {
    shutdown: Option<SyncSender<()>>,
    worker: Option<JoinHandle<Result<PlayWriterExit<W>>>>,
}

impl<W> PlayWriterWorker<W> {
    pub fn shutdown(mut self) -> Result<PlayWriterExit<W>> {
        self.signal_shutdown();
        self.join_worker()
    }

    pub fn join(mut self) -> Result<PlayWriterExit<W>> {
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

    fn join_worker(&mut self) -> Result<PlayWriterExit<W>> {
        let Some(worker) = self.worker.take() else {
            bail!("Play writer worker was already joined");
        };
        let result = worker
            .join()
            .map_err(|_| anyhow::anyhow!("Play writer worker panicked"))?;
        self.shutdown.take();
        result
    }
}

impl<W> Drop for PlayWriterWorker<W> {
    fn drop(&mut self) {
        self.signal_shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn spawn_play_writer<W, H>(
    endpoint: PlayWriterEndpoint,
    writer: W,
    wait_timeout: Duration,
    handler: H,
) -> Result<PlayWriterWorker<W>>
where
    W: Send + 'static,
    H: FnMut(&mut W, PlayOutput) -> Result<PlayWriterDirective> + Send + 'static,
{
    if wait_timeout.is_zero() {
        bail!("Play writer wait timeout must be greater than zero");
    }

    let connection = endpoint.connection_id();
    let (shutdown, shutdown_receiver) = sync_channel(1);
    let worker = thread::Builder::new()
        .name(format!("rom-play-writer-{}", connection.get()))
        .spawn(move || {
            run_play_writer(
                endpoint,
                writer,
                wait_timeout,
                shutdown_receiver,
                handler,
            )
        })
        .context("cannot spawn Play writer worker")?;

    Ok(PlayWriterWorker {
        shutdown: Some(shutdown),
        worker: Some(worker),
    })
}

fn run_play_writer<W, H>(
    endpoint: PlayWriterEndpoint,
    mut writer: W,
    wait_timeout: Duration,
    shutdown: std::sync::mpsc::Receiver<()>,
    mut handler: H,
) -> Result<PlayWriterExit<W>>
where
    H: FnMut(&mut W, PlayOutput) -> Result<PlayWriterDirective>,
{
    let mut outputs = 0_u64;
    loop {
        match shutdown.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => {
                return Ok(PlayWriterExit {
                    writer,
                    outputs,
                    reason: PlayWriterExitReason::Shutdown,
                });
            }
            Err(TryRecvError::Empty) => {}
        }

        let output = match endpoint.recv_output_timeout(wait_timeout) {
            Ok(output) => output,
            Err(WorkerWaitError::Timeout) => continue,
            Err(WorkerWaitError::RuntimeDisconnected) => {
                return Ok(PlayWriterExit {
                    writer,
                    outputs,
                    reason: PlayWriterExitReason::RuntimeDisconnected,
                });
            }
        };
        outputs = outputs.saturating_add(1);
        if handler(&mut writer, output)? == PlayWriterDirective::Stop {
            return Ok(PlayWriterExit {
                writer,
                outputs,
                reason: PlayWriterExitReason::HandlerRequestedStop,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play_connection::register_play_connection;
    use ferrum_runtime::{BoundedInputQueue, ConnectionId, worker_channel};
    use std::{io::Write, num::NonZeroUsize};

    fn non_zero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn rejects_zero_wait_timeout() {
        let (connector, _runtime) = worker_channel(non_zero(2));
        let (_reader, writer) =
            register_play_connection(&connector, ConnectionId::new(1), non_zero(1)).unwrap();
        let error = spawn_play_writer(writer, Vec::new(), Duration::ZERO, |_, _| {
            Ok(PlayWriterDirective::Continue)
        })
        .unwrap_err();
        assert!(error.to_string().contains("greater than zero"));
    }

    #[test]
    fn drains_queued_outputs_before_runtime_disconnect() {
        let (connector, mut runtime) = worker_channel(non_zero(4));
        let connection = ConnectionId::new(2);
        let (_reader, writer) =
            register_play_connection(&connector, connection, non_zero(2)).unwrap();
        let mut inputs = BoundedInputQueue::try_new(2).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        runtime
            .try_send_output(connection, PlayOutput::Packet(vec![1, 2, 3]))
            .unwrap();
        drop(runtime);

        let worker = spawn_play_writer(
            writer,
            Vec::new(),
            Duration::from_millis(1),
            |writer, output| {
                match output {
                    PlayOutput::Packet(bytes) => writer.write_all(&bytes)?,
                    PlayOutput::Disconnect(_) => return Ok(PlayWriterDirective::Stop),
                }
                Ok(PlayWriterDirective::Continue)
            },
        )
        .unwrap();
        let exit = worker.join().unwrap();
        assert_eq!(exit.outputs(), 1);
        assert_eq!(exit.reason(), PlayWriterExitReason::RuntimeDisconnected);
        assert_eq!(exit.into_writer(), vec![1, 2, 3]);
    }

    #[test]
    fn handler_can_request_writer_stop() {
        let (connector, mut runtime) = worker_channel(non_zero(4));
        let connection = ConnectionId::new(3);
        let (_reader, writer) =
            register_play_connection(&connector, connection, non_zero(2)).unwrap();
        let mut inputs = BoundedInputQueue::try_new(2).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();
        runtime
            .try_send_output(connection, PlayOutput::Disconnect("done".to_owned()))
            .unwrap();

        let worker = spawn_play_writer(
            writer,
            Vec::<u8>::new(),
            Duration::from_millis(1),
            |_, output| match output {
                PlayOutput::Packet(_) => Ok(PlayWriterDirective::Continue),
                PlayOutput::Disconnect(reason) => {
                    assert_eq!(reason, "done");
                    Ok(PlayWriterDirective::Stop)
                }
            },
        )
        .unwrap();
        let exit = worker.join().unwrap();
        assert_eq!(exit.outputs(), 1);
        assert_eq!(exit.reason(), PlayWriterExitReason::HandlerRequestedStop);
    }

    #[test]
    fn idle_writer_can_be_shutdown() {
        let (connector, mut runtime) = worker_channel(non_zero(4));
        let (_reader, writer) =
            register_play_connection(&connector, ConnectionId::new(4), non_zero(1)).unwrap();
        let mut inputs = BoundedInputQueue::try_new(2).unwrap();
        runtime.ingest_available(&mut inputs, 1).unwrap();

        let worker = spawn_play_writer(
            writer,
            Vec::<u8>::new(),
            Duration::from_millis(1),
            |_, _| Ok(PlayWriterDirective::Continue),
        )
        .unwrap();
        let exit = worker.shutdown().unwrap();
        assert_eq!(exit.outputs(), 0);
        assert_eq!(exit.reason(), PlayWriterExitReason::Shutdown);
    }
}
