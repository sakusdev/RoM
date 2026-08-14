//! Authoritative block-breaking sessions.
//!
//! A client may announce start/abort/stop actions, but only server game time
//! decides whether a target has been mined for long enough to break.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::BlockPos;

pub const MAX_MINING_TICKS: u32 = 20 * 60 * 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiningSession {
    pub position: BlockPos,
    pub started_at_tick: u64,
    pub required_ticks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MiningCompletion {
    pub position: BlockPos,
    pub started_at_tick: u64,
    pub completed_at_tick: u64,
    pub required_ticks: u32,
    pub elapsed_ticks: u64,
}

impl MiningSession {
    pub fn new(
        position: BlockPos,
        started_at_tick: u64,
        required_ticks: u32,
    ) -> Result<Self, MiningSessionError> {
        if required_ticks == 0 || required_ticks > MAX_MINING_TICKS {
            return Err(MiningSessionError::InvalidRequiredTicks { required_ticks });
        }
        Ok(Self {
            position,
            started_at_tick,
            required_ticks,
        })
    }

    #[must_use]
    pub fn elapsed_ticks(self, current_tick: u64) -> u64 {
        current_tick.saturating_sub(self.started_at_tick)
    }

    #[must_use]
    pub fn progress(self, current_tick: u64) -> f64 {
        (self.elapsed_ticks(current_tick) as f64 / f64::from(self.required_ticks)).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn ready(self, current_tick: u64) -> bool {
        self.elapsed_ticks(current_tick) >= u64::from(self.required_ticks)
    }

    pub fn complete(
        self,
        position: BlockPos,
        current_tick: u64,
    ) -> Result<MiningCompletion, MiningSessionError> {
        if position != self.position {
            return Err(MiningSessionError::WrongTarget {
                expected: self.position,
                actual: position,
            });
        }
        let elapsed_ticks = self.elapsed_ticks(current_tick);
        if elapsed_ticks < u64::from(self.required_ticks) {
            return Err(MiningSessionError::TooEarly {
                required_ticks: self.required_ticks,
                elapsed_ticks,
            });
        }
        Ok(MiningCompletion {
            position,
            started_at_tick: self.started_at_tick,
            completed_at_tick: current_tick,
            required_ticks: self.required_ticks,
            elapsed_ticks,
        })
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum MiningSessionError {
    #[error("required mining ticks {required_ticks} are outside 1..={MAX_MINING_TICKS}")]
    InvalidRequiredTicks { required_ticks: u32 },
    #[error("mining target changed from {expected:?} to {actual:?}")]
    WrongTarget { expected: BlockPos, actual: BlockPos },
    #[error("mining stopped after {elapsed_ticks} ticks but requires {required_ticks}")]
    TooEarly {
        required_ticks: u32,
        elapsed_ticks: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos() -> BlockPos {
        BlockPos { x: 1, y: 64, z: -2 }
    }

    #[test]
    fn progress_is_server_tick_based_and_clamped() {
        let session = MiningSession::new(pos(), 100, 20).unwrap();
        assert_eq!(session.progress(90), 0.0);
        assert_eq!(session.progress(110), 0.5);
        assert_eq!(session.progress(120), 1.0);
        assert_eq!(session.progress(500), 1.0);
    }

    #[test]
    fn early_stop_is_rejected() {
        let session = MiningSession::new(pos(), 100, 5).unwrap();
        assert_eq!(
            session.complete(pos(), 104).unwrap_err(),
            MiningSessionError::TooEarly {
                required_ticks: 5,
                elapsed_ticks: 4,
            }
        );
    }

    #[test]
    fn completed_session_reports_elapsed_ticks() {
        let session = MiningSession::new(pos(), 100, 5).unwrap();
        let completion = session.complete(pos(), 106).unwrap();
        assert_eq!(completion.elapsed_ticks, 6);
        assert_eq!(completion.required_ticks, 5);
    }

    #[test]
    fn target_swap_is_rejected() {
        let session = MiningSession::new(pos(), 0, 1).unwrap();
        let other = BlockPos { x: 2, ..pos() };
        assert!(matches!(
            session.complete(other, 1),
            Err(MiningSessionError::WrongTarget { .. })
        ));
    }
}