use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::BlockPos;

pub const DEFAULT_MAX_SCHEDULED_TICKS: usize = 1_000_000;
pub const MAX_TICKS_PER_DRAIN: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TickKind {
    Block,
    Fluid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TickPriority {
    ExtremelyHigh,
    VeryHigh,
    High,
    Normal,
    Low,
    VeryLow,
    ExtremelyLow,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScheduledTickKey {
    pub kind: TickKind,
    pub position: BlockPos,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTick {
    pub due_tick: u64,
    pub priority: TickPriority,
    pub sequence: u64,
    pub key: ScheduledTickKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickScheduler {
    current_tick: u64,
    next_sequence: u64,
    max_pending: usize,
    due: BTreeMap<u64, Vec<ScheduledTick>>,
    pending: BTreeSet<ScheduledTickKey>,
}

impl Default for TickScheduler {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_SCHEDULED_TICKS)
            .expect("default scheduled-tick limit is greater than zero")
    }
}

impl TickScheduler {
    pub fn new(max_pending: usize) -> Result<Self, TickSchedulerError> {
        if max_pending == 0 {
            return Err(TickSchedulerError::ZeroPendingLimit);
        }
        Ok(Self {
            current_tick: 0,
            next_sequence: 0,
            max_pending,
            due: BTreeMap::new(),
            pending: BTreeSet::new(),
        })
    }

    #[must_use]
    pub const fn current_tick(&self) -> u64 {
        self.current_tick
    }

    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn schedule(
        &mut self,
        delay_ticks: u64,
        priority: TickPriority,
        key: ScheduledTickKey,
    ) -> Result<bool, TickSchedulerError> {
        if key.target.is_empty()
            || key.target.len() > 256
            || key.target.chars().any(char::is_control)
        {
            return Err(TickSchedulerError::InvalidTarget { target: key.target });
        }
        if self.pending.contains(&key) {
            return Ok(false);
        }
        if self.pending.len() >= self.max_pending {
            return Err(TickSchedulerError::QueueFull {
                limit: self.max_pending,
            });
        }
        let due_tick = self
            .current_tick
            .checked_add(delay_ticks)
            .ok_or(TickSchedulerError::TickOverflow)?;
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(TickSchedulerError::SequenceOverflow)?;
        let scheduled = ScheduledTick {
            due_tick,
            priority,
            sequence,
            key: key.clone(),
        };
        self.pending.insert(key);
        self.due.entry(due_tick).or_default().push(scheduled);
        Ok(true)
    }

    pub fn advance_to(
        &mut self,
        tick: u64,
        max_ticks: usize,
    ) -> Result<Vec<ScheduledTick>, TickSchedulerError> {
        if tick < self.current_tick {
            return Err(TickSchedulerError::CannotRewind {
                current: self.current_tick,
                requested: tick,
            });
        }
        if max_ticks == 0 || max_ticks > MAX_TICKS_PER_DRAIN {
            return Err(TickSchedulerError::InvalidDrainLimit {
                requested: max_ticks,
                maximum: MAX_TICKS_PER_DRAIN,
            });
        }
        self.current_tick = tick;
        let due_keys = self
            .due
            .range(..=tick)
            .map(|(due_tick, _)| *due_tick)
            .collect::<Vec<_>>();
        let mut ready = Vec::new();
        for due_tick in due_keys {
            let mut ticks = self
                .due
                .remove(&due_tick)
                .expect("due key came from scheduler map");
            ticks.sort_by_key(|scheduled| (scheduled.priority, scheduled.sequence));
            for scheduled in ticks {
                if ready.len() >= max_ticks {
                    self.due
                        .entry(scheduled.due_tick)
                        .or_default()
                        .push(scheduled);
                    continue;
                }
                self.pending.remove(&scheduled.key);
                ready.push(scheduled);
            }
        }
        Ok(ready)
    }

    pub fn cancel(&mut self, key: &ScheduledTickKey) -> bool {
        if !self.pending.remove(key) {
            return false;
        }
        for ticks in self.due.values_mut() {
            ticks.retain(|scheduled| &scheduled.key != key);
        }
        self.due.retain(|_, ticks| !ticks.is_empty());
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomTickSelector {
    state: u64,
}

impl RandomTickSelector {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    #[must_use]
    pub fn next_local_position(&mut self, section_y: i32) -> BlockPos {
        let value = self.next_u64();
        BlockPos {
            x: (value & 15) as i32,
            y: section_y * 16 + ((value >> 4) & 15) as i32,
            z: ((value >> 8) & 15) as i32,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state = self.state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        self.state
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TickSchedulerError {
    #[error("scheduled-tick pending limit must be greater than zero")]
    ZeroPendingLimit,
    #[error("scheduled-tick target is invalid: {target}")]
    InvalidTarget { target: String },
    #[error("scheduled-tick queue reached limit {limit}")]
    QueueFull { limit: usize },
    #[error("scheduled-tick time overflowed")]
    TickOverflow,
    #[error("scheduled-tick sequence overflowed")]
    SequenceOverflow,
    #[error("cannot rewind scheduled ticks from {current} to {requested}")]
    CannotRewind { current: u64, requested: u64 },
    #[error("scheduled-tick drain limit {requested} must be between 1 and {maximum}")]
    InvalidDrainLimit { requested: usize, maximum: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(x: i32, target: &str) -> ScheduledTickKey {
        ScheduledTickKey {
            kind: TickKind::Block,
            position: BlockPos { x, y: 64, z: 0 },
            target: target.to_owned(),
        }
    }

    #[test]
    fn scheduler_deduplicates_and_orders_ticks() {
        let mut scheduler = TickScheduler::new(8).unwrap();
        assert!(
            scheduler
                .schedule(2, TickPriority::Normal, key(1, "minecraft:stone"))
                .unwrap()
        );
        assert!(
            !scheduler
                .schedule(2, TickPriority::High, key(1, "minecraft:stone"))
                .unwrap()
        );
        scheduler
            .schedule(2, TickPriority::High, key(2, "minecraft:water"))
            .unwrap();
        let ready = scheduler.advance_to(2, 8).unwrap();
        assert_eq!(ready[0].key.position.x, 2);
        assert_eq!(ready[1].key.position.x, 1);
        assert_eq!(scheduler.pending_len(), 0);
    }

    #[test]
    fn random_tick_selection_is_replayable() {
        let mut left = RandomTickSelector::new(42);
        let mut right = RandomTickSelector::new(42);
        for _ in 0..100 {
            assert_eq!(left.next_local_position(3), right.next_local_position(3));
        }
    }
}
