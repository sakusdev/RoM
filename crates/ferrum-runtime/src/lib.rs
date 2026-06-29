//! Deterministic runtime primitives for RoM's authoritative server loop.
//!
//! This crate intentionally does not own sockets or Minecraft packet codecs. It
//! provides a fixed-rate tick clock, bounded per-connection input queues, and a
//! small state runner that applies queued inputs in a stable order.

use std::{
    collections::{BTreeMap, VecDeque},
    num::{NonZeroU32, NonZeroUsize},
    time::{Duration, Instant},
};

use thiserror::Error;

pub const SERVER_TICKS_PER_SECOND: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(u64);

impl ConnectionId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputSequence(u64);

impl InputSequence {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u64);

impl Tick {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputEnvelope<T> {
    pub connection: ConnectionId,
    pub sequence: InputSequence,
    pub payload: T,
}

#[derive(Debug, Clone)]
pub struct BoundedInputQueue<T> {
    capacity: NonZeroUsize,
    pending: BTreeMap<ConnectionId, VecDeque<InputEnvelope<T>>>,
    next_sequence: BTreeMap<ConnectionId, u64>,
    len: usize,
}

impl<T> BoundedInputQueue<T> {
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            pending: BTreeMap::new(),
            next_sequence: BTreeMap::new(),
            len: 0,
        }
    }

    pub fn try_new(capacity: usize) -> Result<Self, QueueError> {
        let capacity = NonZeroUsize::new(capacity).ok_or(QueueError::ZeroCapacity)?;
        Ok(Self::new(capacity))
    }

    pub fn push(
        &mut self,
        connection: ConnectionId,
        payload: T,
    ) -> Result<InputSequence, QueueError> {
        if self.len >= self.capacity.get() {
            return Err(QueueError::Full {
                capacity: self.capacity.get(),
            });
        }

        let next = self.next_sequence.entry(connection).or_insert(0);
        let sequence = InputSequence(*next);
        *next = next
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow { connection })?;

        self.pending
            .entry(connection)
            .or_default()
            .push_back(InputEnvelope {
                connection,
                sequence,
                payload,
            });
        self.len += 1;
        Ok(sequence)
    }

    /// Drain up to `max_events` in stable, fair round-robin order.
    ///
    /// Connections are visited by ascending [`ConnectionId`]. At most one input
    /// per connection is selected in each round, so a noisy connection cannot
    /// consume the complete per-tick budget while another connection is ready.
    pub fn drain_tick(&mut self, max_events: usize) -> Vec<InputEnvelope<T>> {
        let target = max_events.min(self.len);
        let mut drained = Vec::with_capacity(target);

        while drained.len() < target {
            let connection_ids = self.pending.keys().copied().collect::<Vec<_>>();
            let mut progressed = false;

            for connection in connection_ids {
                if drained.len() == target {
                    break;
                }
                let Some(queue) = self.pending.get_mut(&connection) else {
                    continue;
                };
                let Some(event) = queue.pop_front() else {
                    continue;
                };
                drained.push(event);
                self.len -= 1;
                progressed = true;
            }

            self.pending.retain(|_, queue| !queue.is_empty());
            if !progressed {
                break;
            }
        }

        drained
    }

    /// Remove all queued input and sequence state for a disconnected client.
    pub fn remove_connection(&mut self, connection: ConnectionId) -> usize {
        let removed = self.pending.remove(&connection).map_or(0, |queue| queue.len());
        self.len -= removed;
        self.next_sequence.remove(&connection);
        removed
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity.get()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueError {
    #[error("input queue capacity must be greater than zero")]
    ZeroCapacity,
    #[error("input queue reached its capacity of {capacity} events")]
    Full { capacity: usize },
    #[error("input sequence overflow for connection {connection:?}")]
    SequenceOverflow { connection: ConnectionId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickBatch {
    pub first: Tick,
    pub count: u32,
    pub dropped: u32,
}

impl TickBatch {
    pub fn ticks(self) -> impl ExactSizeIterator<Item = Tick> {
        (0..self.count).map(move |offset| Tick(self.first.0 + u64::from(offset)))
    }
}

#[derive(Debug, Clone)]
pub struct FixedRateClock {
    interval: Duration,
    next_deadline: Instant,
    current_tick: Tick,
    max_catch_up: NonZeroU32,
}

impl FixedRateClock {
    pub fn server_clock(start: Instant, max_catch_up: NonZeroU32) -> Result<Self, ClockError> {
        Self::new(
            start,
            NonZeroU32::new(SERVER_TICKS_PER_SECOND).expect("20 is non-zero"),
            max_catch_up,
        )
    }

    pub fn new(
        start: Instant,
        ticks_per_second: NonZeroU32,
        max_catch_up: NonZeroU32,
    ) -> Result<Self, ClockError> {
        let nanos = 1_000_000_000_u64 / u64::from(ticks_per_second.get());
        if nanos == 0 {
            return Err(ClockError::RateTooHigh {
                ticks_per_second: ticks_per_second.get(),
            });
        }
        let interval = Duration::from_nanos(nanos);
        let next_deadline = start
            .checked_add(interval)
            .ok_or(ClockError::DeadlineOverflow)?;
        Ok(Self {
            interval,
            next_deadline,
            current_tick: Tick::ZERO,
            max_catch_up,
        })
    }

    /// Return the authoritative ticks due at `now`.
    ///
    /// When the runtime falls behind, at most `max_catch_up` recent ticks are
    /// returned for execution. Older overdue ticks are reported as dropped and
    /// the clock advances over them so an overloaded server cannot enter an
    /// unbounded catch-up spiral.
    pub fn poll(&mut self, now: Instant) -> Result<Option<TickBatch>, ClockError> {
        if now < self.next_deadline {
            return Ok(None);
        }

        let overdue = now.duration_since(self.next_deadline);
        let extra_intervals = overdue.as_nanos() / self.interval.as_nanos();
        let due_u128 = extra_intervals.saturating_add(1);
        let due = u32::try_from(due_u128).unwrap_or(u32::MAX);
        let count = due.min(self.max_catch_up.get());
        let dropped = due - count;

        let advanced_tick = self
            .current_tick
            .0
            .checked_add(u64::from(due))
            .ok_or(ClockError::TickOverflow)?;
        let first = Tick(
            advanced_tick
                .checked_sub(u64::from(count))
                .and_then(|tick| tick.checked_add(1))
                .ok_or(ClockError::TickOverflow)?,
        );
        let advance = self
            .interval
            .checked_mul(due)
            .ok_or(ClockError::DeadlineOverflow)?;
        self.next_deadline = self
            .next_deadline
            .checked_add(advance)
            .ok_or(ClockError::DeadlineOverflow)?;
        self.current_tick = Tick(advanced_tick);

        Ok(Some(TickBatch {
            first,
            count,
            dropped,
        }))
    }

    #[must_use]
    pub const fn current_tick(&self) -> Tick {
        self.current_tick
    }

    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    #[must_use]
    pub const fn next_deadline(&self) -> Instant {
        self.next_deadline
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClockError {
    #[error("tick rate {ticks_per_second} is too high for nanosecond scheduling")]
    RateTooHigh { ticks_per_second: u32 },
    #[error("authoritative tick number overflowed")]
    TickOverflow,
    #[error("tick deadline arithmetic overflowed")]
    DeadlineOverflow,
}

#[derive(Debug, Clone)]
pub struct DeterministicRuntime<S, E> {
    state: S,
    inputs: BoundedInputQueue<E>,
    max_events_per_tick: NonZeroUsize,
}

impl<S, E> DeterministicRuntime<S, E> {
    #[must_use]
    pub fn new(
        state: S,
        queue_capacity: NonZeroUsize,
        max_events_per_tick: NonZeroUsize,
    ) -> Self {
        Self {
            state,
            inputs: BoundedInputQueue::new(queue_capacity),
            max_events_per_tick,
        }
    }

    pub fn push_input(
        &mut self,
        connection: ConnectionId,
        payload: E,
    ) -> Result<InputSequence, QueueError> {
        self.inputs.push(connection, payload)
    }

    pub fn execute_tick(
        &mut self,
        tick: Tick,
        mut apply: impl FnMut(&mut S, Tick, InputEnvelope<E>),
    ) -> usize {
        let events = self.inputs.drain_tick(self.max_events_per_tick.get());
        let processed = events.len();
        for event in events {
            apply(&mut self.state, tick, event);
        }
        processed
    }

    #[must_use]
    pub const fn state(&self) -> &S {
        &self.state
    }

    #[must_use]
    pub const fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    #[must_use]
    pub const fn pending_inputs(&self) -> usize {
        self.inputs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn non_zero_usize(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    fn non_zero_u32(value: u32) -> NonZeroU32 {
        NonZeroU32::new(value).unwrap()
    }

    #[test]
    fn assigns_independent_monotonic_sequences() {
        let mut queue = BoundedInputQueue::try_new(8).unwrap();
        assert_eq!(
            queue.push(ConnectionId::new(2), "a").unwrap().get(),
            0
        );
        assert_eq!(
            queue.push(ConnectionId::new(2), "b").unwrap().get(),
            1
        );
        assert_eq!(
            queue.push(ConnectionId::new(9), "c").unwrap().get(),
            0
        );
    }

    #[test]
    fn enforces_a_global_queue_bound() {
        assert_eq!(
            BoundedInputQueue::<()>::try_new(0).unwrap_err(),
            QueueError::ZeroCapacity
        );
        let mut queue = BoundedInputQueue::try_new(1).unwrap();
        queue.push(ConnectionId::new(1), "first").unwrap();
        assert_eq!(
            queue.push(ConnectionId::new(2), "second").unwrap_err(),
            QueueError::Full { capacity: 1 }
        );
    }

    #[test]
    fn drains_connections_in_deterministic_fair_rounds() {
        let mut queue = BoundedInputQueue::try_new(16).unwrap();
        queue.push(ConnectionId::new(2), "2a").unwrap();
        queue.push(ConnectionId::new(1), "1a").unwrap();
        queue.push(ConnectionId::new(1), "1b").unwrap();
        queue.push(ConnectionId::new(2), "2b").unwrap();
        queue.push(ConnectionId::new(1), "1c").unwrap();

        let drained = queue
            .drain_tick(5)
            .into_iter()
            .map(|event| event.payload)
            .collect::<Vec<_>>();
        assert_eq!(drained, ["1a", "2a", "1b", "2b", "1c"]);
        assert!(queue.is_empty());
    }

    #[test]
    fn preserves_remaining_inputs_after_a_tick_budget() {
        let mut queue = BoundedInputQueue::try_new(8).unwrap();
        for value in 0..5 {
            queue.push(ConnectionId::new(1), value).unwrap();
        }
        assert_eq!(
            queue
                .drain_tick(2)
                .into_iter()
                .map(|event| event.payload)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn removing_a_connection_clears_pending_inputs_and_resets_sequence() {
        let connection = ConnectionId::new(7);
        let mut queue = BoundedInputQueue::try_new(8).unwrap();
        queue.push(connection, "a").unwrap();
        queue.push(connection, "b").unwrap();
        assert_eq!(queue.remove_connection(connection), 2);
        assert_eq!(queue.push(connection, "c").unwrap().get(), 0);
    }

    #[test]
    fn server_clock_ticks_at_twenty_tps() {
        let start = Instant::now();
        let mut clock = FixedRateClock::server_clock(start, non_zero_u32(4)).unwrap();
        assert_eq!(clock.interval(), Duration::from_millis(50));
        assert_eq!(clock.poll(start + Duration::from_millis(49)).unwrap(), None);

        let batch = clock
            .poll(start + Duration::from_millis(50))
            .unwrap()
            .unwrap();
        assert_eq!(batch.first, Tick::new(1));
        assert_eq!(batch.count, 1);
        assert_eq!(batch.dropped, 0);
        assert_eq!(batch.ticks().collect::<Vec<_>>(), [Tick::new(1)]);
    }

    #[test]
    fn clock_caps_catch_up_and_skips_old_overdue_ticks() {
        let start = Instant::now();
        let mut clock = FixedRateClock::server_clock(start, non_zero_u32(3)).unwrap();
        let batch = clock
            .poll(start + Duration::from_millis(500))
            .unwrap()
            .unwrap();
        assert_eq!(batch.count, 3);
        assert_eq!(batch.dropped, 7);
        assert_eq!(
            batch.ticks().collect::<Vec<_>>(),
            [Tick::new(8), Tick::new(9), Tick::new(10)]
        );
        assert_eq!(clock.current_tick(), Tick::new(10));
        assert_eq!(
            clock.poll(start + Duration::from_millis(500)).unwrap(),
            None
        );
    }

    #[test]
    fn runtime_applies_inputs_in_tick_order_with_a_fixed_budget() {
        let mut runtime = DeterministicRuntime::new(
            Vec::<(u64, u64, i32)>::new(),
            non_zero_usize(8),
            non_zero_usize(2),
        );
        runtime.push_input(ConnectionId::new(2), 20).unwrap();
        runtime.push_input(ConnectionId::new(1), 10).unwrap();
        runtime.push_input(ConnectionId::new(1), 11).unwrap();

        assert_eq!(
            runtime.execute_tick(Tick::new(4), |state, tick, event| {
                state.push((tick.get(), event.connection.get(), event.payload));
            }),
            2
        );
        assert_eq!(runtime.state(), &[(4, 1, 10), (4, 2, 20)]);
        assert_eq!(runtime.pending_inputs(), 1);

        runtime.execute_tick(Tick::new(5), |state, tick, event| {
            state.push((tick.get(), event.connection.get(), event.payload));
        });
        assert_eq!(runtime.state()[2], (5, 1, 11));
    }
}
