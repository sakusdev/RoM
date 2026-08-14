//! Version-neutral crafting recipe matching and consumption planning.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{InventoryError, ItemStack, validate_resource_location};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ingredient(pub Vec<String>);

impl Ingredient {
    pub fn new(items: Vec<String>) -> Result<Self, CraftingError> {
        if items.is_empty() {
            return Err(CraftingError::Empty);
        }
        for value in &items {
            if !validate_resource_location(value) {
                return Err(CraftingError::InvalidId(value.clone()));
            }
        }
        Ok(Self(items))
    }

    #[must_use]
    pub fn matches(&self, stack: &ItemStack) -> bool {
        self.0.iter().any(|value| value == stack.item())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CraftingGrid {
    pub width: u8,
    pub height: u8,
    pub slots: Vec<Option<ItemStack>>,
}

impl CraftingGrid {
    pub fn new(width: u8, height: u8) -> Result<Self, CraftingError> {
        let size = usize::from(width) * usize::from(height);
        if width == 0 || height == 0 || size > 9 {
            return Err(CraftingError::Grid);
        }
        Ok(Self {
            width,
            height,
            slots: vec![None; size],
        })
    }

    pub fn set(&mut self, index: usize, stack: Option<ItemStack>) -> Result<(), CraftingError> {
        let slot = self.slots.get_mut(index).ok_or(CraftingError::Slot)?;
        *slot = stack;
        Ok(())
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<&ItemStack> {
        if x >= usize::from(self.width) || y >= usize::from(self.height) {
            return None;
        }
        self.slots[y * usize::from(self.width) + x].as_ref()
    }

    pub fn consume_slots(&mut self, slots: &[usize]) -> Result<(), CraftingError> {
        for &index in slots {
            let slot = self.slots.get_mut(index).ok_or(CraftingError::Slot)?;
            let stack = slot.take().ok_or(CraftingError::MissingIngredient { index })?;
            if stack.count() > 1 {
                *slot = Some(stack.copy_with_count(stack.count() - 1)?);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapelessRecipe {
    pub id: String,
    pub ingredients: Vec<Ingredient>,
    pub result: ItemStack,
}

impl ShapelessRecipe {
    pub fn new(
        id: String,
        ingredients: Vec<Ingredient>,
        result: ItemStack,
    ) -> Result<Self, CraftingError> {
        validate_recipe_id(&id)?;
        if ingredients.is_empty() || ingredients.len() > 9 {
            return Err(CraftingError::Empty);
        }
        Ok(Self {
            id,
            ingredients,
            result,
        })
    }

    #[must_use]
    pub fn matches(&self, grid: &CraftingGrid) -> bool {
        self.consumption_plan(grid).is_some()
    }

    #[must_use]
    pub fn consumption_plan(&self, grid: &CraftingGrid) -> Option<Vec<usize>> {
        let stacks = grid
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, stack)| stack.as_ref().map(|stack| (index, stack)))
            .collect::<Vec<_>>();
        if stacks.len() != self.ingredients.len() {
            return None;
        }
        let mut used = vec![false; stacks.len()];
        let mut plan = Vec::with_capacity(stacks.len());
        for ingredient in &self.ingredients {
            let position = stacks
                .iter()
                .enumerate()
                .position(|(i, (_, stack))| !used[i] && ingredient.matches(stack))?;
            used[position] = true;
            plan.push(stacks[position].0);
        }
        plan.sort_unstable();
        Some(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapedRecipe {
    pub id: String,
    pub width: u8,
    pub height: u8,
    pub ingredients: Vec<Option<Ingredient>>,
    pub result: ItemStack,
    pub allow_horizontal_mirror: bool,
}

impl ShapedRecipe {
    pub fn new(
        id: String,
        width: u8,
        height: u8,
        ingredients: Vec<Option<Ingredient>>,
        result: ItemStack,
    ) -> Result<Self, CraftingError> {
        validate_recipe_id(&id)?;
        let size = usize::from(width) * usize::from(height);
        if width == 0 || height == 0 || width > 3 || height > 3 || ingredients.len() != size {
            return Err(CraftingError::InvalidPattern);
        }
        if ingredients.iter().all(Option::is_none) {
            return Err(CraftingError::Empty);
        }
        Ok(Self {
            id,
            width,
            height,
            ingredients,
            result,
            allow_horizontal_mirror: true,
        })
    }

    #[must_use]
    pub fn matches(&self, grid: &CraftingGrid) -> bool {
        self.consumption_plan(grid).is_some()
    }

    #[must_use]
    pub fn consumption_plan(&self, grid: &CraftingGrid) -> Option<Vec<usize>> {
        if self.width > grid.width || self.height > grid.height {
            return None;
        }
        let max_x = usize::from(grid.width - self.width);
        let max_y = usize::from(grid.height - self.height);
        for offset_y in 0..=max_y {
            for offset_x in 0..=max_x {
                if let Some(plan) = self.match_at(grid, offset_x, offset_y, false) {
                    return Some(plan);
                }
                if self.allow_horizontal_mirror {
                    if let Some(plan) = self.match_at(grid, offset_x, offset_y, true) {
                        return Some(plan);
                    }
                }
            }
        }
        None
    }

    fn match_at(
        &self,
        grid: &CraftingGrid,
        offset_x: usize,
        offset_y: usize,
        mirrored: bool,
    ) -> Option<Vec<usize>> {
        let grid_width = usize::from(grid.width);
        let recipe_width = usize::from(self.width);
        let recipe_height = usize::from(self.height);
        let mut plan = Vec::new();
        for y in 0..usize::from(grid.height) {
            for x in 0..grid_width {
                let inside = x >= offset_x
                    && y >= offset_y
                    && x < offset_x + recipe_width
                    && y < offset_y + recipe_height;
                let expected = if inside {
                    let recipe_x = x - offset_x;
                    let recipe_y = y - offset_y;
                    let recipe_x = if mirrored {
                        recipe_width - 1 - recipe_x
                    } else {
                        recipe_x
                    };
                    self.ingredients[recipe_y * recipe_width + recipe_x].as_ref()
                } else {
                    None
                };
                let index = y * grid_width + x;
                let actual = grid.slots[index].as_ref();
                match (expected, actual) {
                    (None, None) => {}
                    (Some(ingredient), Some(stack)) if ingredient.matches(stack) => plan.push(index),
                    _ => return None,
                }
            }
        }
        Some(plan)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CraftingRecipe {
    Shapeless(ShapelessRecipe),
    Shaped(ShapedRecipe),
}

impl CraftingRecipe {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Shapeless(recipe) => &recipe.id,
            Self::Shaped(recipe) => &recipe.id,
        }
    }

    #[must_use]
    pub fn result(&self) -> &ItemStack {
        match self {
            Self::Shapeless(recipe) => &recipe.result,
            Self::Shaped(recipe) => &recipe.result,
        }
    }

    #[must_use]
    pub fn consumption_plan(&self, grid: &CraftingGrid) -> Option<Vec<usize>> {
        match self {
            Self::Shapeless(recipe) => recipe.consumption_plan(grid),
            Self::Shaped(recipe) => recipe.consumption_plan(grid),
        }
    }
}

pub fn craft_once(
    grid: &mut CraftingGrid,
    recipe: &CraftingRecipe,
) -> Result<Option<ItemStack>, CraftingError> {
    let Some(plan) = recipe.consumption_plan(grid) else {
        return Ok(None);
    };
    grid.consume_slots(&plan)?;
    Ok(Some(recipe.result().clone()))
}

fn validate_recipe_id(id: &str) -> Result<(), CraftingError> {
    if !validate_resource_location(id) {
        return Err(CraftingError::InvalidId(id.to_owned()));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum CraftingError {
    #[error("invalid id {0}")]
    InvalidId(String),
    #[error("empty recipe or ingredient")]
    Empty,
    #[error("invalid crafting grid")]
    Grid,
    #[error("crafting slot out of range")]
    Slot,
    #[error("invalid shaped recipe pattern")]
    InvalidPattern,
    #[error("crafting ingredient missing from slot {index}")]
    MissingIngredient { index: usize },
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> Ingredient {
        Ingredient::new(vec![name.to_owned()]).unwrap()
    }

    #[test]
    fn shapeless_matches_any_slot_order() {
        let recipe = ShapelessRecipe::new(
            "rom:test".into(),
            vec![item("minecraft:stone"), item("minecraft:dirt")],
            ItemStack::new("minecraft:diamond", 1).unwrap(),
        )
        .unwrap();
        let mut grid = CraftingGrid::new(2, 2).unwrap();
        grid.set(3, Some(ItemStack::new("minecraft:stone", 1).unwrap()))
            .unwrap();
        grid.set(0, Some(ItemStack::new("minecraft:dirt", 1).unwrap()))
            .unwrap();
        assert!(recipe.matches(&grid));
    }

    #[test]
    fn shaped_recipe_matches_offset_and_mirror() {
        let recipe = ShapedRecipe::new(
            "rom:axe".into(),
            2,
            2,
            vec![
                Some(item("minecraft:planks")),
                Some(item("minecraft:stick")),
                Some(item("minecraft:planks")),
                None,
            ],
            ItemStack::new("minecraft:wooden_axe", 1).unwrap(),
        )
        .unwrap();
        let mut grid = CraftingGrid::new(3, 3).unwrap();
        grid.set(1, Some(ItemStack::new("minecraft:stick", 1).unwrap()))
            .unwrap();
        grid.set(0, Some(ItemStack::new("minecraft:planks", 1).unwrap()))
            .unwrap();
        grid.set(3, Some(ItemStack::new("minecraft:planks", 1).unwrap()))
            .unwrap();
        assert!(recipe.matches(&grid));
    }

    #[test]
    fn craft_once_consumes_one_from_each_slot() {
        let recipe = CraftingRecipe::Shapeless(
            ShapelessRecipe::new(
                "rom:test".into(),
                vec![item("minecraft:stone")],
                ItemStack::new("minecraft:dirt", 1).unwrap(),
            )
            .unwrap(),
        );
        let mut grid = CraftingGrid::new(2, 2).unwrap();
        grid.set(2, Some(ItemStack::new("minecraft:stone", 3).unwrap()))
            .unwrap();
        let result = craft_once(&mut grid, &recipe).unwrap().unwrap();
        assert_eq!(result.item(), "minecraft:dirt");
        assert_eq!(grid.slots[2].as_ref().unwrap().count(), 2);
    }
}
