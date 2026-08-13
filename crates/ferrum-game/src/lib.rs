//! Version-neutral gameplay state for the native Ferrum server.
//!
//! This crate deliberately owns game concepts rather than wire encodings:
//! players, inventories, entities, commands, game time, and persistence
//! snapshots. Protocol crates translate these values to and from the selected
//! Minecraft version.

pub mod attributes;
pub mod command;
pub mod container;
pub mod entity;
pub mod inventory;
pub mod item_entity;
pub mod persistence;
pub mod player;
pub mod status_effect;
mod state;

pub use attributes::{Attribute, AttributeError, AttributeMap, AttributeModifier, AttributeOperation};
pub use command::{
    CommandError, CommandOutcome, CommandSource, GameCommand, execute_command, parse_command,
};
pub use container::{
    ContainerClick, ContainerClickKind, ContainerError, ContainerMutation, ContainerSnapshot,
    InventorySession, MAX_CONTAINER_SLOTS, OUTSIDE_SLOT, PLAYER_CONTAINER_ID,
};
pub use entity::{
    Entity, EntityError, EntityId, EntityStore, EntityType, EntityUuid, Transform, Velocity,
};
pub use inventory::{
    EquipmentSlot, HOTBAR_END, HOTBAR_SLOTS, HOTBAR_START, Inventory, InventoryError, ItemStack,
    MAIN_INVENTORY_END, MAIN_INVENTORY_START, MAX_VANILLA_STACK_SIZE, OFFHAND_SLOT,
    PLAYER_INVENTORY_SLOTS,
};
pub use item_entity::{
    DEFAULT_ITEM_PICKUP_DELAY_TICKS, ITEM_ENTITY_AIR_DRAG, ITEM_ENTITY_DATA_KEY,
    ITEM_ENTITY_GRAVITY_PER_TICK, ITEM_ENTITY_GROUND_DRAG, ITEM_ENTITY_LIFETIME_TICKS,
    ITEM_ENTITY_VERTICAL_GROUND_DRAG, ITEM_MERGE_RANGE, ITEM_PICKUP_RANGE, ItemEntityData,
    ItemEntityError, ItemMergeResult, ItemPickupResult, item_entity_data,
    item_entity_in_pickup_range, merge_nearby_item_entities, set_item_entity_data,
    spawn_item_entity, spawn_item_entity_with_default_delay, tick_item_entities,
    try_merge_item_entities, try_pickup_item_entity,
};
pub use persistence::{GameSnapshot, PersistenceError};
pub use player::{
    Abilities, Difficulty, Experience, GameMode, PlayerError, PlayerState, PlayerUuid, Vitals,
};
pub use status_effect::{
    EffectUpdate, StatusEffect, StatusEffectError, StatusEffectStore, MAX_EFFECT_AMPLIFIER,
    MAX_EFFECT_DURATION_TICKS,
};
pub use state::{GameEvent, GameRuleValue, GameState, GameStateError, GameTime};

pub const GAME_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

pub(crate) fn validate_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else { return false; };
    if namespace.is_empty() || path.is_empty() { return false; }
    let namespace_valid = namespace.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
    });
    let path_valid = path.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.' | b'/')
    });
    let segments_valid = path.split('/').all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    namespace_valid && path_valid && segments_valid
}

#[cfg(test)]
mod tests {
    use super::validate_resource_location;
    #[test]
    fn validates_minecraft_resource_locations() {
        assert!(validate_resource_location("minecraft:stone"));
        assert!(validate_resource_location("example:folder/value"));
        assert!(!validate_resource_location("stone"));
        assert!(!validate_resource_location("Minecraft:stone"));
        assert!(!validate_resource_location("minecraft:"));
        assert!(!validate_resource_location("minecraft:../stone"));
        assert!(!validate_resource_location("minecraft:folder//stone"));
    }
}
