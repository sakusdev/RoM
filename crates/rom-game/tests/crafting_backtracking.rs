use rom_game::{CraftingGrid, Ingredient, ItemStack, ShapelessRecipe};

fn ingredient(items: &[&str]) -> Ingredient {
    Ingredient::new(items.iter().map(|item| (*item).to_owned()).collect()).unwrap()
}

#[test]
fn overlapping_alternatives_backtrack_instead_of_greedy_failure() {
    // The broad ingredient is intentionally first. A greedy matcher consumes
    // stone for it and leaves dirt for the stone-only ingredient, producing a
    // false negative. A complete matcher assigns dirt to the broad ingredient.
    let recipe = ShapelessRecipe::new(
        "rom:overlapping".to_owned(),
        vec![
            ingredient(&["minecraft:stone", "minecraft:dirt"]),
            ingredient(&["minecraft:stone"]),
        ],
        ItemStack::new("minecraft:diamond", 1).unwrap(),
    )
    .unwrap();
    let mut grid = CraftingGrid::new(2, 2).unwrap();
    grid.set(0, Some(ItemStack::new("minecraft:stone", 1).unwrap()))
        .unwrap();
    grid.set(3, Some(ItemStack::new("minecraft:dirt", 1).unwrap()))
        .unwrap();

    assert!(recipe.matches(&grid));
    assert_eq!(recipe.consumption_plan(&grid), Some(vec![0, 3]));
}

#[test]
fn impossible_overlap_still_rejects_recipe() {
    let recipe = ShapelessRecipe::new(
        "rom:impossible_overlap".to_owned(),
        vec![
            ingredient(&["minecraft:stone", "minecraft:dirt"]),
            ingredient(&["minecraft:stone"]),
        ],
        ItemStack::new("minecraft:diamond", 1).unwrap(),
    )
    .unwrap();
    let mut grid = CraftingGrid::new(2, 2).unwrap();
    grid.set(0, Some(ItemStack::new("minecraft:dirt", 1).unwrap()))
        .unwrap();
    grid.set(3, Some(ItemStack::new("minecraft:gravel", 1).unwrap()))
        .unwrap();

    assert!(!recipe.matches(&grid));
}
