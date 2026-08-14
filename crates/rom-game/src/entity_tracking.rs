//! Entity tracking sets for per-player replication.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{EntityId, EntityStore};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub horizontal_range: f64,
    pub vertical_range: f64,
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            horizontal_range: 64.0,
            vertical_range: 64.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackingDelta {
    pub entered: Vec<EntityId>,
    pub remained: Vec<EntityId>,
    pub left: Vec<EntityId>,
}

#[derive(Debug, Clone, Default)]
pub struct EntityTrackingState {
    tracked: BTreeMap<EntityId, BTreeSet<EntityId>>,
}

impl EntityTrackingState {
    pub fn update_observer(
        &mut self,
        observer: EntityId,
        store: &EntityStore,
        config: TrackingConfig,
    ) -> TrackingDelta {
        let visible = visible_entities(observer, store, config);
        let previous = self.tracked.entry(observer).or_default();
        let entered = visible.difference(previous).copied().collect::<Vec<_>>();
        let remained = visible.intersection(previous).copied().collect::<Vec<_>>();
        let left = previous.difference(&visible).copied().collect::<Vec<_>>();
        *previous = visible;
        TrackingDelta {
            entered,
            remained,
            left,
        }
    }

    pub fn remove_observer(&mut self, observer: EntityId) -> Vec<EntityId> {
        self.tracked
            .remove(&observer)
            .map_or_else(Vec::new, |set| set.into_iter().collect())
    }

    pub fn forget_entity(&mut self, entity: EntityId) {
        self.tracked.remove(&entity);
        for tracked in self.tracked.values_mut() {
            tracked.remove(&entity);
        }
    }

    #[must_use]
    pub fn tracked_by(&self, observer: EntityId) -> Option<&BTreeSet<EntityId>> {
        self.tracked.get(&observer)
    }

    pub fn clear(&mut self) {
        self.tracked.clear();
    }
}

#[must_use]
pub fn visible_entities(
    observer: EntityId,
    store: &EntityStore,
    config: TrackingConfig,
) -> BTreeSet<EntityId> {
    let Some(observer_entity) = store.get(observer) else {
        return BTreeSet::new();
    };
    if !config.horizontal_range.is_finite()
        || !config.vertical_range.is_finite()
        || config.horizontal_range < 0.0
        || config.vertical_range < 0.0
    {
        return BTreeSet::new();
    }
    let [ox, oy, oz] = observer_entity.transform.position;
    let horizontal_squared = config.horizontal_range * config.horizontal_range;
    store
        .iter()
        .filter_map(|(id, entity)| {
            if *id == observer {
                return None;
            }
            let [x, y, z] = entity.transform.position;
            let dx = x - ox;
            let dz = z - oz;
            let dy = (y - oy).abs();
            (dx * dx + dz * dz <= horizontal_squared && dy <= config.vertical_range).then_some(*id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntityStore, EntityType, EntityUuid, Transform};

    fn transform(position: [f64; 3]) -> Transform {
        Transform::new(position, 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn tracks_enter_and_leave() {
        let mut store = EntityStore::new();
        let observer = store
            .spawn(
                EntityUuid::new(1),
                EntityType::new("minecraft:player").unwrap(),
                transform([0.0; 3]),
            )
            .unwrap();
        let target = store
            .spawn(
                EntityUuid::new(2),
                EntityType::new("minecraft:item").unwrap(),
                transform([2.0, 0.0, 0.0]),
            )
            .unwrap();
        let mut tracking = EntityTrackingState::default();
        let first = tracking.update_observer(observer, &store, TrackingConfig::default());
        assert_eq!(first.entered, vec![target]);
        store
            .set_transform(target, transform([100.0, 0.0, 0.0]))
            .unwrap();
        let second = tracking.update_observer(observer, &store, TrackingConfig::default());
        assert_eq!(second.left, vec![target]);
    }

    #[test]
    fn vertical_range_is_bounded_separately() {
        let mut store = EntityStore::new();
        let observer = store
            .spawn(
                EntityUuid::new(3),
                EntityType::new("minecraft:player").unwrap(),
                transform([0.0; 3]),
            )
            .unwrap();
        let target = store
            .spawn(
                EntityUuid::new(4),
                EntityType::new("minecraft:item").unwrap(),
                transform([0.0, 80.0, 0.0]),
            )
            .unwrap();
        assert!(!visible_entities(observer, &store, TrackingConfig::default()).contains(&target));
    }
}
