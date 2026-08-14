//! Minecraft Java Edition 26.1.2 synchronized entity-data layout.
//!
//! These values are version metadata, not gameplay semantics. They are derived
//! from the official 26.1.2 server classes (`Entity`, `ItemEntity`,
//! `ExperienceOrb`, and `EntityDataSerializers`) and intentionally stay in the
//! version adapter rather than `rom-play` or `rom-game`.

/// `Entity` defines eight synchronized data accessors before subclass fields.
pub const BASE_ENTITY_DATA_COUNT: u8 = 8;

/// `ItemEntity.DATA_ITEM` is the first accessor declared by `ItemEntity`.
pub const ITEM_ENTITY_STACK_INDEX: u8 = BASE_ENTITY_DATA_COUNT;
/// `EntityDataSerializers.ITEM_STACK` is registered after BYTE, INT, LONG,
/// FLOAT, STRING, COMPONENT and OPTIONAL_COMPONENT.
pub const ITEM_STACK_SERIALIZER_ID: i32 = 7;

/// `ExperienceOrb.DATA_VALUE` is the first accessor declared by `ExperienceOrb`.
pub const EXPERIENCE_ORB_VALUE_INDEX: u8 = BASE_ENTITY_DATA_COUNT;
/// `EntityDataSerializers.INT` is the second serializer registration.
pub const INT_SERIALIZER_ID: i32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subclass_metadata_starts_after_base_entity_fields() {
        assert_eq!(ITEM_ENTITY_STACK_INDEX, 8);
        assert_eq!(EXPERIENCE_ORB_VALUE_INDEX, 8);
    }

    #[test]
    fn serializer_ids_match_registration_order() {
        assert_eq!(INT_SERIALIZER_ID, 1);
        assert_eq!(ITEM_STACK_SERIALIZER_ID, 7);
    }
}
