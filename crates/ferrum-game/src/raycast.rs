//! Grid ray traversal for block targeting and line-of-sight checks.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{BlockFace, BlockPos};

pub const MAX_RAYCAST_STEPS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ray {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
    pub max_distance: f64,
}

impl Ray {
    pub fn new(
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
    ) -> Result<Self, RaycastError> {
        if origin
            .into_iter()
            .chain(direction)
            .any(|value| !value.is_finite())
        {
            return Err(RaycastError::NonFinite);
        }
        if !max_distance.is_finite() || max_distance < 0.0 {
            return Err(RaycastError::InvalidDistance { max_distance });
        }
        let length = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt();
        if length < 1.0e-12 {
            return Err(RaycastError::ZeroDirection);
        }
        Ok(Self {
            origin,
            direction: [
                direction[0] / length,
                direction[1] / length,
                direction[2] / length,
            ],
            max_distance,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastVisit {
    pub block: BlockPos,
    pub entered_face: Option<BlockFace>,
    pub distance: f64,
}

pub fn traverse_voxels(ray: Ray) -> Vec<RaycastVisit> {
    let [ox, oy, oz] = ray.origin;
    let [dx, dy, dz] = ray.direction;
    let mut x = floor_i32(ox);
    let mut y = floor_i32(oy);
    let mut z = floor_i32(oz);
    let step_x = sign_i32(dx);
    let step_y = sign_i32(dy);
    let step_z = sign_i32(dz);
    let delta_x = axis_delta(dx);
    let delta_y = axis_delta(dy);
    let delta_z = axis_delta(dz);
    let mut max_x = axis_initial(ox, x, dx, step_x);
    let mut max_y = axis_initial(oy, y, dy, step_y);
    let mut max_z = axis_initial(oz, z, dz, step_z);
    let mut out = Vec::new();
    out.push(RaycastVisit {
        block: BlockPos { x, y, z },
        entered_face: None,
        distance: 0.0,
    });

    while out.len() < MAX_RAYCAST_STEPS {
        let (distance, face) = if max_x <= max_y && max_x <= max_z {
            x += step_x;
            let distance = max_x;
            max_x += delta_x;
            (
                distance,
                if step_x > 0 {
                    BlockFace::West
                } else {
                    BlockFace::East
                },
            )
        } else if max_y <= max_z {
            y += step_y;
            let distance = max_y;
            max_y += delta_y;
            (
                distance,
                if step_y > 0 {
                    BlockFace::Down
                } else {
                    BlockFace::Up
                },
            )
        } else {
            z += step_z;
            let distance = max_z;
            max_z += delta_z;
            (
                distance,
                if step_z > 0 {
                    BlockFace::North
                } else {
                    BlockFace::South
                },
            )
        };
        if !distance.is_finite() || distance > ray.max_distance {
            break;
        }
        out.push(RaycastVisit {
            block: BlockPos { x, y, z },
            entered_face: Some(face),
            distance,
        });
    }
    out
}

pub fn first_matching<F>(ray: Ray, mut predicate: F) -> Option<RaycastVisit>
where
    F: FnMut(BlockPos) -> bool,
{
    traverse_voxels(ray)
        .into_iter()
        .find(|visit| predicate(visit.block))
}

#[must_use]
pub fn direction_from_rotation(yaw_degrees: f32, pitch_degrees: f32) -> [f64; 3] {
    let yaw = f64::from(yaw_degrees).to_radians();
    let pitch = f64::from(pitch_degrees).to_radians();
    let cos_pitch = pitch.cos();
    [-yaw.sin() * cos_pitch, -pitch.sin(), yaw.cos() * cos_pitch]
}

fn floor_i32(value: f64) -> i32 {
    value
        .floor()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

const fn sign_i32(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn axis_delta(direction: f64) -> f64 {
    if direction == 0.0 {
        f64::INFINITY
    } else {
        (1.0 / direction).abs()
    }
}

fn axis_initial(origin: f64, block: i32, direction: f64, step: i32) -> f64 {
    if step > 0 {
        (f64::from(block + 1) - origin) / direction
    } else if step < 0 {
        (origin - f64::from(block)) / -direction
    } else {
        f64::INFINITY
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum RaycastError {
    #[error("ray values must be finite")]
    NonFinite,
    #[error("ray direction must be non-zero")]
    ZeroDirection,
    #[error("ray distance {max_distance} must be finite and non-negative")]
    InvalidDistance { max_distance: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traverses_positive_x_blocks() {
        let ray = Ray::new([0.5, 64.5, 0.5], [1.0, 0.0, 0.0], 3.0).unwrap();
        let visits = traverse_voxels(ray);
        assert_eq!(visits[0].block, BlockPos { x: 0, y: 64, z: 0 });
        assert_eq!(visits[1].block, BlockPos { x: 1, y: 64, z: 0 });
        assert_eq!(visits[1].entered_face, Some(BlockFace::West));
    }

    #[test]
    fn first_matching_stops_at_target() {
        let ray = Ray::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], 10.0).unwrap();
        let hit = first_matching(ray, |pos| pos.z == 4).unwrap();
        assert_eq!(hit.block.z, 4);
    }

    #[test]
    fn rotation_direction_is_normalized() {
        let direction = direction_from_rotation(0.0, 0.0);
        assert!((direction[2] - 1.0).abs() < 1.0e-12);
    }
}
