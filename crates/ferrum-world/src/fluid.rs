use thiserror::Error;

use crate::BlockPos;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FluidKind {
    Empty,
    Water,
    Lava,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FluidState {
    pub kind: FluidKind,
    pub level: u8,
    pub falling: bool,
}

impl FluidState {
    pub fn new(kind: FluidKind, level: u8, falling: bool) -> Result<Self, FluidError> {
        match &kind {
            FluidKind::Empty if level != 0 => return Err(FluidError::EmptyFluidHasLevel { level }),
            FluidKind::Empty => {}
            _ if !(1..=8).contains(&level) => return Err(FluidError::InvalidLevel { level }),
            _ => {}
        }
        Ok(Self {
            kind,
            level,
            falling,
        })
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self {
            kind: FluidKind::Empty,
            level: 0,
            falling: false,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self.kind, FluidKind::Empty)
    }

    #[must_use]
    pub fn next_horizontal_level(&self) -> Self {
        if self.is_empty() || self.level >= 8 {
            return Self::empty();
        }
        Self {
            kind: self.kind.clone(),
            level: self.level + 1,
            falling: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FluidFlowPlan {
    pub source: BlockPos,
    pub downward: BlockPos,
    pub horizontal: [BlockPos; 4],
}

impl FluidFlowPlan {
    #[must_use]
    pub fn around(source: BlockPos) -> Self {
        Self {
            source,
            downward: BlockPos {
                x: source.x,
                y: source.y - 1,
                z: source.z,
            },
            horizontal: [
                BlockPos {
                    x: source.x - 1,
                    ..source
                },
                BlockPos {
                    x: source.x + 1,
                    ..source
                },
                BlockPos {
                    z: source.z - 1,
                    ..source
                },
                BlockPos {
                    z: source.z + 1,
                    ..source
                },
            ],
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FluidError {
    #[error("fluid level {level} must be between 1 and 8")]
    InvalidLevel { level: u8 },
    #[error("empty fluid must have level zero, got {level}")]
    EmptyFluidHasLevel { level: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flowing_levels_are_bounded() {
        let water = FluidState::new(FluidKind::Water, 1, false).unwrap();
        assert_eq!(water.next_horizontal_level().level, 2);
        let last = FluidState::new(FluidKind::Water, 8, false).unwrap();
        assert!(last.next_horizontal_level().is_empty());
    }
}
