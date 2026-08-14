//! Typed experience-orb entities and merge/pickup helpers.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{EntityError, EntityId, EntityStore, EntityType, EntityUuid, Transform, Velocity};

pub const EXPERIENCE_ORB_DATA_KEY: &str = "rom:experience_orb";
pub const EXPERIENCE_ORB_PICKUP_DELAY_TICKS: u16 = 10;
pub const EXPERIENCE_ORB_LIFETIME_TICKS: u64 = 6_000;
pub const EXPERIENCE_ORB_PICKUP_RANGE: f64 = 1.5;
pub const EXPERIENCE_ORB_MERGE_RANGE: f64 = 1.0;
pub const EXPERIENCE_ORB_GRAVITY_PER_TICK: f64 = 0.03;
pub const EXPERIENCE_ORB_DRAG: f64 = 0.98;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceOrbData {
    pub value: u32,
    pub pickup_delay_ticks: u16,
}

impl ExperienceOrbData {
    pub fn new(value: u32, pickup_delay_ticks: u16) -> Result<Self, ExperienceOrbError> {
        if value == 0 {
            return Err(ExperienceOrbError::ZeroValue);
        }
        Ok(Self {
            value,
            pickup_delay_ticks,
        })
    }
}

pub fn spawn_experience_orb(
    store: &mut EntityStore,
    uuid: EntityUuid,
    transform: Transform,
    velocity: Velocity,
    value: u32,
    pickup_delay_ticks: u16,
) -> Result<EntityId, ExperienceOrbError> {
    let id = store.spawn(
        uuid,
        EntityType::new("minecraft:experience_orb")?,
        transform,
    )?;
    store.set_velocity(id, velocity)?;
    set_experience_orb_data(
        store,
        id,
        ExperienceOrbData::new(value, pickup_delay_ticks)?,
    )?;
    Ok(id)
}

pub fn experience_orb_data(
    store: &EntityStore,
    id: EntityId,
) -> Result<Option<ExperienceOrbData>, ExperienceOrbError> {
    let entity = store
        .get(id)
        .ok_or(ExperienceOrbError::UnknownEntity { id })?;
    if entity.entity_type.as_str() != "minecraft:experience_orb" {
        return Ok(None);
    }
    let value = entity
        .data
        .get(EXPERIENCE_ORB_DATA_KEY)
        .ok_or(ExperienceOrbError::MissingData { id })?;
    Ok(Some(serde_json::from_value(value.clone())?))
}

pub fn set_experience_orb_data(
    store: &mut EntityStore,
    id: EntityId,
    data: ExperienceOrbData,
) -> Result<(), ExperienceOrbError> {
    if data.value == 0 {
        return Err(ExperienceOrbError::ZeroValue);
    }
    let entity = store
        .get_mut(id)
        .ok_or(ExperienceOrbError::UnknownEntity { id })?;
    if entity.entity_type.as_str() != "minecraft:experience_orb" {
        return Err(ExperienceOrbError::NotExperienceOrb { id });
    }
    entity.data.insert(
        EXPERIENCE_ORB_DATA_KEY.to_owned(),
        serde_json::to_value(data)?,
    );
    Ok(())
}

pub fn tick_experience_orbs(store: &mut EntityStore) -> Result<Vec<EntityId>, ExperienceOrbError> {
    let ids = store
        .iter()
        .filter_map(|(id, entity)| {
            (entity.entity_type.as_str() == "minecraft:experience_orb").then_some(*id)
        })
        .collect::<Vec<_>>();
    let mut expired = Vec::new();
    for id in ids {
        let entity = store
            .get_mut(id)
            .ok_or(ExperienceOrbError::UnknownEntity { id })?;
        let value = entity
            .data
            .get(EXPERIENCE_ORB_DATA_KEY)
            .ok_or(ExperienceOrbError::MissingData { id })?;
        let mut data: ExperienceOrbData = serde_json::from_value(value.clone())?;
        data.pickup_delay_ticks = data.pickup_delay_ticks.saturating_sub(1);
        entity.data.insert(
            EXPERIENCE_ORB_DATA_KEY.to_owned(),
            serde_json::to_value(data)?,
        );
        let [mut vx, mut vy, mut vz] = entity.velocity.0;
        if !entity.transform.on_ground {
            vy -= EXPERIENCE_ORB_GRAVITY_PER_TICK;
        }
        let [x, y, z] = entity.transform.position;
        entity.transform = Transform::new(
            [x + vx, y + vy, z + vz],
            entity.transform.yaw,
            entity.transform.pitch,
            entity.transform.on_ground,
        )?;
        vx *= EXPERIENCE_ORB_DRAG;
        vy *= EXPERIENCE_ORB_DRAG;
        vz *= EXPERIENCE_ORB_DRAG;
        entity.velocity = Velocity::new([vx, vy, vz])?;
        if entity.age_ticks >= EXPERIENCE_ORB_LIFETIME_TICKS {
            expired.push(id);
        }
    }
    for id in &expired {
        store.despawn(*id);
    }
    Ok(expired)
}

pub fn try_pickup_experience_orb(
    store: &mut EntityStore,
    id: EntityId,
    player_position: [f64; 3],
) -> Result<Option<u32>, ExperienceOrbError> {
    if player_position.into_iter().any(|value| !value.is_finite()) {
        return Err(ExperienceOrbError::NonFinitePickupPosition);
    }
    let entity = store
        .get(id)
        .ok_or(ExperienceOrbError::UnknownEntity { id })?;
    if entity.entity_type.as_str() != "minecraft:experience_orb" {
        return Ok(None);
    }
    if distance_squared(entity.transform.position, player_position)
        > EXPERIENCE_ORB_PICKUP_RANGE * EXPERIENCE_ORB_PICKUP_RANGE
    {
        return Ok(None);
    }
    let Some(data) = experience_orb_data(store, id)? else {
        return Ok(None);
    };
    if data.pickup_delay_ticks > 0 {
        return Ok(None);
    }
    store.despawn(id);
    Ok(Some(data.value))
}

pub fn merge_experience_orbs(store: &mut EntityStore) -> Result<usize, ExperienceOrbError> {
    let mut ids = store
        .iter()
        .filter_map(|(id, entity)| {
            (entity.entity_type.as_str() == "minecraft:experience_orb").then_some(*id)
        })
        .collect::<Vec<_>>();
    ids.sort();
    let mut merged = 0usize;
    for i in 0..ids.len() {
        if store.get(ids[i]).is_none() {
            continue;
        }
        for j in (i + 1)..ids.len() {
            let Some(first) = store.get(ids[i]) else {
                break;
            };
            let Some(second) = store.get(ids[j]) else {
                continue;
            };
            if distance_squared(first.transform.position, second.transform.position)
                > EXPERIENCE_ORB_MERGE_RANGE * EXPERIENCE_ORB_MERGE_RANGE
            {
                continue;
            }
            let Some(mut a) = experience_orb_data(store, ids[i])? else {
                continue;
            };
            let Some(b) = experience_orb_data(store, ids[j])? else {
                continue;
            };
            a.value = a.value.saturating_add(b.value);
            a.pickup_delay_ticks = a.pickup_delay_ticks.max(b.pickup_delay_ticks);
            set_experience_orb_data(store, ids[i], a)?;
            store.despawn(ids[j]);
            merged += 1;
        }
    }
    Ok(merged)
}

#[must_use]
pub fn split_experience_value(mut total: u32) -> Vec<u32> {
    const VALUES: [u32; 11] = [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3, 1];
    let mut out = Vec::new();
    while total > 0 {
        let value = VALUES
            .into_iter()
            .find(|value| *value <= total)
            .unwrap_or(1);
        out.push(value);
        total -= value;
    }
    out
}

fn distance_squared(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

#[derive(Debug, Error)]
pub enum ExperienceOrbError {
    #[error(transparent)]
    Entity(#[from] EntityError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("experience orb value must be non-zero")]
    ZeroValue,
    #[error("unknown entity {id:?}")]
    UnknownEntity { id: EntityId },
    #[error("entity {id:?} is not an experience orb")]
    NotExperienceOrb { id: EntityId },
    #[error("experience orb {id:?} is missing typed data")]
    MissingData { id: EntityId },
    #[error("experience orb pickup position is not finite")]
    NonFinitePickupPosition,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform(position: [f64; 3]) -> Transform {
        Transform::new(position, 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn pickup_returns_value_after_delay() {
        let mut store = EntityStore::new();
        let id = spawn_experience_orb(
            &mut store,
            EntityUuid::new(1),
            transform([0.0; 3]),
            Velocity::default(),
            7,
            0,
        )
        .unwrap();
        assert_eq!(
            try_pickup_experience_orb(&mut store, id, [0.0; 3]).unwrap(),
            Some(7)
        );
        assert!(store.get(id).is_none());
    }

    #[test]
    fn nearby_orbs_merge_values() {
        let mut store = EntityStore::new();
        let a = spawn_experience_orb(
            &mut store,
            EntityUuid::new(2),
            transform([0.0; 3]),
            Velocity::default(),
            3,
            0,
        )
        .unwrap();
        spawn_experience_orb(
            &mut store,
            EntityUuid::new(3),
            transform([0.2, 0.0, 0.0]),
            Velocity::default(),
            7,
            0,
        )
        .unwrap();
        assert_eq!(merge_experience_orbs(&mut store).unwrap(), 1);
        assert_eq!(experience_orb_data(&store, a).unwrap().unwrap().value, 10);
    }

    #[test]
    fn split_values_sum_to_total() {
        let values = split_experience_value(5_000);
        assert_eq!(values.iter().sum::<u32>(), 5_000);
    }
}
