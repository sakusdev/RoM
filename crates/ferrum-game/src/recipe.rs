use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{InventoryError, ItemStack, validate_resource_location};

pub const MAX_RECIPE_INGREDIENT_ALTERNATIVES: usize = 128;
pub const MAX_CRAFTING_GRID_SLOTS: usize = 9;
pub const MAX_RECIPES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ingredient {
    alternatives: BTreeSet<String>,
}

impl Ingredient {
    pub fn new<I, S>(alternatives: I) -> Result<Self, RecipeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let alternatives = alternatives
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        if alternatives.is_empty() {
            return Err(RecipeError::EmptyIngredient);
        }
        if alternatives.len() > MAX_RECIPE_INGREDIENT_ALTERNATIVES {
            return Err(RecipeError::TooManyIngredientAlternatives {
                actual: alternatives.len(),
                limit: MAX_RECIPE_INGREDIENT_ALTERNATIVES,
            });
        }
        for item in &alternatives {
            if !validate_resource_location(item) {
                return Err(RecipeError::InvalidItemId { item: item.clone() });
            }
        }
        Ok(Self { alternatives })
    }

    #[must_use]
    pub fn matches(&self, stack: &ItemStack) -> bool {
        self.alternatives.contains(stack.item())
    }

    #[must_use]
    pub fn alternatives(&self) -> &BTreeSet<String> {
        &self.alternatives
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CraftingGrid {
    width: usize,
    height: usize,
    slots: Vec<Option<ItemStack>>,
}

impl CraftingGrid {
    pub fn new(
        width: usize,
        height: usize,
        slots: Vec<Option<ItemStack>>,
    ) -> Result<Self, RecipeError> {
        if !(1..=3).contains(&width) || !(1..=3).contains(&height) {
            return Err(RecipeError::InvalidGridSize { width, height });
        }
        let expected = width
            .checked_mul(height)
            .ok_or(RecipeError::GridSizeOverflow)?;
        if expected > MAX_CRAFTING_GRID_SLOTS || slots.len() != expected {
            return Err(RecipeError::InvalidGridSlotCount {
                actual: slots.len(),
                expected,
            });
        }
        Ok(Self {
            width,
            height,
            slots,
        })
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }

    pub fn slots_mut(&mut self) -> &mut [Option<ItemStack>] {
        &mut self.slots
    }

    #[must_use]
    pub fn slot(&self, x: usize, y: usize) -> Option<&ItemStack> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.slots[y * self.width + x].as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapedRecipe {
    pub width: usize,
    pub height: usize,
    pub pattern: Vec<Option<Ingredient>>,
    pub output: ItemStack,
    pub mirrored: bool,
}

impl ShapedRecipe {
    pub fn validate(&self) -> Result<(), RecipeError> {
        if !(1..=3).contains(&self.width) || !(1..=3).contains(&self.height) {
            return Err(RecipeError::InvalidRecipeSize {
                width: self.width,
                height: self.height,
            });
        }
        let expected = self
            .width
            .checked_mul(self.height)
            .ok_or(RecipeError::GridSizeOverflow)?;
        if self.pattern.len() != expected {
            return Err(RecipeError::InvalidPatternLength {
                actual: self.pattern.len(),
                expected,
            });
        }
        if self.pattern.iter().all(Option::is_none) {
            return Err(RecipeError::EmptyPattern);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapelessRecipe {
    pub ingredients: Vec<Ingredient>,
    pub output: ItemStack,
}

impl ShapelessRecipe {
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.ingredients.is_empty() || self.ingredients.len() > MAX_CRAFTING_GRID_SLOTS {
            return Err(RecipeError::InvalidShapelessIngredientCount {
                actual: self.ingredients.len(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CookingRecipe {
    pub input: Ingredient,
    pub output: ItemStack,
    pub cooking_time_ticks: u32,
    pub experience: f32,
}

impl CookingRecipe {
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.cooking_time_ticks == 0 {
            return Err(RecipeError::ZeroCookingTime);
        }
        if !self.experience.is_finite() || self.experience < 0.0 {
            return Err(RecipeError::InvalidExperience {
                experience: self.experience,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Recipe {
    Shaped(ShapedRecipe),
    Shapeless(ShapelessRecipe),
    Smelting(CookingRecipe),
    Blasting(CookingRecipe),
    Smoking(CookingRecipe),
    CampfireCooking(CookingRecipe),
    Stonecutting {
        input: Ingredient,
        output: ItemStack,
    },
    SmithingTransform {
        template: Ingredient,
        base: Ingredient,
        addition: Ingredient,
        output: ItemStack,
    },
}

impl Recipe {
    pub fn validate(&self) -> Result<(), RecipeError> {
        match self {
            Self::Shaped(recipe) => recipe.validate(),
            Self::Shapeless(recipe) => recipe.validate(),
            Self::Smelting(recipe)
            | Self::Blasting(recipe)
            | Self::Smoking(recipe)
            | Self::CampfireCooking(recipe) => recipe.validate(),
            Self::Stonecutting { .. } | Self::SmithingTransform { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CraftingMatch {
    pub recipe_id: String,
    pub consumed_slots: Vec<usize>,
    pub output: ItemStack,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RecipeRegistry {
    recipes: BTreeMap<String, Recipe>,
}

impl RecipeRegistry {
    pub fn insert(
        &mut self,
        id: impl Into<String>,
        recipe: Recipe,
    ) -> Result<Option<Recipe>, RecipeError> {
        let id = id.into();
        if !validate_resource_location(&id) {
            return Err(RecipeError::InvalidRecipeId { id });
        }
        recipe.validate()?;
        if !self.recipes.contains_key(&id) && self.recipes.len() >= MAX_RECIPES {
            return Err(RecipeError::TooManyRecipes { limit: MAX_RECIPES });
        }
        Ok(self.recipes.insert(id, recipe))
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Recipe> {
        self.recipes.get(id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Recipe)> {
        self.recipes
            .iter()
            .map(|(id, recipe)| (id.as_str(), recipe))
    }

    pub fn match_crafting(&self, grid: &CraftingGrid) -> Option<CraftingMatch> {
        for (id, recipe) in &self.recipes {
            let matched = match recipe {
                Recipe::Shaped(recipe) => match_shaped(id, recipe, grid),
                Recipe::Shapeless(recipe) => match_shapeless(id, recipe, grid),
                _ => None,
            };
            if matched.is_some() {
                return matched;
            }
        }
        None
    }

    pub fn match_cooking(
        &self,
        input: &ItemStack,
        kind: CookingKind,
    ) -> Option<(&str, &CookingRecipe)> {
        self.recipes.iter().find_map(|(id, recipe)| {
            let recipe = match (kind, recipe) {
                (CookingKind::Smelting, Recipe::Smelting(recipe))
                | (CookingKind::Blasting, Recipe::Blasting(recipe))
                | (CookingKind::Smoking, Recipe::Smoking(recipe))
                | (CookingKind::Campfire, Recipe::CampfireCooking(recipe)) => recipe,
                _ => return None,
            };
            recipe.input.matches(input).then_some((id.as_str(), recipe))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookingKind {
    Smelting,
    Blasting,
    Smoking,
    Campfire,
}

pub fn consume_crafting_match(
    grid: &mut CraftingGrid,
    matched: &CraftingMatch,
) -> Result<(), RecipeError> {
    let mut counts = BTreeMap::<usize, u32>::new();
    for &slot in &matched.consumed_slots {
        if slot >= grid.slots.len() {
            return Err(RecipeError::ConsumedSlotOutOfRange { slot });
        }
        *counts.entry(slot).or_default() += 1;
    }
    for (&slot, &amount) in &counts {
        let stack = grid.slots[slot]
            .as_ref()
            .ok_or(RecipeError::ConsumedSlotEmpty { slot })?;
        if stack.count() < amount {
            return Err(RecipeError::InsufficientIngredientCount {
                slot,
                available: stack.count(),
                required: amount,
            });
        }
    }
    for (slot, amount) in counts {
        let remove_slot = {
            let stack = grid.slots[slot]
                .as_mut()
                .expect("preflight verified crafting slot");
            stack.consume(amount)?
        };
        if remove_slot {
            grid.slots[slot] = None;
        }
    }
    Ok(())
}

fn match_shaped(id: &str, recipe: &ShapedRecipe, grid: &CraftingGrid) -> Option<CraftingMatch> {
    if recipe.width > grid.width || recipe.height > grid.height {
        return None;
    }
    for offset_y in 0..=grid.height - recipe.height {
        for offset_x in 0..=grid.width - recipe.width {
            for mirrored in [false, true] {
                if mirrored && !recipe.mirrored {
                    continue;
                }
                let mut valid = true;
                for y in 0..grid.height {
                    for x in 0..grid.width {
                        let grid_index = y * grid.width + x;
                        let recipe_x = x.checked_sub(offset_x);
                        let recipe_y = y.checked_sub(offset_y);
                        let expected = match (recipe_x, recipe_y) {
                            (Some(recipe_x), Some(recipe_y))
                                if recipe_x < recipe.width && recipe_y < recipe.height =>
                            {
                                let pattern_x = if mirrored {
                                    recipe.width - 1 - recipe_x
                                } else {
                                    recipe_x
                                };
                                recipe.pattern[recipe_y * recipe.width + pattern_x].as_ref()
                            }
                            _ => None,
                        };
                        match (expected, grid.slots[grid_index].as_ref()) {
                            (Some(ingredient), Some(stack)) if ingredient.matches(stack) => {}
                            (None, None) => {}
                            _ => valid = false,
                        }
                    }
                }
                if valid {
                    let consumed = recipe
                        .pattern
                        .iter()
                        .enumerate()
                        .filter_map(|(pattern_index, ingredient)| {
                            ingredient.as_ref()?;
                            let pattern_x = pattern_index % recipe.width;
                            let pattern_y = pattern_index / recipe.width;
                            let grid_x = offset_x
                                + if mirrored {
                                    recipe.width - 1 - pattern_x
                                } else {
                                    pattern_x
                                };
                            Some((offset_y + pattern_y) * grid.width + grid_x)
                        })
                        .collect();
                    return Some(CraftingMatch {
                        recipe_id: id.to_owned(),
                        consumed_slots: consumed,
                        output: recipe.output.clone(),
                    });
                }
            }
        }
    }
    None
}

fn match_shapeless(
    id: &str,
    recipe: &ShapelessRecipe,
    grid: &CraftingGrid,
) -> Option<CraftingMatch> {
    let occupied = grid
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, stack)| stack.as_ref().map(|stack| (slot, stack)))
        .collect::<Vec<_>>();
    if occupied.len() != recipe.ingredients.len() {
        return None;
    }
    let mut used = vec![false; occupied.len()];
    let mut assignment = Vec::with_capacity(recipe.ingredients.len());
    if !match_shapeless_recursive(
        &recipe.ingredients,
        &occupied,
        0,
        &mut used,
        &mut assignment,
    ) {
        return None;
    }
    Some(CraftingMatch {
        recipe_id: id.to_owned(),
        consumed_slots: assignment,
        output: recipe.output.clone(),
    })
}

fn match_shapeless_recursive(
    ingredients: &[Ingredient],
    occupied: &[(usize, &ItemStack)],
    index: usize,
    used: &mut [bool],
    assignment: &mut Vec<usize>,
) -> bool {
    if index == ingredients.len() {
        return true;
    }
    for candidate in 0..occupied.len() {
        if used[candidate] || !ingredients[index].matches(occupied[candidate].1) {
            continue;
        }
        used[candidate] = true;
        assignment.push(occupied[candidate].0);
        if match_shapeless_recursive(ingredients, occupied, index + 1, used, assignment) {
            return true;
        }
        assignment.pop();
        used[candidate] = false;
    }
    false
}

#[derive(Debug, Error)]
pub enum RecipeError {
    #[error("ingredient must contain at least one item")]
    EmptyIngredient,
    #[error("ingredient has {actual} alternatives; limit is {limit}")]
    TooManyIngredientAlternatives { actual: usize, limit: usize },
    #[error("invalid item resource location {item}")]
    InvalidItemId { item: String },
    #[error("crafting grid dimensions {width}x{height} must each be between 1 and 3")]
    InvalidGridSize { width: usize, height: usize },
    #[error("crafting-grid size arithmetic overflowed")]
    GridSizeOverflow,
    #[error("crafting grid has {actual} slots; expected {expected}")]
    InvalidGridSlotCount { actual: usize, expected: usize },
    #[error("recipe dimensions {width}x{height} must each be between 1 and 3")]
    InvalidRecipeSize { width: usize, height: usize },
    #[error("recipe pattern has {actual} entries; expected {expected}")]
    InvalidPatternLength { actual: usize, expected: usize },
    #[error("shaped recipe pattern cannot be empty")]
    EmptyPattern,
    #[error("shapeless recipe has invalid ingredient count {actual}")]
    InvalidShapelessIngredientCount { actual: usize },
    #[error("cooking time must be greater than zero")]
    ZeroCookingTime,
    #[error("recipe experience {experience} must be finite and non-negative")]
    InvalidExperience { experience: f32 },
    #[error("invalid recipe resource location {id}")]
    InvalidRecipeId { id: String },
    #[error("recipe count exceeds {limit}")]
    TooManyRecipes { limit: usize },
    #[error("consumed crafting slot {slot} is outside the grid")]
    ConsumedSlotOutOfRange { slot: usize },
    #[error("consumed crafting slot {slot} is empty")]
    ConsumedSlotEmpty { slot: usize },
    #[error("crafting slot {slot} has {available} items but {required} are required")]
    InsufficientIngredientCount {
        slot: usize,
        available: u32,
        required: u32,
    },
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> Ingredient {
        Ingredient::new([id]).unwrap()
    }

    #[test]
    fn matches_offset_and_mirrored_shaped_recipes() {
        let recipe = ShapedRecipe {
            width: 2,
            height: 1,
            pattern: vec![Some(item("minecraft:stick")), Some(item("minecraft:coal"))],
            output: ItemStack::new("minecraft:torch", 4).unwrap(),
            mirrored: true,
        };
        let mut registry = RecipeRegistry::default();
        registry
            .insert("minecraft:torch", Recipe::Shaped(recipe))
            .unwrap();
        let grid = CraftingGrid::new(
            3,
            3,
            vec![
                None,
                None,
                None,
                None,
                Some(ItemStack::new("minecraft:coal", 1).unwrap()),
                Some(ItemStack::new("minecraft:stick", 1).unwrap()),
                None,
                None,
                None,
            ],
        )
        .unwrap();
        let matched = registry.match_crafting(&grid).unwrap();
        assert_eq!(matched.output.item(), "minecraft:torch");
        assert_eq!(matched.consumed_slots, [5, 4]);
    }

    #[test]
    fn shapeless_matching_handles_overlapping_alternatives() {
        let recipe = ShapelessRecipe {
            ingredients: vec![
                Ingredient::new(["minecraft:oak_planks", "minecraft:birch_planks"]).unwrap(),
                item("minecraft:oak_planks"),
            ],
            output: ItemStack::new("minecraft:stick", 4).unwrap(),
        };
        let mut registry = RecipeRegistry::default();
        registry
            .insert("minecraft:sticks", Recipe::Shapeless(recipe))
            .unwrap();
        let grid = CraftingGrid::new(
            2,
            2,
            vec![
                Some(ItemStack::new("minecraft:oak_planks", 1).unwrap()),
                Some(ItemStack::new("minecraft:birch_planks", 1).unwrap()),
                None,
                None,
            ],
        )
        .unwrap();
        assert!(registry.match_crafting(&grid).is_some());
    }

    #[test]
    fn consuming_a_match_is_atomic() {
        let mut grid = CraftingGrid::new(
            2,
            2,
            vec![
                Some(ItemStack::new("minecraft:stone", 2).unwrap()),
                None,
                None,
                None,
            ],
        )
        .unwrap();
        let matched = CraftingMatch {
            recipe_id: "minecraft:test".to_owned(),
            consumed_slots: vec![0, 0],
            output: ItemStack::new("minecraft:diamond", 1).unwrap(),
        };
        consume_crafting_match(&mut grid, &matched).unwrap();
        assert!(grid.slots()[0].is_none());
    }
}
