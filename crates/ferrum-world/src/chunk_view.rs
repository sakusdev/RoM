use crate::ChunkPos;
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkView {
    center: ChunkPos,
    radius: i32,
    loaded: BTreeSet<ChunkPos>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkViewDelta {
    pub center_changed: bool,
    pub newly_visible: Vec<ChunkPos>,
    pub no_longer_visible: Vec<ChunkPos>,
}

impl ChunkView {
    pub fn new(center: ChunkPos, radius: i32) -> Result<Self, ChunkViewError> {
        if radius < 0 {
            return Err(ChunkViewError::NegativeRadius(radius));
        }
        Ok(Self {
            center,
            radius,
            loaded: BTreeSet::new(),
        })
    }

    #[must_use]
    pub const fn center(&self) -> ChunkPos {
        self.center
    }

    #[must_use]
    pub const fn radius(&self) -> i32 {
        self.radius
    }

    #[must_use]
    pub fn loaded(&self) -> &BTreeSet<ChunkPos> {
        &self.loaded
    }

    pub fn mark_loaded(&mut self, pos: ChunkPos) {
        self.loaded.insert(pos);
    }

    pub fn synchronize(&mut self) -> Result<ChunkViewDelta, ChunkViewError> {
        self.reconcile(false)
    }

    pub fn recenter(&mut self, center: ChunkPos) -> Result<ChunkViewDelta, ChunkViewError> {
        let center_changed = center != self.center;
        self.center = center;
        self.reconcile(center_changed)
    }

    fn reconcile(&mut self, center_changed: bool) -> Result<ChunkViewDelta, ChunkViewError> {
        let target = target_chunks(self.center, self.radius)?;
        let newly_visible = target.difference(&self.loaded).copied().collect::<Vec<_>>();
        let no_longer_visible = self.loaded.difference(&target).copied().collect::<Vec<_>>();
        self.loaded = target;
        Ok(ChunkViewDelta {
            center_changed,
            newly_visible,
            no_longer_visible,
        })
    }
}

fn target_chunks(center: ChunkPos, radius: i32) -> Result<BTreeSet<ChunkPos>, ChunkViewError> {
    let min_x = center
        .x
        .checked_sub(radius)
        .ok_or(ChunkViewError::CoordinateOverflow)?;
    let max_x = center
        .x
        .checked_add(radius)
        .ok_or(ChunkViewError::CoordinateOverflow)?;
    let min_z = center
        .z
        .checked_sub(radius)
        .ok_or(ChunkViewError::CoordinateOverflow)?;
    let max_z = center
        .z
        .checked_add(radius)
        .ok_or(ChunkViewError::CoordinateOverflow)?;

    let mut chunks = BTreeSet::new();
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            chunks.insert(ChunkPos { x, z });
        }
    }
    Ok(chunks)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChunkViewError {
    #[error("chunk view radius must not be negative: {0}")]
    NegativeRadius(i32),
    #[error("chunk view coordinate arithmetic overflowed")]
    CoordinateOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_three_by_three_view_is_deterministic() {
        let mut view = ChunkView::new(ChunkPos { x: 0, z: 0 }, 1).unwrap();
        let delta = view.synchronize().unwrap();
        assert!(!delta.center_changed);
        assert_eq!(delta.newly_visible.len(), 9);
        assert!(delta.no_longer_visible.is_empty());
        assert_eq!(delta.newly_visible.first(), Some(&ChunkPos { x: -1, z: -1 }));
        assert_eq!(delta.newly_visible.last(), Some(&ChunkPos { x: 1, z: 1 }));
    }

    #[test]
    fn crossing_one_chunk_boundary_loads_and_unloads_only_one_column() {
        let mut view = ChunkView::new(ChunkPos { x: 0, z: 0 }, 1).unwrap();
        view.synchronize().unwrap();
        let delta = view.recenter(ChunkPos { x: 1, z: 0 }).unwrap();
        assert!(delta.center_changed);
        assert_eq!(
            delta.newly_visible,
            vec![
                ChunkPos { x: 2, z: -1 },
                ChunkPos { x: 2, z: 0 },
                ChunkPos { x: 2, z: 1 },
            ]
        );
        assert_eq!(
            delta.no_longer_visible,
            vec![
                ChunkPos { x: -1, z: -1 },
                ChunkPos { x: -1, z: 0 },
                ChunkPos { x: -1, z: 1 },
            ]
        );
    }

    #[test]
    fn staying_in_the_same_chunk_produces_no_delta() {
        let mut view = ChunkView::new(ChunkPos { x: -2, z: 4 }, 2).unwrap();
        view.synchronize().unwrap();
        let delta = view.recenter(ChunkPos { x: -2, z: 4 }).unwrap();
        assert!(!delta.center_changed);
        assert!(delta.newly_visible.is_empty());
        assert!(delta.no_longer_visible.is_empty());
    }

    #[test]
    fn can_seed_a_chunk_that_was_already_sent() {
        let mut view = ChunkView::new(ChunkPos { x: 0, z: 0 }, 1).unwrap();
        view.mark_loaded(ChunkPos { x: 0, z: 0 });
        let delta = view.synchronize().unwrap();
        assert_eq!(delta.newly_visible.len(), 8);
        assert!(!delta.newly_visible.contains(&ChunkPos { x: 0, z: 0 }));
    }

    #[test]
    fn rejects_negative_radius_and_coordinate_overflow() {
        assert_eq!(
            ChunkView::new(ChunkPos { x: 0, z: 0 }, -1).unwrap_err(),
            ChunkViewError::NegativeRadius(-1)
        );
        let mut view = ChunkView::new(ChunkPos { x: i32::MAX, z: 0 }, 1).unwrap();
        assert_eq!(
            view.synchronize().unwrap_err(),
            ChunkViewError::CoordinateOverflow
        );
    }
}
