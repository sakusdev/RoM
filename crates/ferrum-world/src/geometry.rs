use thiserror::Error;

pub const MAX_VOXEL_SHAPE_BOXES: usize = 64;
const RAY_EPSILON: f64 = 1.0e-9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Result<Self, GeometryError> {
        for axis in 0..3 {
            if !min[axis].is_finite() || !max[axis].is_finite() {
                return Err(GeometryError::NonFiniteCoordinate { axis });
            }
            if min[axis] > max[axis] {
                return Err(GeometryError::InvertedBounds {
                    axis,
                    min: min[axis],
                    max: max[axis],
                });
            }
        }
        Ok(Self { min, max })
    }

    #[must_use]
    pub const fn unit_cube() -> Self {
        Self {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 1.0],
        }
    }

    #[must_use]
    pub fn translated(self, offset: [f64; 3]) -> Self {
        Self {
            min: [
                self.min[0] + offset[0],
                self.min[1] + offset[1],
                self.min[2] + offset[2],
            ],
            max: [
                self.max[0] + offset[0],
                self.max[1] + offset[1],
                self.max[2] + offset[2],
            ],
        }
    }

    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        (0..3).all(|axis| self.max[axis] > other.min[axis] && self.min[axis] < other.max[axis])
    }

    #[must_use]
    pub fn contains(self, point: [f64; 3]) -> bool {
        (0..3).all(|axis| point[axis] >= self.min[axis] && point[axis] <= self.max[axis])
    }

    pub fn ray_intersection(
        self,
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
    ) -> Result<Option<RayHit>, GeometryError> {
        validate_ray(origin, direction, max_distance)?;
        let mut t_min = 0.0;
        let mut t_max = max_distance;
        let mut normal = [0.0; 3];
        for axis in 0..3 {
            if direction[axis].abs() <= RAY_EPSILON {
                if origin[axis] < self.min[axis] || origin[axis] > self.max[axis] {
                    return Ok(None);
                }
                continue;
            }
            let inverse = 1.0 / direction[axis];
            let mut near = (self.min[axis] - origin[axis]) * inverse;
            let mut far = (self.max[axis] - origin[axis]) * inverse;
            let mut axis_normal = [0.0; 3];
            axis_normal[axis] = -inverse.signum();
            if near > far {
                std::mem::swap(&mut near, &mut far);
                axis_normal[axis] = -axis_normal[axis];
            }
            if near > t_min {
                t_min = near;
                normal = axis_normal;
            }
            t_max = t_max.min(far);
            if t_min > t_max {
                return Ok(None);
            }
        }
        if t_min < 0.0 || t_min > max_distance {
            return Ok(None);
        }
        Ok(Some(RayHit {
            distance: t_min,
            position: [
                origin[0] + direction[0] * t_min,
                origin[1] + direction[1] * t_min,
                origin[2] + direction[2] * t_min,
            ],
            normal,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub distance: f64,
    pub position: [f64; 3],
    pub normal: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoxelShape {
    boxes: Vec<Aabb>,
}

impl VoxelShape {
    pub fn new(boxes: Vec<Aabb>) -> Result<Self, GeometryError> {
        if boxes.len() > MAX_VOXEL_SHAPE_BOXES {
            return Err(GeometryError::TooManyBoxes {
                actual: boxes.len(),
                limit: MAX_VOXEL_SHAPE_BOXES,
            });
        }
        Ok(Self { boxes })
    }

    #[must_use]
    pub fn empty() -> Self {
        Self { boxes: Vec::new() }
    }

    #[must_use]
    pub fn full_cube() -> Self {
        Self {
            boxes: vec![Aabb::unit_cube()],
        }
    }

    #[must_use]
    pub fn boxes(&self) -> &[Aabb] {
        &self.boxes
    }

    #[must_use]
    pub fn intersects(&self, bounds: Aabb, block_offset: [f64; 3]) -> bool {
        self.boxes
            .iter()
            .any(|shape| shape.translated(block_offset).intersects(bounds))
    }

    pub fn raycast(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
        block_offset: [f64; 3],
    ) -> Result<Option<RayHit>, GeometryError> {
        let mut best: Option<RayHit> = None;
        for bounds in &self.boxes {
            let Some(hit) = bounds.translated(block_offset).ray_intersection(
                origin,
                direction,
                max_distance,
            )?
            else {
                continue;
            };
            if best.is_none_or(|current| hit.distance < current.distance) {
                best = Some(hit);
            }
        }
        Ok(best)
    }
}

pub fn normalized_direction(from: [f64; 3], to: [f64; 3]) -> Result<[f64; 3], GeometryError> {
    let delta = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if !length.is_finite() || length <= RAY_EPSILON {
        return Err(GeometryError::InvalidRayDirection);
    }
    Ok([delta[0] / length, delta[1] / length, delta[2] / length])
}

fn validate_ray(
    origin: [f64; 3],
    direction: [f64; 3],
    max_distance: f64,
) -> Result<(), GeometryError> {
    if !max_distance.is_finite() || max_distance < 0.0 {
        return Err(GeometryError::InvalidRayDistance { max_distance });
    }
    for axis in 0..3 {
        if !origin[axis].is_finite() {
            return Err(GeometryError::NonFiniteCoordinate { axis });
        }
        if !direction[axis].is_finite() {
            return Err(GeometryError::NonFiniteDirection { axis });
        }
    }
    let length_squared = direction.iter().map(|value| value * value).sum::<f64>();
    if length_squared <= RAY_EPSILON {
        return Err(GeometryError::InvalidRayDirection);
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq)]
pub enum GeometryError {
    #[error("geometry coordinate for axis {axis} is not finite")]
    NonFiniteCoordinate { axis: usize },
    #[error("geometry bounds are inverted on axis {axis}: {min} > {max}")]
    InvertedBounds { axis: usize, min: f64, max: f64 },
    #[error("voxel shape has {actual} boxes; limit is {limit}")]
    TooManyBoxes { actual: usize, limit: usize },
    #[error("ray direction for axis {axis} is not finite")]
    NonFiniteDirection { axis: usize },
    #[error("ray direction must be non-zero")]
    InvalidRayDirection,
    #[error("ray maximum distance {max_distance} must be finite and non-negative")]
    InvalidRayDistance { max_distance: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_cube_raycast_reports_nearest_face() {
        let hit = VoxelShape::full_cube()
            .raycast([-1.0, 0.5, 0.5], [1.0, 0.0, 0.0], 5.0, [0.0, 0.0, 0.0])
            .unwrap()
            .unwrap();
        assert_eq!(hit.distance, 1.0);
        assert_eq!(hit.normal, [-1.0, 0.0, 0.0]);
    }

    #[test]
    fn translated_shapes_collide_in_world_space() {
        let shape = VoxelShape::full_cube();
        let player = Aabb::new([3.2, 1.0, 4.2], [3.8, 2.8, 4.8]).unwrap();
        assert!(shape.intersects(player, [3.0, 1.0, 4.0]));
        assert!(!shape.intersects(player, [5.0, 1.0, 4.0]));
    }
}
