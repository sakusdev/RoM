//! Typed item-entity gameplay primitives.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    EntityError, EntityId, EntityStore, EntityType, EntityUuid, ItemStack, Transform, Velocity,
};

pub const ITEM_ENTITY_DATA_KEY: &str = "rom:item_entity";
pub const DEFAULT_ITEM_PICKUP_DELAY_TICKS: u16 = 10;
pub const ITEM_ENTITY_LIFETIME_TICKS: u64 = 6_000;
pub const ITEM_ENTITY_GRAVITY_PER_TICK: f64 = 0.04;
pub const ITEM_ENTITY_AIR_DRAG: f64 = 0.98;
pub const ITEM_ENTITY_GROUND_DRAG: f64 = 0.588;
pub const ITEM_ENTITY_VERTICAL_GROUND_DRAG: f64 = 0.98;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemEntityData {
    pub stack: ItemStack,
    pub pickup_delay_ticks: u16,
}

impl ItemEntityData {
    #[must_use]
    pub const fn can_be_picked_up(&self) -> bool {
        self.pickup_delay_ticks == 0
    }

    pub fn tick_pickup_delay(&mut self) {
        self.pickup_delay_ticks = self.pickup_delay_ticks.saturating_sub(1);
    }
}

pub fn spawn_item_entity(
    store: &mut EntityStore,
    uuid: EntityUuid,
    transform: Transform,
    velocity: Velocity,
    stack: ItemStack,
    pickup_delay_ticks: u16,
) -> Result<EntityId, ItemEntityError> {
    let id = store.spawn(uuid, EntityType::new("minecraft:item")?, transform)?;
    store.set_velocity(id, velocity)?;
    let entity = store
        .get_mut(id)
        .ok_or(ItemEntityError::SpawnedEntityMissing { id })?;
    let value = serde_json::to_value(ItemEntityData {
        stack,
        pickup_delay_ticks,
    })?;
    entity.data.insert(ITEM_ENTITY_DATA_KEY.to_owned(), value);
    Ok(id)
}

pub fn spawn_item_entity_with_default_delay(
    store: &mut EntityStore,
    uuid: EntityUuid,
    transform: Transform,
    velocity: Velocity,
    stack: ItemStack,
) -> Result<EntityId, ItemEntityError> {
    spawn_item_entity(
        store,
        uuid,
        transform,
        velocity,
        stack,
        DEFAULT_ITEM_PICKUP_DELAY_TICKS,
    )
}

pub fn item_entity_data(
    store: &EntityStore,
    id: EntityId,
) -> Result<Option<ItemEntityData>, ItemEntityError> {
    let entity = store
        .get(id)
        .ok_or(ItemEntityError::UnknownEntity { id })?;
    if entity.entity_type.as_str() != "minecraft:item" {
        return Ok(None);
    }
    decode_item_entity_data(entity.data.get(ITEM_ENTITY_DATA_KEY), id).map(Some)
}

pub fn tick_item_entities(store: &mut EntityStore) -> Result<Vec<EntityId>, ItemEntityError> {
    let item_ids = store
        .iter()
        .filter_map(|(id, entity)| {
            (entity.entity_type.as_str() == "minecraft:item").then_some(*id)
        })
        .collect::<Vec<_>>();

    let mut expired = Vec::new();
    for id in item_ids {
        let entity = store
            .get_mut(id)
            .ok_or(ItemEntityError::UnknownEntity { id })?;
        let mut data = decode_item_entity_data(entity.data.get(ITEM_ENTITY_DATA_KEY), id)?;
        data.tick_pickup_delay();
        entity.data.insert(
            ITEM_ENTITY_DATA_KEY.to_owned(),
            serde_json::to_value(data)?,
        );

        let [mut vx, mut vy, mut vz] = entity.velocity.0;
        if !entity.transform.on_ground {
            vy -= ITEM_ENTITY_GRAVITY_PER_TICK;
        }

        let [x, y, z] = entity.transform.position;
        entity.transform = Transform::new(
            [x + vx, y + vy, z + vz],
            entity.transform.yaw,
            entity.transform.pitch,
            entity.transform.on_ground,
        )?;

        if entity.transform.on_ground {
            vx *= ITEM_ENTITY_GROUND_DRAG;
            vz *= ITEM_ENTITY_GROUND_DRAG;
            vy *= ITEM_ENTITY_VERTICAL_GROUND_DRAG;
        } else {
            vx *= ITEM_ENTITY_AIR_DRAG;
            vy *= ITEM_ENTITY_AIR_DRAG;
            vz *= ITEM_ENTITY_AIR_DRAG;
        }
        entity.velocity = Velocity::new([vx, vy, vz])?;

        if entity.age_ticks >= ITEM_ENTITY_LIFETIME_TICKS {
            expired.push(id);
        }
    }

    for id in &expired {
        store.despawn(*id);
    }
    Ok(expired)
}

fn decode_item_entity_data(
    value: Option<&serde_json::Value>,
    id: EntityId,
) -> Result<ItemEntityData, ItemEntityError> {
    let value = value.ok_or(ItemEntityError::MissingItemData { id })?;
    serde_json::from_value(value.clone()).map_err(ItemEntityError::Json)
}

#[derive(Debug, Error)]
pub enum ItemEntityError {
    #[error(transparent)]
    Entity(#[from] EntityError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("unknown entity {id:?}")]
    UnknownEntity { id: EntityId },
    #[error("spawned item entity {id:?} disappeared before initialization")]
    SpawnedEntityMissing { id: EntityId },
    #[error("item entity {id:?} is missing typed item data")]
    MissingItemData { id: EntityId },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform(on_ground: bool) -> Transform {
        Transform::new([1.0, 64.0, 2.0], 0.0, 0.0, on_ground).unwrap()
    }

    #[test]
    fn spawns_typed_item_entity() {
        let mut store = EntityStore::new();
        let id = spawn_item_entity_with_default_delay(
            &mut store,
            EntityUuid::new(1),
            transform(false),
            Velocity::new([0.1, 0.2, 0.3]).unwrap(),
            ItemStack::new("minecraft:stone", 3).unwrap(),
        )
        .unwrap();

        let entity = store.get(id).unwrap();
        assert_eq!(entity.entity_type.as_str(), "minecraft:item");
        let data = item_entity_data(&store, id).unwrap().unwrap();
        assert_eq!(data.stack.item(), "minecraft:stone");
        assert_eq!(data.stack.count(), 3);
        assert_eq!(data.pickup_delay_ticks, DEFAULT_ITEM_PICKUP_DELAY_TICKS);
    }

    #[test]
    fn ticking_applies_gravity_drag_and_pickup_delay() {
        let mut store = EntityStore::new();
        let id = spawn_item_entity(
            &mut store,
            EntityUuid::new(2),
            transform(false),
            Velocity::new([1.0, 0.0, 0.0]).unwrap(),
            ItemStack::new("minecraft:dirt", 1).unwrap(),
            2,
        )
        .unwrap();

        tick_item_entities(&mut store).unwrap();
        let entity = store.get(id).unwrap();
        assert_eq!(entity.transform.position, [2.0, 63.96, 2.0]);
        assert_eq!(entity.velocity.0, [0.98, -0.0392, 0.0]);
        assert_eq!(
            item_entity_data(&store, id)
                .unwrap()
                .unwrap()
                .pickup_delay_ticks,
            1
        );
    }

    #[test]
    fn expires_items_after_vanilla_lifetime() {
        let mut store = EntityStore::new();
        let id = spawn_item_entity_with_default_delay(
            &mut store,
            EntityUuid::new(3),
            transform(true),
            Velocity::default(),
            ItemStack::new("minecraft:cobblestone", 1).unwrap(),
        )
        .unwrap();
        store.get_mut(id).unwrap().age_ticks = ITEM_ENTITY_LIFETIME_TICKS;

        let expired = tick_item_entities(&mut store).unwrap();
        assert_eq!(expired, vec![id]);
        assert!(store.get(id).is_none());
    }

    #[test]
    fn non_item_entities_are_ignored() {
        let mut store = EntityStore::new();
        let player = store
            .spawn(
                EntityUuid::new(4),
                EntityType::new("minecraft:player").unwrap(),
                transform(true),
            )
            .unwrap();
        tick_item_entities(&mut store).unwrap();
        assert!(store.get(player).is_some());
    }
}
