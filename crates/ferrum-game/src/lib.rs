//! Version-neutral gameplay state for the native Ferrum server.
//!
//! This crate deliberately owns game concepts rather than wire encodings:
//! players, inventories, entities, commands, game time, and persistence
//! snapshots. Protocol crates translate these values to and from the selected
//! Minecraft version.

pub mod attributes;
pub mod combat;
pub mod command;
pub mod container;
pub mod effects;
pub mod entity;
pub mod inventory;
pub mod menu;
pub mod persistence;
pub mod player;
pub mod recipe;
mod state;

pub use attributes::{
    AttributeError, AttributeId, AttributeInstance, AttributeModifier, AttributeOperation,
    AttributeSet, MAX_ATTRIBUTE_MODIFIERS,
};
pub use combat::{
    CombatError, DamageContext, DamageKind, DamageResult, DamageSource, calculate_damage,
    fall_damage, knockback_velocity, reduce_by_armor,
};
pub use command::{
    CommandError, CommandOutcome, CommandSource, GameCommand, execute_command, parse_command,
};
pub use container::{
    ContainerClick, ContainerClickKind, ContainerError, ContainerMutation, ContainerSnapshot,
    InventorySession, MAX_CONTAINER_SLOTS, OUTSIDE_SLOT, PLAYER_CONTAINER_ID,
};
pub use effects::{
    MAX_STATUS_EFFECT_DURATION_TICKS, MAX_STATUS_EFFECTS, StatusEffectError, StatusEffectId,
    StatusEffectInstance, StatusEffectSet,
};
pub use entity::{
    DEFAULT_ITEM_DESPAWN_TICKS, DEFAULT_ITEM_PICKUP_DELAY_TICKS, Entity, EntityError, EntityId,
    EntityPayload, EntityStore, EntityType, EntityUuid, ItemEntityData, LivingEntityData,
    MAX_EXPERIENCE_ORB_VALUE, MAX_LIVING_ENTITY_DROPS, MAX_MOB_ATTACK_DAMAGE,
    MAX_MOB_ATTACK_INTERVAL_TICKS, MAX_MOB_ATTACK_RANGE, MAX_MOB_FOLLOW_RANGE,
    MAX_MOB_MOVEMENT_SPEED, MobAi, Transform, Velocity,
};
pub use inventory::{
    EquipmentSlot, HOTBAR_END, HOTBAR_SLOTS, HOTBAR_START, Inventory, InventoryError, ItemStack,
    MAIN_INVENTORY_END, MAIN_INVENTORY_START, MAX_VANILLA_STACK_SIZE, OFFHAND_SLOT,
    PLAYER_INVENTORY_SLOTS,
};
pub use menu::{
    FurnaceProgress, MAX_MENU_PROPERTIES, MAX_MENU_SLOTS, MenuError, MenuKind, MenuState,
};
pub use persistence::{GameSnapshot, PersistenceError};
pub use player::{
    Abilities, Difficulty, Experience, GameMode, MAX_EXPERIENCE_LEVEL, MAX_TOTAL_EXPERIENCE,
    PlayerError, PlayerState, PlayerUuid, Vitals,
};
pub use recipe::{
    CookingKind, CookingRecipe, CraftingGrid, CraftingMatch, Ingredient, MAX_CRAFTING_GRID_SLOTS,
    MAX_RECIPE_INGREDIENT_ALTERNATIVES, MAX_RECIPES, Recipe, RecipeError, RecipeRegistry,
    ShapedRecipe, ShapelessRecipe, consume_crafting_match,
};
pub use state::{
    GameEvent, GameRuleValue, GameState, GameStateError, GameTime, MAX_HOSTILE_MOB_SPAWN_DISTANCE,
    MAX_HOSTILE_MOBS, MIN_HOSTILE_MOB_SPAWN_DISTANCE,
};

pub const GAME_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

pub(crate) fn validate_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    if namespace.is_empty() || path.is_empty() {
        return false;
    }
    let namespace_valid = namespace.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
    });
    let path_valid = path.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'_' | b'-' | b'.' | b'/')
    });
    let segments_valid = path
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
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
