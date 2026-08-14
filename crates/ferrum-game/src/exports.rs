//! Crate-root re-exports for ferrum-game.

pub use crate::attributes::{
    Attribute, AttributeError, AttributeMap, AttributeModifier, AttributeOperation,
};
pub use crate::block_interaction::{
    BlockFace, BlockInteractionError, BlockPos, PlacementContext, PlacementDecision, PlacementMode,
    evaluate_placement, hit_vector_inside_block, opposite, within_reach as block_within_reach,
    yaw_to_horizontal_face,
};
pub use crate::command::{
    CommandError, CommandOutcome, CommandSource, GameCommand, execute_command, parse_command,
};
pub use crate::container::{
    ContainerClick, ContainerClickKind, ContainerError, ContainerMutation, ContainerSnapshot,
    InventorySession, MAX_CONTAINER_SLOTS, OUTSIDE_SLOT, PLAYER_CONTAINER_ID,
};
pub use crate::crafting::{
    CraftingError, CraftingGrid, CraftingRecipe, Ingredient, ShapedRecipe, ShapelessRecipe,
    craft_once,
};
pub use crate::damage::{
    DamageError, DamageKind, DamageMitigation, DamageResult, DamageSource, apply_armor,
    apply_protection, apply_resistance, calculate_damage, difficulty_multiplier, fall_damage,
    knockback_vector,
};
pub use crate::entity::{
    Entity, EntityError, EntityId, EntityStore, EntityType, EntityUuid, Transform, Velocity,
};
pub use crate::entity_tracking::{
    EntityTrackingState, TrackingConfig, TrackingDelta, visible_entities,
};
pub use crate::experience_orb::{
    EXPERIENCE_ORB_DATA_KEY, EXPERIENCE_ORB_DRAG, EXPERIENCE_ORB_GRAVITY_PER_TICK,
    EXPERIENCE_ORB_LIFETIME_TICKS, EXPERIENCE_ORB_MERGE_RANGE,
    EXPERIENCE_ORB_PICKUP_DELAY_TICKS, EXPERIENCE_ORB_PICKUP_RANGE, ExperienceOrbData,
    ExperienceOrbError, experience_orb_data, merge_experience_orbs, set_experience_orb_data,
    spawn_experience_orb, split_experience_value, tick_experience_orbs,
    try_pickup_experience_orb,
};
pub use crate::furnace::{
    DEFAULT_COOK_TIME_TICKS, Fuel, FurnaceError, FurnaceState, FurnaceTick, SmeltingRecipe,
    vanilla_fuel_burn_time,
};
pub use crate::inventory::{
    EquipmentSlot, HOTBAR_END, HOTBAR_SLOTS, HOTBAR_START, Inventory, InventoryError, ItemStack,
    MAIN_INVENTORY_END, MAIN_INVENTORY_START, MAX_VANILLA_STACK_SIZE, OFFHAND_SLOT,
    PLAYER_INVENTORY_SLOTS,
};
pub use crate::item_entity::{
    DEFAULT_ITEM_PICKUP_DELAY_TICKS, ITEM_ENTITY_AIR_DRAG, ITEM_ENTITY_DATA_KEY,
    ITEM_ENTITY_GRAVITY_PER_TICK, ITEM_ENTITY_GROUND_DRAG, ITEM_ENTITY_LIFETIME_TICKS,
    ITEM_ENTITY_VERTICAL_GROUND_DRAG, ITEM_MERGE_RANGE, ITEM_PICKUP_RANGE, ItemEntityData,
    ItemEntityError, ItemMergeResult, ItemPickupResult, item_entity_data,
    item_entity_in_pickup_range, merge_nearby_item_entities, set_item_entity_data,
    spawn_item_entity, spawn_item_entity_with_default_delay, tick_item_entities,
    try_merge_item_entities, try_pickup_item_entity,
};
pub use crate::loot::{LootContext, LootEntry, LootError, LootPool, LootTable, simple_block_drop};
pub use crate::persistence::{GameSnapshot, PersistenceError};
pub use crate::player::{
    Abilities, Difficulty, Experience, GameMode, PlayerError, PlayerState, PlayerUuid, Vitals,
};
pub use crate::raycast::{
    MAX_RAYCAST_STEPS, Ray, RaycastError, RaycastVisit, direction_from_rotation, first_matching,
    traverse_voxels,
};
pub use crate::scheduled_tick::{
    MAX_SCHEDULED_TICKS, ScheduledTick, ScheduledTickError, ScheduledTickQueue, TickPriority,
};
pub use crate::state::{GameEvent, GameRuleValue, GameState, GameStateError, GameTime};
pub use crate::status_effect::{
    EffectUpdate, MAX_EFFECT_AMPLIFIER, MAX_EFFECT_DURATION_TICKS, StatusEffect, StatusEffectError,
    StatusEffectStore,
};
