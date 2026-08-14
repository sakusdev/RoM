//! Version-neutral block placement and interaction helpers.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockFace {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl BlockFace {
    #[must_use]
    pub const fn offset(self) -> [i32; 3] {
        match self {
            Self::Down => [0, -1, 0],
            Self::Up => [0, 1, 0],
            Self::North => [0, 0, -1],
            Self::South => [0, 0, 1],
            Self::West => [-1, 0, 0],
            Self::East => [1, 0, 0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    #[must_use]
    pub const fn offset(self, face: BlockFace) -> Self {
        let [x, y, z] = face.offset();
        Self {
            x: self.x + x,
            y: self.y + y,
            z: self.z + z,
        }
    }

    #[must_use]
    pub fn center(self) -> [f64; 3] {
        [
            f64::from(self.x) + 0.5,
            f64::from(self.y) + 0.5,
            f64::from(self.z) + 0.5,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlacementContext {
    pub clicked: BlockPos,
    pub face: BlockFace,
    pub player_eye: [f64; 3],
    pub interaction_range: f64,
    pub mode: PlacementMode,
    pub clicked_replaceable: bool,
    pub adjacent_replaceable: bool,
    pub collides_with_player: bool,
    pub held_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementDecision {
    pub target: BlockPos,
    pub consume_item: bool,
}

pub fn evaluate_placement(
    context: PlacementContext,
) -> Result<PlacementDecision, BlockInteractionError> {
    validate_context(context)?;
    if matches!(context.mode, PlacementMode::Spectator) {
        return Err(BlockInteractionError::ModeCannotPlace);
    }
    if context.held_count == 0 && !matches!(context.mode, PlacementMode::Creative) {
        return Err(BlockInteractionError::EmptyHand);
    }

    let target = if context.clicked_replaceable {
        context.clicked
    } else {
        context.clicked.offset(context.face)
    };

    if !context.clicked_replaceable && !context.adjacent_replaceable {
        return Err(BlockInteractionError::TargetOccupied { target });
    }
    if context.collides_with_player {
        return Err(BlockInteractionError::PlayerCollision { target });
    }
    if !within_reach(context.player_eye, target.center(), context.interaction_range) {
        return Err(BlockInteractionError::OutOfReach { target });
    }

    Ok(PlacementDecision {
        target,
        consume_item: !matches!(context.mode, PlacementMode::Creative),
    })
}

#[must_use]
pub fn within_reach(origin: [f64; 3], target: [f64; 3], range: f64) -> bool {
    if origin.into_iter().chain(target).any(|value| !value.is_finite())
        || !range.is_finite()
        || range < 0.0
    {
        return false;
    }
    let dx = origin[0] - target[0];
    let dy = origin[1] - target[1];
    let dz = origin[2] - target[2];
    dx * dx + dy * dy + dz * dz <= range * range
}

#[must_use]
pub fn hit_vector_inside_block(hit: [f32; 3]) -> bool {
    hit.into_iter()
        .all(|value| value.is_finite() && (-0.001..=1.001).contains(&value))
}

#[must_use]
pub fn yaw_to_horizontal_face(yaw_degrees: f32) -> BlockFace {
    let normalized = yaw_degrees.rem_euclid(360.0);
    if (45.0..135.0).contains(&normalized) {
        BlockFace::West
    } else if (135.0..225.0).contains(&normalized) {
        BlockFace::North
    } else if (225.0..315.0).contains(&normalized) {
        BlockFace::East
    } else {
        BlockFace::South
    }
}

#[must_use]
pub fn opposite(face: BlockFace) -> BlockFace {
    match face {
        BlockFace::Down => BlockFace::Up,
        BlockFace::Up => BlockFace::Down,
        BlockFace::North => BlockFace::South,
        BlockFace::South => BlockFace::North,
        BlockFace::West => BlockFace::East,
        BlockFace::East => BlockFace::West,
    }
}

fn validate_context(context: PlacementContext) -> Result<(), BlockInteractionError> {
    if context.player_eye.into_iter().any(|value| !value.is_finite()) {
        return Err(BlockInteractionError::NonFinitePosition);
    }
    if !context.interaction_range.is_finite() || context.interaction_range < 0.0 {
        return Err(BlockInteractionError::InvalidRange {
            range: context.interaction_range,
        });
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq)]
pub enum BlockInteractionError {
    #[error("placement position must be finite")]
    NonFinitePosition,
    #[error("interaction range {range} must be finite and non-negative")]
    InvalidRange { range: f64 },
    #[error("current game mode cannot place blocks")]
    ModeCannotPlace,
    #[error("cannot place from an empty hand")]
    EmptyHand,
    #[error("placement target {target:?} is occupied")]
    TargetOccupied { target: BlockPos },
    #[error("placement target {target:?} intersects the player")]
    PlayerCollision { target: BlockPos },
    #[error("placement target {target:?} is outside interaction range")]
    OutOfReach { target: BlockPos },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> PlacementContext {
        PlacementContext {
            clicked: BlockPos { x: 0, y: 64, z: 0 },
            face: BlockFace::Up,
            player_eye: [0.5, 65.6, 2.5],
            interaction_range: 4.5,
            mode: PlacementMode::Survival,
            clicked_replaceable: false,
            adjacent_replaceable: true,
            collides_with_player: false,
            held_count: 8,
        }
    }

    #[test]
    fn placement_targets_adjacent_face() {
        let result = evaluate_placement(context()).unwrap();
        assert_eq!(result.target, BlockPos { x: 0, y: 65, z: 0 });
        assert!(result.consume_item);
    }

    #[test]
    fn replaceable_clicked_block_is_reused() {
        let mut value = context();
        value.clicked_replaceable = true;
        let result = evaluate_placement(value).unwrap();
        assert_eq!(result.target, value.clicked);
    }

    #[test]
    fn creative_does_not_consume() {
        let mut value = context();
        value.mode = PlacementMode::Creative;
        value.held_count = 0;
        assert!(!evaluate_placement(value).unwrap().consume_item);
    }

    #[test]
    fn spectator_cannot_place() {
        let mut value = context();
        value.mode = PlacementMode::Spectator;
        assert_eq!(
            evaluate_placement(value).unwrap_err(),
            BlockInteractionError::ModeCannotPlace
        );
    }

    #[test]
    fn horizontal_faces_follow_yaw() {
        assert_eq!(yaw_to_horizontal_face(0.0), BlockFace::South);
        assert_eq!(yaw_to_horizontal_face(90.0), BlockFace::West);
        assert_eq!(yaw_to_horizontal_face(180.0), BlockFace::North);
        assert_eq!(yaw_to_horizontal_face(270.0), BlockFace::East);
    }
}
