from pathlib import Path


def replace_once(path: Path, original: str, replacement: str, label: str) -> None:
    source = path.read_text()
    if source.count(original) != 1:
        raise SystemExit(f"expected {label} pattern not found exactly once in {path}")
    path.write_text(source.replace(original, replacement))


menu_path = Path("crates/ferrum-game/src/menu.rs")
menu = menu_path.read_text()
menu_replacements = [
    (
        "        let slot = self\n",
        "        let slot_count = self.slots.len();\n        let slot = self\n",
    ),
    (
        "                slots: self.slots.len(),\n",
        "                slots: slot_count,\n",
    ),
    (
        "        let property = self\n",
        "        let property_count = self.properties.len();\n        let property = self\n",
    ),
    (
        "                properties: self.properties.len(),\n",
        "                properties: property_count,\n",
    ),
]
for original, replacement in menu_replacements:
    if menu.count(original) != 1:
        raise SystemExit(f"expected menu pattern not found exactly once: {original!r}")
    menu = menu.replace(original, replacement)
menu_path.write_text(menu)

replace_once(
    Path("crates/ferrum-game/src/entity.rs"),
    "                age_ticks: 20,\n                data: BTreeMap::new(),\n",
    "                age_ticks: 20,\n                payload: EntityPayload::Generic,\n                data: BTreeMap::new(),\n",
    "restored entity fixture",
)

replace_once(
    Path("crates/ferrum-world/src/block.rs"),
    "    #[must_use]\n    pub fn len(&self) -> usize {\n        self.by_state.len()\n    }\n",
    "    #[must_use]\n    pub fn len(&self) -> usize {\n        self.by_state.len()\n    }\n\n    #[must_use]\n    pub fn is_empty(&self) -> bool {\n        self.by_state.is_empty()\n    }\n",
    "block behavior registry len",
)

replace_once(
    Path("crates/ferrum-world/src/block_entity.rs"),
    "    #[must_use]\n    pub fn len(&self) -> usize {\n        self.entries.len()\n    }\n",
    "    #[must_use]\n    pub fn len(&self) -> usize {\n        self.entries.len()\n    }\n\n    #[must_use]\n    pub fn is_empty(&self) -> bool {\n        self.entries.is_empty()\n    }\n",
    "block entity store len",
)

recipe_path = Path("crates/ferrum-game/src/recipe.rs")
recipe = recipe_path.read_text()
recipe_replacements = [
    (
        "                let mut consumed = Vec::new();\n                let mut valid = true;\n",
        "                let mut valid = true;\n",
    ),
    (
        "                            (Some(ingredient), Some(stack)) if ingredient.matches(stack) => {\n                                consumed.push(grid_index);\n                            }\n",
        "                            (Some(ingredient), Some(stack)) if ingredient.matches(stack) => {}\n",
    ),
    (
        "                if valid {\n                    return Some(CraftingMatch {\n                        recipe_id: id.to_owned(),\n                        consumed_slots: consumed,\n                        output: recipe.output.clone(),\n                    });\n                }\n",
        "                if valid {\n                    let consumed = recipe\n                        .pattern\n                        .iter()\n                        .enumerate()\n                        .filter_map(|(pattern_index, ingredient)| {\n                            ingredient.as_ref()?;\n                            let pattern_x = pattern_index % recipe.width;\n                            let pattern_y = pattern_index / recipe.width;\n                            let grid_x = offset_x\n                                + if mirrored {\n                                    recipe.width - 1 - pattern_x\n                                } else {\n                                    pattern_x\n                                };\n                            Some((offset_y + pattern_y) * grid.width + grid_x)\n                        })\n                        .collect();\n                    return Some(CraftingMatch {\n                        recipe_id: id.to_owned(),\n                        consumed_slots: consumed,\n                        output: recipe.output.clone(),\n                    });\n                }\n",
    ),
]
for original, replacement in recipe_replacements:
    if recipe.count(original) != 1:
        raise SystemExit(f"expected shaped recipe pattern not found exactly once: {original!r}")
    recipe = recipe.replace(original, replacement)
recipe_path.write_text(recipe)
