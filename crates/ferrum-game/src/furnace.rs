//! Furnace state, fuel accounting, and deterministic smelting progression.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{InventoryError, ItemStack, validate_resource_location};

pub const DEFAULT_COOK_TIME_TICKS: u32 = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmeltingRecipe {
    pub input: String,
    pub output: String,
    pub experience: f32,
    pub cook_time_ticks: u32,
}

impl SmeltingRecipe {
    pub fn new(
        input: impl Into<String>,
        output: impl Into<String>,
        experience: f32,
        cook_time_ticks: u32,
    ) -> Result<Self, FurnaceError> {
        let input = input.into();
        let output = output.into();
        if !validate_resource_location(&input) {
            return Err(FurnaceError::InvalidItem { item: input });
        }
        if !validate_resource_location(&output) {
            return Err(FurnaceError::InvalidItem { item: output });
        }
        if !experience.is_finite() || experience < 0.0 {
            return Err(FurnaceError::InvalidExperience { experience });
        }
        if cook_time_ticks == 0 || cook_time_ticks > 72_000 {
            return Err(FurnaceError::InvalidCookTime { cook_time_ticks });
        }
        Ok(Self {
            input,
            output,
            experience,
            cook_time_ticks,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fuel {
    pub item: String,
    pub burn_time_ticks: u32,
}

impl Fuel {
    pub fn new(item: impl Into<String>, burn_time_ticks: u32) -> Result<Self, FurnaceError> {
        let item = item.into();
        if !validate_resource_location(&item) {
            return Err(FurnaceError::InvalidItem { item });
        }
        if burn_time_ticks == 0 || burn_time_ticks > 1_000_000 {
            return Err(FurnaceError::InvalidBurnTime { burn_time_ticks });
        }
        Ok(Self {
            item,
            burn_time_ticks,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FurnaceState {
    pub input: Option<ItemStack>,
    pub fuel: Option<ItemStack>,
    pub output: Option<ItemStack>,
    pub burn_time_remaining: u32,
    pub burn_time_total: u32,
    pub cook_progress: u32,
    pub cook_time_total: u32,
    pub stored_experience: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FurnaceTick {
    Idle,
    Burning,
    StartedFuel,
    Smelted,
}

impl FurnaceState {
    #[must_use]
    pub fn is_burning(&self) -> bool {
        self.burn_time_remaining > 0
    }

    pub fn tick(
        &mut self,
        recipe: Option<&SmeltingRecipe>,
        fuel: Option<&Fuel>,
    ) -> Result<FurnaceTick, FurnaceError> {
        if self.burn_time_remaining > 0 {
            self.burn_time_remaining -= 1;
        }

        let Some(recipe) = recipe else {
            self.cook_progress = 0;
            self.cook_time_total = 0;
            return Ok(if self.is_burning() {
                FurnaceTick::Burning
            } else {
                FurnaceTick::Idle
            });
        };

        if !self.can_smelt(recipe)? {
            self.cook_progress = 0;
            self.cook_time_total = recipe.cook_time_ticks;
            return Ok(if self.is_burning() {
                FurnaceTick::Burning
            } else {
                FurnaceTick::Idle
            });
        }

        let mut started_fuel = false;
        if self.burn_time_remaining == 0 {
            if let Some(fuel_def) = fuel {
                if self.consume_fuel_if_matching(fuel_def)? {
                    self.burn_time_remaining = fuel_def.burn_time_ticks;
                    self.burn_time_total = fuel_def.burn_time_ticks;
                    started_fuel = true;
                }
            }
        }

        self.cook_time_total = recipe.cook_time_ticks;
        if self.burn_time_remaining == 0 {
            self.cook_progress = 0;
            return Ok(FurnaceTick::Idle);
        }

        self.cook_progress = self.cook_progress.saturating_add(1);
        if self.cook_progress >= recipe.cook_time_ticks {
            self.finish_smelt(recipe)?;
            self.cook_progress = 0;
            self.stored_experience += recipe.experience;
            return Ok(FurnaceTick::Smelted);
        }

        Ok(if started_fuel {
            FurnaceTick::StartedFuel
        } else {
            FurnaceTick::Burning
        })
    }

    pub fn take_experience(&mut self) -> f32 {
        let experience = self.stored_experience;
        self.stored_experience = 0.0;
        experience
    }

    fn can_smelt(&self, recipe: &SmeltingRecipe) -> Result<bool, FurnaceError> {
        let Some(input) = self.input.as_ref() else {
            return Ok(false);
        };
        if input.item() != recipe.input {
            return Ok(false);
        }
        let candidate = ItemStack::new(recipe.output.clone(), 1)?;
        match self.output.as_ref() {
            None => Ok(true),
            Some(output) if output.can_merge(&candidate) => Ok(output.remaining_capacity() > 0),
            Some(_) => Ok(false),
        }
    }

    fn consume_fuel_if_matching(&mut self, fuel_def: &Fuel) -> Result<bool, FurnaceError> {
        let Some(stack) = self.fuel.take() else {
            return Ok(false);
        };
        if stack.item() != fuel_def.item {
            self.fuel = Some(stack);
            return Ok(false);
        }
        if stack.count() > 1 {
            self.fuel = Some(stack.copy_with_count(stack.count() - 1)?);
        }
        Ok(true)
    }

    fn finish_smelt(&mut self, recipe: &SmeltingRecipe) -> Result<(), FurnaceError> {
        let input = self.input.take().ok_or(FurnaceError::MissingInput)?;
        if input.item() != recipe.input {
            self.input = Some(input);
            return Err(FurnaceError::RecipeMismatch);
        }
        if input.count() > 1 {
            self.input = Some(input.copy_with_count(input.count() - 1)?);
        }
        let produced = ItemStack::new(recipe.output.clone(), 1)?;
        self.output = match self.output.take() {
            None => Some(produced),
            Some(existing) if existing.can_merge(&produced) && existing.remaining_capacity() > 0 => {
                Some(existing.copy_with_count(existing.count() + 1)?)
            }
            Some(existing) => {
                self.output = Some(existing);
                return Err(FurnaceError::OutputBlocked);
            }
        };
        Ok(())
    }
}

#[must_use]
pub fn vanilla_fuel_burn_time(item: &str) -> Option<u32> {
    match item {
        "minecraft:coal" | "minecraft:charcoal" => Some(1_600),
        "minecraft:coal_block" => Some(16_000),
        "minecraft:blaze_rod" => Some(2_400),
        "minecraft:lava_bucket" => Some(20_000),
        "minecraft:stick" => Some(100),
        "minecraft:dried_kelp_block" => Some(4_001),
        _ if item.ends_with("_planks") => Some(300),
        _ if item.ends_with("_log") || item.ends_with("_wood") => Some(300),
        _ => None,
    }
}

#[derive(Debug, Error)]
pub enum FurnaceError {
    #[error("invalid furnace item {item}")]
    InvalidItem { item: String },
    #[error("invalid furnace experience {experience}")]
    InvalidExperience { experience: f32 },
    #[error("invalid furnace cook time {cook_time_ticks}")]
    InvalidCookTime { cook_time_ticks: u32 },
    #[error("invalid fuel burn time {burn_time_ticks}")]
    InvalidBurnTime { burn_time_ticks: u32 },
    #[error("furnace input disappeared during smelt")]
    MissingInput,
    #[error("furnace input no longer matches recipe")]
    RecipeMismatch,
    #[error("furnace output slot cannot accept the smelt result")]
    OutputBlocked,
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe() -> SmeltingRecipe {
        SmeltingRecipe::new("minecraft:iron_ore", "minecraft:iron_ingot", 0.7, 3).unwrap()
    }

    fn fuel() -> Fuel {
        Fuel::new("minecraft:coal", 20).unwrap()
    }

    #[test]
    fn furnace_consumes_one_fuel_and_smelts() {
        let mut furnace = FurnaceState {
            input: Some(ItemStack::new("minecraft:iron_ore", 2).unwrap()),
            fuel: Some(ItemStack::new("minecraft:coal", 2).unwrap()),
            ..FurnaceState::default()
        };
        assert_eq!(furnace.tick(Some(&recipe()), Some(&fuel())).unwrap(), FurnaceTick::StartedFuel);
        assert_eq!(furnace.tick(Some(&recipe()), Some(&fuel())).unwrap(), FurnaceTick::Burning);
        assert_eq!(furnace.tick(Some(&recipe()), Some(&fuel())).unwrap(), FurnaceTick::Smelted);
        assert_eq!(furnace.output.as_ref().unwrap().item(), "minecraft:iron_ingot");
        assert_eq!(furnace.input.as_ref().unwrap().count(), 1);
        assert_eq!(furnace.fuel.as_ref().unwrap().count(), 1);
    }

    #[test]
    fn blocked_output_prevents_progress() {
        let mut furnace = FurnaceState {
            input: Some(ItemStack::new("minecraft:iron_ore", 1).unwrap()),
            fuel: Some(ItemStack::new("minecraft:coal", 1).unwrap()),
            output: Some(ItemStack::new("minecraft:gold_ingot", 1).unwrap()),
            ..FurnaceState::default()
        };
        assert_eq!(furnace.tick(Some(&recipe()), Some(&fuel())).unwrap(), FurnaceTick::Idle);
        assert_eq!(furnace.cook_progress, 0);
    }

    #[test]
    fn vanilla_fuels_cover_common_items() {
        assert_eq!(vanilla_fuel_burn_time("minecraft:coal"), Some(1_600));
        assert_eq!(vanilla_fuel_burn_time("minecraft:oak_planks"), Some(300));
        assert_eq!(vanilla_fuel_burn_time("minecraft:diamond"), None);
    }
}
