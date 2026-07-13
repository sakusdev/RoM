use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::validate_resource_location;

pub const PLAYER_INVENTORY_SLOTS: usize = 46;
pub const CRAFTING_OUTPUT_SLOT: usize = 0;
pub const CRAFTING_INPUT_START: usize = 1;
pub const CRAFTING_INPUT_END: usize = 4;
pub const ARMOR_START: usize = 5;
pub const ARMOR_END: usize = 8;
pub const MAIN_INVENTORY_START: usize = 9;
pub const MAIN_INVENTORY_END: usize = 35;
pub const HOTBAR_START: usize = 36;
pub const HOTBAR_END: usize = 44;
pub const OFFHAND_SLOT: usize = 45;
pub const HOTBAR_SLOTS: u8 = 9;
pub const MAX_VANILLA_STACK_SIZE: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentSlot {
    Head,
    Chest,
    Legs,
    Feet,
    MainHand,
    OffHand,
}

impl EquipmentSlot {
    #[must_use]
    pub fn inventory_index(self, selected_hotbar: u8) -> usize {
        match self {
            Self::Head => ARMOR_START,
            Self::Chest => ARMOR_START + 1,
            Self::Legs => ARMOR_START + 2,
            Self::Feet => ARMOR_END,
            Self::MainHand => HOTBAR_START + usize::from(selected_hotbar),
            Self::OffHand => OFFHAND_SLOT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    item: String,
    count: u32,
    max_count: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    components: BTreeMap<String, Value>,
}

impl ItemStack {
    pub fn new(item: impl Into<String>, count: u32) -> Result<Self, InventoryError> {
        Self::with_max_count(item, count, MAX_VANILLA_STACK_SIZE)
    }

    pub fn with_max_count(
        item: impl Into<String>,
        count: u32,
        max_count: u32,
    ) -> Result<Self, InventoryError> {
        let item = item.into();
        if !validate_resource_location(&item) {
            return Err(InventoryError::InvalidItemId { item });
        }
        if max_count == 0 || max_count > MAX_VANILLA_STACK_SIZE {
            return Err(InventoryError::InvalidMaximumCount { max_count });
        }
        if count == 0 || count > max_count {
            return Err(InventoryError::InvalidStackCount { count, max_count });
        }
        Ok(Self {
            item,
            count,
            max_count,
            components: BTreeMap::new(),
        })
    }

    pub fn with_component(mut self, key: impl Into<String>, value: Value) -> Self {
        self.components.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn item(&self) -> &str {
        &self.item
    }

    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }

    #[must_use]
    pub const fn max_count(&self) -> u32 {
        self.max_count
    }

    #[must_use]
    pub fn components(&self) -> &BTreeMap<String, Value> {
        &self.components
    }

    #[must_use]
    pub fn remaining_capacity(&self) -> u32 {
        self.max_count - self.count
    }

    #[must_use]
    pub fn can_merge(&self, other: &Self) -> bool {
        self.item == other.item
            && self.max_count == other.max_count
            && self.components == other.components
    }

    pub fn split(&mut self, amount: u32) -> Result<Self, InventoryError> {
        if amount == 0 || amount >= self.count {
            return Err(InventoryError::InvalidSplitAmount {
                amount,
                available: self.count,
            });
        }
        self.count -= amount;
        Ok(Self {
            item: self.item.clone(),
            count: amount,
            max_count: self.max_count,
            components: self.components.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    slots: Vec<Option<ItemStack>>,
    selected_hotbar: u8,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

impl Inventory {
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: vec![None; PLAYER_INVENTORY_SLOTS],
            selected_hotbar: 0,
        }
    }

    pub fn from_slots(
        slots: Vec<Option<ItemStack>>,
        selected_hotbar: u8,
    ) -> Result<Self, InventoryError> {
        if slots.len() != PLAYER_INVENTORY_SLOTS {
            return Err(InventoryError::InvalidSlotCount {
                actual: slots.len(),
                expected: PLAYER_INVENTORY_SLOTS,
            });
        }
        if selected_hotbar >= HOTBAR_SLOTS {
            return Err(InventoryError::InvalidHotbarSelection { selected_hotbar });
        }
        Ok(Self {
            slots,
            selected_hotbar,
        })
    }

    #[must_use]
    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }

    #[must_use]
    pub const fn selected_hotbar(&self) -> u8 {
        self.selected_hotbar
    }

    pub fn select_hotbar(&mut self, selected_hotbar: u8) -> Result<(), InventoryError> {
        if selected_hotbar >= HOTBAR_SLOTS {
            return Err(InventoryError::InvalidHotbarSelection { selected_hotbar });
        }
        self.selected_hotbar = selected_hotbar;
        Ok(())
    }

    pub fn slot(&self, index: usize) -> Result<Option<&ItemStack>, InventoryError> {
        self.slots
            .get(index)
            .map(Option::as_ref)
            .ok_or(InventoryError::SlotOutOfRange { index })
    }

    pub fn set_slot(
        &mut self,
        index: usize,
        stack: Option<ItemStack>,
    ) -> Result<Option<ItemStack>, InventoryError> {
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(InventoryError::SlotOutOfRange { index })?;
        Ok(std::mem::replace(slot, stack))
    }

    pub fn take_slot(&mut self, index: usize) -> Result<Option<ItemStack>, InventoryError> {
        self.set_slot(index, None)
    }

    #[must_use]
    pub fn selected_stack(&self) -> Option<&ItemStack> {
        self.slots[HOTBAR_START + usize::from(self.selected_hotbar)].as_ref()
    }

    #[must_use]
    pub fn equipment(&self, slot: EquipmentSlot) -> Option<&ItemStack> {
        self.slots[slot.inventory_index(self.selected_hotbar)].as_ref()
    }

    pub fn clear(&mut self) {
        self.slots.fill(None);
    }

    pub fn insert(&mut self, stack: ItemStack) -> Option<ItemStack> {
        self.insert_with_changed_slots(stack).0
    }

    pub fn insert_with_changed_slots(
        &mut self,
        mut stack: ItemStack,
    ) -> (Option<ItemStack>, Vec<usize>) {
        let mut changed_slots = Vec::new();
        for index in MAIN_INVENTORY_START..=HOTBAR_END {
            let Some(existing) = self.slots[index].as_mut() else {
                continue;
            };
            if !existing.can_merge(&stack) || existing.remaining_capacity() == 0 {
                continue;
            }
            let moved = existing.remaining_capacity().min(stack.count);
            existing.count += moved;
            stack.count -= moved;
            changed_slots.push(index);
            if stack.count == 0 {
                return (None, changed_slots);
            }
        }

        for index in MAIN_INVENTORY_START..=HOTBAR_END {
            if self.slots[index].is_some() {
                continue;
            }
            let moved = stack.count.min(stack.max_count);
            let mut placed = stack.clone();
            placed.count = moved;
            self.slots[index] = Some(placed);
            stack.count -= moved;
            changed_slots.push(index);
            if stack.count == 0 {
                return (None, changed_slots);
            }
        }

        (Some(stack), changed_slots)
    }

    #[must_use]
    pub fn occupied_slots(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryError {
    #[error("invalid item resource location {item}")]
    InvalidItemId { item: String },
    #[error("maximum stack count {max_count} must be between 1 and {MAX_VANILLA_STACK_SIZE}")]
    InvalidMaximumCount { max_count: u32 },
    #[error("stack count {count} must be between 1 and maximum {max_count}")]
    InvalidStackCount { count: u32, max_count: u32 },
    #[error("split amount {amount} must be between 1 and available count {available}")]
    InvalidSplitAmount { amount: u32, available: u32 },
    #[error("inventory has {actual} slots; expected {expected}")]
    InvalidSlotCount { actual: usize, expected: usize },
    #[error("inventory slot {index} is outside 0..{PLAYER_INVENTORY_SLOTS}")]
    SlotOutOfRange { index: usize },
    #[error("selected hotbar index {selected_hotbar} is outside 0..{HOTBAR_SLOTS}")]
    InvalidHotbarSelection { selected_hotbar: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_stack_sizes_and_ids() {
        assert!(ItemStack::new("minecraft:stone", 64).is_ok());
        assert!(ItemStack::new("stone", 1).is_err());
        assert!(ItemStack::new("minecraft:stone", 0).is_err());
        assert!(ItemStack::new("minecraft:stone", 65).is_err());
        assert!(ItemStack::with_max_count("minecraft:diamond_sword", 1, 1).is_ok());
    }

    #[test]
    fn merges_matching_stacks_and_uses_empty_slots() {
        let mut inventory = Inventory::new();
        inventory
            .set_slot(9, Some(ItemStack::new("minecraft:stone", 60).unwrap()))
            .unwrap();

        assert_eq!(
            inventory.insert(ItemStack::new("minecraft:stone", 8).unwrap()),
            None
        );
        assert_eq!(inventory.slot(9).unwrap().unwrap().count(), 64);
        assert_eq!(inventory.slot(10).unwrap().unwrap().count(), 4);
    }

    #[test]
    fn insertion_reports_every_changed_slot() {
        let mut inventory = Inventory::new();
        inventory
            .set_slot(9, Some(ItemStack::new("minecraft:stone", 60).unwrap()))
            .unwrap();
        let (remainder, changed) =
            inventory.insert_with_changed_slots(ItemStack::new("minecraft:stone", 8).unwrap());
        assert_eq!(remainder, None);
        assert_eq!(changed, vec![9, 10]);
    }

    #[test]
    fn components_prevent_incompatible_merges() {
        let mut inventory = Inventory::new();
        let named = ItemStack::new("minecraft:stone", 1)
            .unwrap()
            .with_component("minecraft:custom_name", json!({"text": "Named"}));
        inventory.set_slot(9, Some(named)).unwrap();
        inventory.insert(ItemStack::new("minecraft:stone", 1).unwrap());
        assert_eq!(inventory.occupied_slots(), 2);
    }

    #[test]
    fn selected_and_equipment_slots_match_player_inventory_layout() {
        let mut inventory = Inventory::new();
        inventory.select_hotbar(4).unwrap();
        inventory
            .set_slot(40, Some(ItemStack::new("minecraft:stone", 1).unwrap()))
            .unwrap();
        inventory
            .set_slot(45, Some(ItemStack::new("minecraft:shield", 1).unwrap()))
            .unwrap();

        assert_eq!(
            inventory.selected_stack().unwrap().item(),
            "minecraft:stone"
        );
        assert_eq!(
            inventory.equipment(EquipmentSlot::OffHand).unwrap().item(),
            "minecraft:shield"
        );
    }

    #[test]
    fn inventory_round_trips_through_json() {
        let mut inventory = Inventory::new();
        inventory.insert(ItemStack::new("minecraft:oak_log", 32).unwrap());
        inventory.select_hotbar(2).unwrap();
        let json = serde_json::to_string(&inventory).unwrap();
        let decoded: Inventory = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, inventory);
    }
}
