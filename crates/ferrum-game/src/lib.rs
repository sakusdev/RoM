//! Version-neutral gameplay state for the native Ferrum server.
//!
//! This crate deliberately owns game concepts rather than wire encodings:
//! players, inventories, entities, commands, game time, and persistence
//! snapshots. Protocol crates translate these values to and from the selected
//! Minecraft version.

pub mod command;
pub mod entity;
pub mod inventory;
pub mod persistence;
pub mod player;
mod state;

pub use command::{
    CommandError, CommandOutcome, CommandSource, GameCommand, execute_command, parse_command,
};
pub use entity::{
    Entity, EntityError, EntityId, EntityStore, EntityType, EntityUuid, Transform, Velocity,
};
pub use inventory::{
    EquipmentSlot, HOTBAR_SLOTS, Inventory, InventoryError, ItemStack, MAX_VANILLA_STACK_SIZE,
    PLAYER_INVENTORY_SLOTS,
};
pub use persistence::{GameSnapshot, PersistenceError};
pub use player::{
    Abilities, Difficulty, Experience, GameMode, PlayerError, PlayerState, PlayerUuid, Vitals,
};
pub use state::{GameEvent, GameRuleValue, GameState, GameStateError, GameTime};

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
