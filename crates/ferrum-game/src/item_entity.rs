//! Typed item-entity gameplay primitives.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    EntityError, EntityId, EntityStore, EntityType, EntityUuid, Inventory, InventoryError, ItemStack,
    Transform, Velocity,
};

pub const ITEM_ENTITY_DATA_KEY: &str = "rom:item_entity";
pub const DEFAULT_ITEM_PICKUP_DELAY_TICKS: u16 = 10;
pub const ITEM_ENTITY_LIFETIME_TICKS: u64 = 6_000;
pub const ITEM_ENTITY_GRAVITY_PER_TICK: f64 = 0.04;
pub const ITEM_ENTITY_AIR_DRAG: f64 = 0.98;
pub const ITEM_ENTITY_GROUND_DRAG: f64 = 0.588;
pub const ITEM_ENTITY_VERTICAL_GROUND_DRAG: f64 = 0.98;
pub const ITEM_PICKUP_RANGE: f64 = 1.5;
pub const ITEM_MERGE_RANGE: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemEntityData {
    pub stack: ItemStack,
    pub pickup_delay_ticks: u16,
}

impl ItemEntityData {
    #[must_use]
    pub const fn can_be_picked_up(&self) -> bool { self.pickup_delay_ticks == 0 }
    pub fn tick_pickup_delay(&mut self) { self.pickup_delay_ticks = self.pickup_delay_ticks.saturating_sub(1); }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemPickupResult {
    pub entity_id: EntityId,
    pub inserted: u32,
    pub changed_slots: Vec<usize>,
    pub remainder: Option<ItemStack>,
    pub removed_entity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemMergeResult {
    pub survivor: EntityId,
    pub removed: EntityId,
    pub moved: u32,
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
    set_item_entity_data(store, id, ItemEntityData { stack, pickup_delay_ticks })?;
    Ok(id)
}

pub fn spawn_item_entity_with_default_delay(
    store: &mut EntityStore,
    uuid: EntityUuid,
    transform: Transform,
    velocity: Velocity,
    stack: ItemStack,
) -> Result<EntityId, ItemEntityError> {
    spawn_item_entity(store, uuid, transform, velocity, stack, DEFAULT_ITEM_PICKUP_DELAY_TICKS)
}

pub fn item_entity_data(store: &EntityStore, id: EntityId) -> Result<Option<ItemEntityData>, ItemEntityError> {
    let entity = store.get(id).ok_or(ItemEntityError::UnknownEntity { id })?;
    if entity.entity_type.as_str() != "minecraft:item" { return Ok(None); }
    decode_item_entity_data(entity.data.get(ITEM_ENTITY_DATA_KEY), id).map(Some)
}

pub fn set_item_entity_data(store: &mut EntityStore, id: EntityId, data: ItemEntityData) -> Result<(), ItemEntityError> {
    let entity = store.get_mut(id).ok_or(ItemEntityError::UnknownEntity { id })?;
    if entity.entity_type.as_str() != "minecraft:item" { return Err(ItemEntityError::NotItemEntity { id }); }
    entity.data.insert(ITEM_ENTITY_DATA_KEY.to_owned(), serde_json::to_value(data)?);
    Ok(())
}

pub fn item_entity_in_pickup_range(store: &EntityStore, id: EntityId, player_position: [f64; 3]) -> Result<bool, ItemEntityError> {
    let entity = store.get(id).ok_or(ItemEntityError::UnknownEntity { id })?;
    if player_position.into_iter().any(|v| !v.is_finite()) { return Err(ItemEntityError::NonFinitePickupPosition); }
    Ok(distance_squared(entity.transform.position, player_position) <= ITEM_PICKUP_RANGE * ITEM_PICKUP_RANGE)
}

pub fn try_pickup_item_entity(
    store: &mut EntityStore,
    id: EntityId,
    player_position: [f64; 3],
    inventory: &mut Inventory,
) -> Result<Option<ItemPickupResult>, ItemEntityError> {
    if !item_entity_in_pickup_range(store, id, player_position)? { return Ok(None); }
    let Some(data) = item_entity_data(store, id)? else { return Ok(None); };
    if !data.can_be_picked_up() { return Ok(None); }
    let requested = data.stack.count();
    let (remainder, changed_slots) = inventory.insert_with_changed_slots(data.stack);
    let inserted = requested - remainder.as_ref().map_or(0, ItemStack::count);
    if inserted == 0 { return Ok(Some(ItemPickupResult { entity_id:id, inserted:0, changed_slots, remainder, removed_entity:false })); }
    let removed_entity = remainder.is_none();
    if let Some(stack) = remainder.as_ref() {
        set_item_entity_data(store, id, ItemEntityData { stack: stack.clone(), pickup_delay_ticks: 0 })?;
    } else {
        store.despawn(id);
    }
    Ok(Some(ItemPickupResult { entity_id:id, inserted, changed_slots, remainder, removed_entity }))
}

pub fn try_merge_item_entities(store: &mut EntityStore, first: EntityId, second: EntityId) -> Result<Option<ItemMergeResult>, ItemEntityError> {
    if first == second { return Ok(None); }
    let first_entity = store.get(first).ok_or(ItemEntityError::UnknownEntity { id:first })?;
    let second_entity = store.get(second).ok_or(ItemEntityError::UnknownEntity { id:second })?;
    if distance_squared(first_entity.transform.position, second_entity.transform.position) > ITEM_MERGE_RANGE * ITEM_MERGE_RANGE { return Ok(None); }
    let Some(mut a) = item_entity_data(store, first)? else { return Ok(None); };
    let Some(mut b) = item_entity_data(store, second)? else { return Ok(None); };
    if !a.stack.can_merge(&b.stack) || a.stack.remaining_capacity() == 0 { return Ok(None); }
    let moved = a.stack.remaining_capacity().min(b.stack.count());
    if moved == 0 { return Ok(None); }
    let new_a = a.stack.copy_with_count(a.stack.count() + moved)?;
    a.stack = new_a;
    a.pickup_delay_ticks = a.pickup_delay_ticks.max(b.pickup_delay_ticks);
    set_item_entity_data(store, first, a)?;
    if moved == b.stack.count() {
        store.despawn(second);
    } else {
        b.stack = b.stack.copy_with_count(b.stack.count() - moved)?;
        set_item_entity_data(store, second, b)?;
        return Ok(None);
    }
    Ok(Some(ItemMergeResult { survivor:first, removed:second, moved }))
}

pub fn merge_nearby_item_entities(store: &mut EntityStore) -> Result<Vec<ItemMergeResult>, ItemEntityError> {
    let mut ids = store.iter().filter_map(|(id,e)|(e.entity_type.as_str()=="minecraft:item").then_some(*id)).collect::<Vec<_>>();
    ids.sort();
    let mut results=Vec::new();
    for i in 0..ids.len() {
        if store.get(ids[i]).is_none() { continue; }
        for j in (i+1)..ids.len() {
            if store.get(ids[j]).is_none() { continue; }
            if let Some(result)=try_merge_item_entities(store,ids[i],ids[j])? { results.push(result); }
        }
    }
    Ok(results)
}

pub fn tick_item_entities(store: &mut EntityStore) -> Result<Vec<EntityId>, ItemEntityError> {
    let item_ids = store.iter().filter_map(|(id,e)|(e.entity_type.as_str()=="minecraft:item").then_some(*id)).collect::<Vec<_>>();
    let mut expired=Vec::new();
    for id in item_ids {
        let entity=store.get_mut(id).ok_or(ItemEntityError::UnknownEntity{id})?;
        let mut data=decode_item_entity_data(entity.data.get(ITEM_ENTITY_DATA_KEY),id)?;
        data.tick_pickup_delay();
        entity.data.insert(ITEM_ENTITY_DATA_KEY.to_owned(),serde_json::to_value(data)?);
        let [mut vx,mut vy,mut vz]=entity.velocity.0;
        if !entity.transform.on_ground { vy-=ITEM_ENTITY_GRAVITY_PER_TICK; }
        let [x,y,z]=entity.transform.position;
        entity.transform=Transform::new([x+vx,y+vy,z+vz],entity.transform.yaw,entity.transform.pitch,entity.transform.on_ground)?;
        if entity.transform.on_ground { vx*=ITEM_ENTITY_GROUND_DRAG; vz*=ITEM_ENTITY_GROUND_DRAG; vy*=ITEM_ENTITY_VERTICAL_GROUND_DRAG; }
        else { vx*=ITEM_ENTITY_AIR_DRAG; vy*=ITEM_ENTITY_AIR_DRAG; vz*=ITEM_ENTITY_AIR_DRAG; }
        entity.velocity=Velocity::new([vx,vy,vz])?;
        if entity.age_ticks>=ITEM_ENTITY_LIFETIME_TICKS { expired.push(id); }
    }
    for id in &expired { store.despawn(*id); }
    Ok(expired)
}

fn distance_squared(a:[f64;3],b:[f64;3])->f64 { let dx=a[0]-b[0];let dy=a[1]-b[1];let dz=a[2]-b[2];dx*dx+dy*dy+dz*dz }
fn decode_item_entity_data(value:Option<&serde_json::Value>,id:EntityId)->Result<ItemEntityData,ItemEntityError>{
    let value=value.ok_or(ItemEntityError::MissingItemData{id})?;
    serde_json::from_value(value.clone()).map_err(ItemEntityError::Json)
}

#[derive(Debug, Error)]
pub enum ItemEntityError {
    #[error(transparent)] Entity(#[from] EntityError),
    #[error(transparent)] Inventory(#[from] InventoryError),
    #[error(transparent)] Json(#[from] serde_json::Error),
    #[error("unknown entity {id:?}")] UnknownEntity { id: EntityId },
    #[error("entity {id:?} is not an item entity")] NotItemEntity { id: EntityId },
    #[error("item entity {id:?} is missing typed item data")] MissingItemData { id: EntityId },
    #[error("pickup position is not finite")] NonFinitePickupPosition,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transform(position:[f64;3],on_ground:bool)->Transform{Transform::new(position,0.0,0.0,on_ground).unwrap()}
    #[test]fn spawns_typed_item_entity(){let mut s=EntityStore::new();let id=spawn_item_entity_with_default_delay(&mut s,EntityUuid::new(1),transform([1.0,64.0,2.0],false),Velocity::default(),ItemStack::new("minecraft:stone",3).unwrap()).unwrap();assert_eq!(item_entity_data(&s,id).unwrap().unwrap().stack.count(),3);}
    #[test]fn pickup_waits_for_delay(){let mut s=EntityStore::new();let id=spawn_item_entity(&mut s,EntityUuid::new(2),transform([0.0;3],true),Velocity::default(),ItemStack::new("minecraft:stone",3).unwrap(),1).unwrap();let mut inv=Inventory::new();assert!(try_pickup_item_entity(&mut s,id,[0.0;3],&mut inv).unwrap().is_none());tick_item_entities(&mut s).unwrap();assert_eq!(try_pickup_item_entity(&mut s,id,[0.0;3],&mut inv).unwrap().unwrap().inserted,3);assert!(s.get(id).is_none());}
    #[test]fn partial_pickup_leaves_remainder(){let mut s=EntityStore::new();let id=spawn_item_entity(&mut s,EntityUuid::new(3),transform([0.0;3],true),Velocity::default(),ItemStack::new("minecraft:stone",64).unwrap(),0).unwrap();let mut inv=Inventory::new();for slot in 9..=44{inv.set_slot(slot,Some(ItemStack::new("minecraft:dirt",64).unwrap())).unwrap();}inv.set_slot(9,Some(ItemStack::new("minecraft:stone",60).unwrap())).unwrap();let result=try_pickup_item_entity(&mut s,id,[0.0;3],&mut inv).unwrap().unwrap();assert_eq!(result.inserted,4);assert_eq!(result.remainder.unwrap().count(),60);assert!(s.get(id).is_some());}
    #[test]fn nearby_stacks_merge(){let mut s=EntityStore::new();let a=spawn_item_entity(&mut s,EntityUuid::new(4),transform([0.0;3],true),Velocity::default(),ItemStack::new("minecraft:stone",60).unwrap(),0).unwrap();let b=spawn_item_entity(&mut s,EntityUuid::new(5),transform([0.1,0.0,0.0],true),Velocity::default(),ItemStack::new("minecraft:stone",4).unwrap(),0).unwrap();let r=try_merge_item_entities(&mut s,a,b).unwrap().unwrap();assert_eq!(r.moved,4);assert_eq!(item_entity_data(&s,a).unwrap().unwrap().stack.count(),64);assert!(s.get(b).is_none());}
    #[test]fn expires_after_lifetime(){let mut s=EntityStore::new();let id=spawn_item_entity_with_default_delay(&mut s,EntityUuid::new(6),transform([0.0;3],true),Velocity::default(),ItemStack::new("minecraft:cobblestone",1).unwrap()).unwrap();s.get_mut(id).unwrap().age_ticks=ITEM_ENTITY_LIFETIME_TICKS;assert_eq!(tick_item_entities(&mut s).unwrap(),vec![id]);}
}
