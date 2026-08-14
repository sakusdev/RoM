//! Version-neutral gameplay state for the native Ferrum server.
//!
//! Gameplay concepts live here; protocol crates translate them to and from the
//! selected Minecraft wire format.

pub mod attributes;
pub mod block_interaction;
pub mod command;
pub mod container;
pub mod crafting;
pub mod damage;
pub mod entity;
pub mod entity_tracking;
pub mod experience;
pub mod experience_orb;
mod exports;
pub mod furnace;
pub mod gameplay_tick;
pub mod hunger;
pub mod inventory;
pub mod item_entity;
pub mod loot;
pub mod mining;
pub mod persistence;
pub mod player;
pub mod player_gameplay;
pub mod player_tick;
pub mod raycast;
pub mod scheduled_tick;
pub mod spatial;
mod state;
pub mod status_effect;

pub use exports::*;

pub const GAME_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

pub(crate) fn validate_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    if namespace.is_empty() || path.is_empty() {
        return false;
    }
    let namespace_valid = namespace
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.'));
    let path_valid = path.bytes().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.' | b'/')
    });
    let segments_valid = path
        .split('/')
        .all(|s| !s.is_empty() && s != "." && s != "..");
    namespace_valid && path_valid && segments_valid
}

#[cfg(test)]
mod tests {
    use super::validate_resource_location;

    #[test]
    fn resource_locations() {
        assert!(validate_resource_location("minecraft:stone"));
        assert!(validate_resource_location("example:folder/value"));
        assert!(!validate_resource_location("stone"));
        assert!(!validate_resource_location("Minecraft:stone"));
        assert!(!validate_resource_location("minecraft:../stone"));
    }
}
