use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ItemStack;

pub const MAX_MENU_SLOTS: usize = 256;
pub const MAX_MENU_PROPERTIES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuKind {
    PlayerCrafting,
    CraftingTable,
    Furnace,
    BlastFurnace,
    Smoker,
    Stonecutter,
    EnchantingTable,
    BrewingStand,
    Anvil,
    SmithingTable,
    Grindstone,
    Loom,
    CartographyTable,
    Merchant,
    Generic9x1,
    Generic9x2,
    Generic9x3,
    Generic9x4,
    Generic9x5,
    Generic9x6,
}

impl MenuKind {
    #[must_use]
    pub const fn container_slots(self) -> usize {
        match self {
            Self::PlayerCrafting => 10,
            Self::CraftingTable => 10,
            Self::Furnace | Self::BlastFurnace | Self::Smoker => 3,
            Self::Stonecutter => 2,
            Self::EnchantingTable => 2,
            Self::BrewingStand => 5,
            Self::Anvil | Self::SmithingTable => 4,
            Self::Grindstone => 3,
            Self::Loom | Self::CartographyTable => 4,
            Self::Merchant => 3,
            Self::Generic9x1 => 9,
            Self::Generic9x2 => 18,
            Self::Generic9x3 => 27,
            Self::Generic9x4 => 36,
            Self::Generic9x5 => 45,
            Self::Generic9x6 => 54,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuState {
    pub container_id: i32,
    pub kind: MenuKind,
    pub state_id: i32,
    slots: Vec<Option<ItemStack>>,
    carried: Option<ItemStack>,
    properties: Vec<i32>,
}

impl MenuState {
    pub fn new(
        container_id: i32,
        kind: MenuKind,
        property_count: usize,
    ) -> Result<Self, MenuError> {
        if container_id <= 0 {
            return Err(MenuError::InvalidContainerId { container_id });
        }
        if property_count > MAX_MENU_PROPERTIES {
            return Err(MenuError::TooManyProperties {
                actual: property_count,
                limit: MAX_MENU_PROPERTIES,
            });
        }
        let slot_count = kind.container_slots();
        if slot_count > MAX_MENU_SLOTS {
            return Err(MenuError::TooManySlots {
                actual: slot_count,
                limit: MAX_MENU_SLOTS,
            });
        }
        Ok(Self {
            container_id,
            kind,
            state_id: 0,
            slots: vec![None; slot_count],
            carried: None,
            properties: vec![0; property_count],
        })
    }

    #[must_use]
    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }

    #[must_use]
    pub fn carried(&self) -> Option<&ItemStack> {
        self.carried.as_ref()
    }

    #[must_use]
    pub fn properties(&self) -> &[i32] {
        &self.properties
    }

    pub fn set_slot(
        &mut self,
        index: usize,
        stack: Option<ItemStack>,
    ) -> Result<Option<ItemStack>, MenuError> {
        let slot_count = self.slots.len();
        let slot = self.slots.get_mut(index).ok_or(MenuError::SlotOutOfRange {
            index,
            slots: slot_count,
        })?;
        self.state_id = self.state_id.wrapping_add(1);
        Ok(std::mem::replace(slot, stack))
    }

    pub fn set_carried(&mut self, stack: Option<ItemStack>) -> Option<ItemStack> {
        self.state_id = self.state_id.wrapping_add(1);
        std::mem::replace(&mut self.carried, stack)
    }

    pub fn set_property(&mut self, index: usize, value: i32) -> Result<i32, MenuError> {
        let property_count = self.properties.len();
        let property = self
            .properties
            .get_mut(index)
            .ok_or(MenuError::PropertyOutOfRange {
                index,
                properties: property_count,
            })?;
        self.state_id = self.state_id.wrapping_add(1);
        Ok(std::mem::replace(property, value))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FurnaceProgress {
    pub lit_time_remaining: u32,
    pub lit_time_total: u32,
    pub cooking_progress: u32,
    pub cooking_total: u32,
}

impl FurnaceProgress {
    pub fn ignite(&mut self, fuel_ticks: u32) -> Result<(), MenuError> {
        if fuel_ticks == 0 {
            return Err(MenuError::ZeroFuelTime);
        }
        self.lit_time_remaining = fuel_ticks;
        self.lit_time_total = fuel_ticks;
        Ok(())
    }

    pub fn set_recipe_time(&mut self, cooking_total: u32) -> Result<(), MenuError> {
        if cooking_total == 0 {
            return Err(MenuError::ZeroCookingTime);
        }
        self.cooking_total = cooking_total;
        self.cooking_progress = self.cooking_progress.min(cooking_total);
        Ok(())
    }

    #[must_use]
    pub fn tick(&mut self, can_cook: bool) -> bool {
        if self.lit_time_remaining > 0 {
            self.lit_time_remaining -= 1;
        }
        if can_cook && self.lit_time_remaining > 0 && self.cooking_total > 0 {
            self.cooking_progress = self.cooking_progress.saturating_add(1);
            if self.cooking_progress >= self.cooking_total {
                self.cooking_progress = 0;
                return true;
            }
        } else if !can_cook {
            self.cooking_progress = self.cooking_progress.saturating_sub(2);
        }
        false
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MenuError {
    #[error("container id {container_id} must be positive")]
    InvalidContainerId { container_id: i32 },
    #[error("menu has {actual} slots; limit is {limit}")]
    TooManySlots { actual: usize, limit: usize },
    #[error("menu has {actual} properties; limit is {limit}")]
    TooManyProperties { actual: usize, limit: usize },
    #[error("menu slot {index} is outside 0..{slots}")]
    SlotOutOfRange { index: usize, slots: usize },
    #[error("menu property {index} is outside 0..{properties}")]
    PropertyOutOfRange { index: usize, properties: usize },
    #[error("fuel time must be greater than zero")]
    ZeroFuelTime,
    #[error("cooking time must be greater than zero")]
    ZeroCookingTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_mutations_advance_state_ids() {
        let mut menu = MenuState::new(1, MenuKind::Furnace, 4).unwrap();
        menu.set_slot(0, Some(ItemStack::new("minecraft:iron_ore", 1).unwrap()))
            .unwrap();
        menu.set_property(0, 20).unwrap();
        assert_eq!(menu.state_id, 2);
    }

    #[test]
    fn furnace_progress_completes_and_resets() {
        let mut furnace = FurnaceProgress::default();
        furnace.ignite(10).unwrap();
        furnace.set_recipe_time(3).unwrap();
        assert!(!furnace.tick(true));
        assert!(!furnace.tick(true));
        assert!(furnace.tick(true));
        assert_eq!(furnace.cooking_progress, 0);
    }
}
