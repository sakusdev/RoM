//! Deterministic scheduled-tick queue for blocks and fluids.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{BlockPos, validate_resource_location};

pub const MAX_SCHEDULED_TICKS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickPriority {
    ExtremelyHigh,
    VeryHigh,
    High,
    Normal,
    Low,
    VeryLow,
    ExtremelyLow,
}

impl TickPriority {
    const fn rank(self) -> u8 {
        match self {
            Self::ExtremelyHigh => 0,
            Self::VeryHigh => 1,
            Self::High => 2,
            Self::Normal => 3,
            Self::Low => 4,
            Self::VeryLow => 5,
            Self::ExtremelyLow => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTick {
    pub target: String,
    pub position: BlockPos,
    pub trigger_tick: u64,
    pub priority: TickPriority,
    pub sequence: u64,
}

impl Ord for ScheduledTick {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .trigger_tick
            .cmp(&self.trigger_tick)
            .then_with(|| other.priority.rank().cmp(&self.priority.rank()))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for ScheduledTick {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScheduledTickQueue {
    heap: BinaryHeap<ScheduledTick>,
    dedupe: BTreeSet<(String, i32, i32, i32)>,
    next_sequence: u64,
}

impl ScheduledTickQueue {
    #[must_use]
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn schedule(
        &mut self,
        target: impl Into<String>,
        position: BlockPos,
        trigger_tick: u64,
        priority: TickPriority,
    ) -> Result<bool, ScheduledTickError> {
        if self.heap.len() >= MAX_SCHEDULED_TICKS {
            return Err(ScheduledTickError::QueueFull);
        }
        let target = target.into();
        if !validate_resource_location(&target) {
            return Err(ScheduledTickError::InvalidTarget { target });
        }
        let key = (target.clone(), position.x, position.y, position.z);
        if !self.dedupe.insert(key) {
            return Ok(false);
        }
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.heap.push(ScheduledTick {
            target,
            position,
            trigger_tick,
            priority,
            sequence,
        });
        Ok(true)
    }

    pub fn pop_due(&mut self, now: u64, limit: usize) -> Vec<ScheduledTick> {
        let mut due = Vec::with_capacity(limit.min(self.heap.len()));
        while due.len() < limit {
            let Some(next) = self.heap.peek() else {
                break;
            };
            if next.trigger_tick > now {
                break;
            }
            let next = self.heap.pop().expect("peeked scheduled tick must exist");
            self.dedupe.remove(&(
                next.target.clone(),
                next.position.x,
                next.position.y,
                next.position.z,
            ));
            due.push(next);
        }
        due
    }

    #[must_use]
    pub fn contains(&self, target: &str, position: BlockPos) -> bool {
        self.dedupe
            .contains(&(target.to_owned(), position.x, position.y, position.z))
    }

    pub fn clear(&mut self) {
        self.heap.clear();
        self.dedupe.clear();
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScheduledTickError {
    #[error("invalid scheduled tick target {target}")]
    InvalidTarget { target: String },
    #[error("scheduled tick queue reached its hard limit")]
    QueueFull,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: i32) -> BlockPos {
        BlockPos { x, y: 64, z: 0 }
    }

    #[test]
    fn due_ticks_are_ordered_by_time_priority_and_sequence() {
        let mut queue = ScheduledTickQueue::default();
        queue
            .schedule("minecraft:water", pos(0), 10, TickPriority::Normal)
            .unwrap();
        queue
            .schedule("minecraft:lava", pos(1), 10, TickPriority::High)
            .unwrap();
        queue
            .schedule("minecraft:stone", pos(2), 9, TickPriority::Low)
            .unwrap();
        let due = queue.pop_due(10, 8);
        assert_eq!(due[0].target, "minecraft:stone");
        assert_eq!(due[1].target, "minecraft:lava");
        assert_eq!(due[2].target, "minecraft:water");
    }

    #[test]
    fn duplicate_target_and_position_is_rejected() {
        let mut queue = ScheduledTickQueue::default();
        assert!(queue
            .schedule("minecraft:water", pos(0), 10, TickPriority::Normal)
            .unwrap());
        assert!(!queue
            .schedule("minecraft:water", pos(0), 20, TickPriority::High)
            .unwrap());
    }
}
