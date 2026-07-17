use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{AttributeSet, ItemStack, PlayerUuid, StatusEffectSet, validate_resource_location};

pub const MAX_ENTITY_COORDINATE: f64 = 30_000_000.0;
pub const MAX_ENTITY_VELOCITY: f64 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(u32);

impl EntityId {
    pub fn new(value: u32) -> Result<Self, EntityError> {
        if value == 0 {
            return Err(EntityError::ZeroEntityId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityUuid([u8; 16]);

impl EntityUuid {
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value.to_be_bytes())
    }

    #[must_use]
    pub const fn from_bytes(value: [u8; 16]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u128 {
        u128::from_be_bytes(self.0)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityType(String);

impl EntityType {
    pub fn new(value: impl Into<String>) -> Result<Self, EntityError> {
        let value = value.into();
        if !validate_resource_location(&value) {
            return Err(EntityError::InvalidEntityType { value });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub position: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl Transform {
    pub fn new(
        position: [f64; 3],
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) -> Result<Self, EntityError> {
        validate_position(position)?;
        if !yaw.is_finite() {
            return Err(EntityError::NonFiniteRotation { field: "yaw" });
        }
        if !pitch.is_finite() {
            return Err(EntityError::NonFiniteRotation { field: "pitch" });
        }
        Ok(Self {
            position,
            yaw,
            pitch,
            on_ground,
        })
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Velocity(pub [f64; 3]);

impl Velocity {
    pub fn new(value: [f64; 3]) -> Result<Self, EntityError> {
        for (axis, component) in ["x", "y", "z"].into_iter().zip(value) {
            if !component.is_finite() {
                return Err(EntityError::NonFiniteVelocity { axis });
            }
            if component.abs() > MAX_ENTITY_VELOCITY {
                return Err(EntityError::VelocityOutOfRange {
                    axis,
                    value: component,
                });
            }
        }
        Ok(Self(value))
    }
}

impl Default for Velocity {
    fn default() -> Self {
        Self([0.0, 0.0, 0.0])
    }
}

pub const DEFAULT_ITEM_PICKUP_DELAY_TICKS: u32 = 10;
pub const DEFAULT_ITEM_DESPAWN_TICKS: u64 = 20 * 60 * 5;
pub const MAX_EXPERIENCE_ORB_VALUE: u32 = 32_767;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemEntityData {
    pub stack: ItemStack,
    pub pickup_delay_ticks: u32,
    pub despawn_after_ticks: u64,
    pub owner: Option<PlayerUuid>,
}

impl ItemEntityData {
    #[must_use]
    pub fn new(stack: ItemStack) -> Self {
        Self {
            stack,
            pickup_delay_ticks: DEFAULT_ITEM_PICKUP_DELAY_TICKS,
            despawn_after_ticks: DEFAULT_ITEM_DESPAWN_TICKS,
            owner: None,
        }
    }

    #[must_use]
    pub const fn can_pick_up(&self) -> bool {
        self.pickup_delay_ticks == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LivingEntityData {
    pub health: f32,
    pub max_health: f32,
    #[serde(default)]
    pub attributes: AttributeSet,
    #[serde(default)]
    pub status_effects: StatusEffectSet,
}

impl LivingEntityData {
    pub fn new(max_health: f32) -> Result<Self, EntityError> {
        if !max_health.is_finite() || max_health <= 0.0 {
            return Err(EntityError::InvalidLivingHealth { health: max_health });
        }
        Ok(Self {
            health: max_health,
            max_health,
            attributes: AttributeSet::default(),
            status_effects: StatusEffectSet::default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum EntityPayload {
    #[default]
    Generic,
    Item(ItemEntityData),
    Living(LivingEntityData),
    ExperienceOrb {
        value: u32,
    },
    Projectile {
        owner: Option<EntityUuid>,
        gravity: f64,
    },
    Vehicle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub uuid: EntityUuid,
    pub entity_type: EntityType,
    pub transform: Transform,
    pub velocity: Velocity,
    pub age_ticks: u64,
    #[serde(default)]
    pub payload: EntityPayload,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, Value>,
}

impl Entity {
    #[must_use]
    pub fn is_player(&self) -> bool {
        self.entity_type.as_str() == "minecraft:player"
    }

    #[must_use]
    pub fn item(&self) -> Option<&ItemEntityData> {
        match &self.payload {
            EntityPayload::Item(item) => Some(item),
            _ => None,
        }
    }

    pub fn item_mut(&mut self) -> Option<&mut ItemEntityData> {
        match &mut self.payload {
            EntityPayload::Item(item) => Some(item),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_living(&self) -> bool {
        self.is_player() || matches!(self.payload, EntityPayload::Living(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityStore {
    next_id: u32,
    entities: BTreeMap<EntityId, Entity>,
    uuids: BTreeMap<EntityUuid, EntityId>,
}

impl Default for EntityStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            entities: BTreeMap::new(),
            uuids: BTreeMap::new(),
        }
    }

    pub fn spawn(
        &mut self,
        uuid: EntityUuid,
        entity_type: EntityType,
        transform: Transform,
    ) -> Result<EntityId, EntityError> {
        self.spawn_with_payload(uuid, entity_type, transform, EntityPayload::Generic)
    }

    pub fn spawn_generated(
        &mut self,
        entity_type: EntityType,
        transform: Transform,
        payload: EntityPayload,
    ) -> Result<EntityId, EntityError> {
        const GENERATED_UUID_PREFIX: u128 = 0xf3_72_6f_6d_00_00_00_00_00_00_00_00_00_00_00_00;
        let mut sequence = u128::from(self.next_id);
        loop {
            let uuid = EntityUuid::new(GENERATED_UUID_PREFIX | sequence);
            if !self.uuids.contains_key(&uuid) {
                return self.spawn_with_payload(uuid, entity_type, transform, payload);
            }
            sequence = sequence
                .checked_add(1)
                .ok_or(EntityError::EntityUuidExhausted)?;
        }
    }

    pub fn spawn_with_payload(
        &mut self,
        uuid: EntityUuid,
        entity_type: EntityType,
        transform: Transform,
        payload: EntityPayload,
    ) -> Result<EntityId, EntityError> {
        validate_payload(&payload)?;
        if self.uuids.contains_key(&uuid) {
            return Err(EntityError::DuplicateEntityUuid { uuid });
        }
        let id = EntityId::new(self.next_id)?;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(EntityError::EntityIdExhausted)?;
        let entity = Entity {
            id,
            uuid,
            entity_type,
            transform,
            velocity: Velocity::default(),
            age_ticks: 0,
            payload,
            data: BTreeMap::new(),
        };
        self.entities.insert(id, entity);
        self.uuids.insert(uuid, id);
        Ok(id)
    }

    pub fn insert_restored(&mut self, entity: Entity) -> Result<(), EntityError> {
        validate_position(entity.transform.position)?;
        Velocity::new(entity.velocity.0)?;
        validate_payload(&entity.payload)?;
        if self.entities.contains_key(&entity.id) {
            return Err(EntityError::DuplicateEntityId { id: entity.id });
        }
        if self.uuids.contains_key(&entity.uuid) {
            return Err(EntityError::DuplicateEntityUuid { uuid: entity.uuid });
        }
        self.next_id = self.next_id.max(
            entity
                .id
                .get()
                .checked_add(1)
                .ok_or(EntityError::EntityIdExhausted)?,
        );
        self.uuids.insert(entity.uuid, entity.id);
        self.entities.insert(entity.id, entity);
        Ok(())
    }

    pub fn despawn(&mut self, id: EntityId) -> Option<Entity> {
        let entity = self.entities.remove(&id)?;
        self.uuids.remove(&entity.uuid);
        Some(entity)
    }

    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    #[must_use]
    pub fn id_by_uuid(&self, uuid: EntityUuid) -> Option<EntityId> {
        self.uuids.get(&uuid).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&EntityId, &Entity)> {
        self.entities.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn set_transform(
        &mut self,
        id: EntityId,
        transform: Transform,
    ) -> Result<Transform, EntityError> {
        let entity = self
            .entities
            .get_mut(&id)
            .ok_or(EntityError::UnknownEntity { id })?;
        Ok(std::mem::replace(&mut entity.transform, transform))
    }

    pub fn set_velocity(
        &mut self,
        id: EntityId,
        velocity: Velocity,
    ) -> Result<Velocity, EntityError> {
        let entity = self
            .entities
            .get_mut(&id)
            .ok_or(EntityError::UnknownEntity { id })?;
        Ok(std::mem::replace(&mut entity.velocity, velocity))
    }

    pub fn tick(&mut self) -> Vec<EntityId> {
        let mut expired = Vec::new();
        for entity in self.entities.values_mut() {
            entity.age_ticks = entity.age_ticks.saturating_add(1);
            match &mut entity.payload {
                EntityPayload::Item(item) => {
                    item.pickup_delay_ticks = item.pickup_delay_ticks.saturating_sub(1);
                    if entity.age_ticks >= item.despawn_after_ticks {
                        expired.push(entity.id);
                        continue;
                    }
                    if !entity.transform.on_ground {
                        entity.velocity.0[1] = (entity.velocity.0[1] - 0.04).max(-3.9);
                    }
                    for axis in 0..3 {
                        entity.transform.position[axis] += entity.velocity.0[axis];
                    }
                    entity.velocity.0[0] *= 0.98;
                    entity.velocity.0[1] *= 0.98;
                    entity.velocity.0[2] *= 0.98;
                    if entity.transform.on_ground {
                        entity.velocity.0[0] *= 0.7;
                        entity.velocity.0[2] *= 0.7;
                    }
                }
                EntityPayload::Living(living) => {
                    let _ = living.status_effects.tick();
                }
                EntityPayload::Projectile { gravity, .. } => {
                    entity.velocity.0[1] = (entity.velocity.0[1] - *gravity).max(-3.9);
                    for axis in 0..3 {
                        entity.transform.position[axis] += entity.velocity.0[axis];
                    }
                }
                EntityPayload::Generic
                | EntityPayload::ExperienceOrb { .. }
                | EntityPayload::Vehicle => {}
            }
        }
        for id in &expired {
            self.despawn(*id);
        }
        expired
    }
}

fn validate_payload(payload: &EntityPayload) -> Result<(), EntityError> {
    match payload {
        EntityPayload::Item(item) => {
            if item.despawn_after_ticks == 0 {
                return Err(EntityError::InvalidItemDespawnTicks {
                    ticks: item.despawn_after_ticks,
                });
            }
        }
        EntityPayload::Living(living) => {
            if !living.health.is_finite()
                || !living.max_health.is_finite()
                || living.max_health <= 0.0
                || living.health < 0.0
                || living.health > living.max_health
            {
                return Err(EntityError::InvalidLivingHealth {
                    health: living.health,
                });
            }
        }
        EntityPayload::ExperienceOrb { value } if *value > MAX_EXPERIENCE_ORB_VALUE => {
            return Err(EntityError::ExperienceOrbValueOutOfRange { value: *value });
        }
        EntityPayload::Projectile { gravity, .. }
            if !gravity.is_finite() || !(0.0..=1.0).contains(gravity) =>
        {
            return Err(EntityError::InvalidProjectileGravity { gravity: *gravity });
        }
        _ => {}
    }
    Ok(())
}

fn validate_position(position: [f64; 3]) -> Result<(), EntityError> {
    for (axis, value) in ["x", "y", "z"].into_iter().zip(position) {
        if !value.is_finite() {
            return Err(EntityError::NonFinitePosition { axis });
        }
        if value.abs() > MAX_ENTITY_COORDINATE {
            return Err(EntityError::PositionOutOfRange { axis, value });
        }
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq)]
pub enum EntityError {
    #[error("entity id zero is reserved")]
    ZeroEntityId,
    #[error("entity id space is exhausted")]
    EntityIdExhausted,
    #[error("generated entity UUID space is exhausted")]
    EntityUuidExhausted,
    #[error("invalid entity resource location {value}")]
    InvalidEntityType { value: String },
    #[error("entity UUID {uuid:?} already exists")]
    DuplicateEntityUuid { uuid: EntityUuid },
    #[error("entity id {id:?} already exists")]
    DuplicateEntityId { id: EntityId },
    #[error("unknown entity {id:?}")]
    UnknownEntity { id: EntityId },
    #[error("entity {axis} position is not finite")]
    NonFinitePosition { axis: &'static str },
    #[error("entity {axis} position {value} exceeds world coordinate bounds")]
    PositionOutOfRange { axis: &'static str, value: f64 },
    #[error("entity {field} rotation is not finite")]
    NonFiniteRotation { field: &'static str },
    #[error("entity {axis} velocity is not finite")]
    NonFiniteVelocity { axis: &'static str },
    #[error("entity {axis} velocity {value} exceeds {MAX_ENTITY_VELOCITY}")]
    VelocityOutOfRange { axis: &'static str, value: f64 },
    #[error("item entity despawn ticks {ticks} must be greater than zero")]
    InvalidItemDespawnTicks { ticks: u64 },
    #[error("living entity health value {health} is invalid")]
    InvalidLivingHealth { health: f32 },
    #[error("experience orb value {value} exceeds {MAX_EXPERIENCE_ORB_VALUE}")]
    ExperienceOrbValueOutOfRange { value: u32 },
    #[error("projectile gravity {gravity} must be finite and between 0 and 1")]
    InvalidProjectileGravity { gravity: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player_type() -> EntityType {
        EntityType::new("minecraft:player").unwrap()
    }

    #[test]
    fn allocates_deterministic_entity_ids() {
        let mut store = EntityStore::new();
        let first = store
            .spawn(EntityUuid::new(10), player_type(), Transform::default())
            .unwrap();
        let second = store
            .spawn(
                EntityUuid::new(11),
                EntityType::new("minecraft:item").unwrap(),
                Transform::default(),
            )
            .unwrap();
        assert_eq!(first.get(), 1);
        assert_eq!(second.get(), 2);
        assert_eq!(
            store.iter().map(|(id, _)| id.get()).collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn rejects_duplicate_uuids_and_invalid_coordinates() {
        let mut store = EntityStore::new();
        store
            .spawn(EntityUuid::new(10), player_type(), Transform::default())
            .unwrap();
        assert!(matches!(
            store.spawn(EntityUuid::new(10), player_type(), Transform::default()),
            Err(EntityError::DuplicateEntityUuid { .. })
        ));
        assert!(Transform::new([f64::NAN, 0.0, 0.0], 0.0, 0.0, false).is_err());
        assert!(Transform::new([MAX_ENTITY_COORDINATE + 1.0, 0.0, 0.0], 0.0, 0.0, false).is_err());
    }

    #[test]
    fn despawning_releases_uuid_and_ticking_ages_entities() {
        let mut store = EntityStore::new();
        let id = store
            .spawn(EntityUuid::new(77), player_type(), Transform::default())
            .unwrap();
        store.tick();
        assert_eq!(store.get(id).unwrap().age_ticks, 1);
        let removed = store.despawn(id).unwrap();
        assert_eq!(removed.uuid, EntityUuid::new(77));
        assert_eq!(store.id_by_uuid(EntityUuid::new(77)), None);
        assert!(store.is_empty());
    }

    #[test]
    fn restored_entities_advance_the_next_id() {
        let mut store = EntityStore::new();
        store
            .insert_restored(Entity {
                id: EntityId::new(41).unwrap(),
                uuid: EntityUuid::new(99),
                entity_type: player_type(),
                transform: Transform::default(),
                velocity: Velocity::default(),
                age_ticks: 20,
                payload: EntityPayload::Generic,
                data: BTreeMap::new(),
            })
            .unwrap();
        let next = store
            .spawn(
                EntityUuid::new(100),
                EntityType::new("minecraft:item").unwrap(),
                Transform::default(),
            )
            .unwrap();
        assert_eq!(next.get(), 42);
    }

    #[test]
    fn uuid_json_is_safe_for_full_128_bit_values() {
        let uuid = EntityUuid::new(u128::MAX);
        let json = serde_json::to_string(&uuid).unwrap();
        let decoded: EntityUuid = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, uuid);
        assert_eq!(decoded.as_bytes(), &[0xff; 16]);
    }

    #[test]
    fn default_store_allocates_from_one() {
        let mut store = EntityStore::default();
        let id = store
            .spawn(EntityUuid::new(1), player_type(), Transform::default())
            .unwrap();
        assert_eq!(id.get(), 1);
    }
}
