from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected one marker in {path}, found {count}: {old[:160]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


enchant = ROOT / "crates/rom-game/src/enchantment.rs"
enchant.write_text('''//! Semantic access to enchantments stored in generic ItemStack components.\n\nuse crate::ItemStack;\n\npub const ENCHANTMENTS_COMPONENT: &str = "minecraft:enchantments";\npub const EFFICIENCY_ENCHANTMENT: &str = "minecraft:efficiency";\npub const UNBREAKING_ENCHANTMENT: &str = "minecraft:unbreaking";\n\n/// Reads a server-semantic enchantment level from the `minecraft:enchantments`\n/// component. The component may also carry an explicit wire representation;\n/// protocol codecs ignore the semantic `levels` field when that wire payload is\n/// present. Unknown or malformed semantic data is treated as level zero.\n#[must_use]\npub fn item_enchantment_level(stack: &ItemStack, enchantment: &str) -> u8 {\n    stack\n        .components()\n        .get(ENCHANTMENTS_COMPONENT)\n        .and_then(serde_json::Value::as_object)\n        .and_then(|component| component.get("levels"))\n        .and_then(serde_json::Value::as_object)\n        .and_then(|levels| levels.get(enchantment))\n        .and_then(serde_json::Value::as_u64)\n        .map(|level| level.min(u64::from(u8::MAX)) as u8)\n        .unwrap_or(0)\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n    use serde_json::json;\n\n    #[test]\n    fn reads_semantic_enchantment_levels_and_clamps_extremes() {\n        let stack = ItemStack::with_max_count("minecraft:diamond_pickaxe", 1, 1)\n            .unwrap()\n            .with_component(\n                ENCHANTMENTS_COMPONENT,\n                json!({\n                    "wire_hex": "00",\n                    "levels": {\n                        EFFICIENCY_ENCHANTMENT: 5,\n                        UNBREAKING_ENCHANTMENT: 999,\n                    }\n                }),\n            );\n        assert_eq!(item_enchantment_level(&stack, EFFICIENCY_ENCHANTMENT), 5);\n        assert_eq!(item_enchantment_level(&stack, UNBREAKING_ENCHANTMENT), u8::MAX);\n        assert_eq!(item_enchantment_level(&stack, "minecraft:fortune"), 0);\n    }\n}\n''', encoding="utf-8")

lib = ROOT / "crates/rom-game/src/lib.rs"
replace_once(lib, "pub mod entity_tracking;\npub mod experience;", "pub mod entity_tracking;\npub mod enchantment;\npub mod experience;")

exports = ROOT / "crates/rom-game/src/exports.rs"
replace_once(
    exports,
    "pub use crate::entity_tracking::{\n    EntityTrackingState, TrackingConfig, TrackingDelta, visible_entities,\n};\n",
    "pub use crate::entity_tracking::{\n    EntityTrackingState, TrackingConfig, TrackingDelta, visible_entities,\n};\npub use crate::enchantment::{\n    EFFICIENCY_ENCHANTMENT, ENCHANTMENTS_COMPONENT, UNBREAKING_ENCHANTMENT,\n    item_enchantment_level,\n};\n",
)

inventory = ROOT / "crates/rom-play/src/inventory.rs"
replace_once(
    inventory,
    "    if object.len() != 1 {\n        return Ok(None);\n    }\n    if let Some(value) = object.get(\"wire_hex\") {",
    "    let known_wire_fields = [\n        \"wire_hex\", \"wire_bytes\", \"varint\", \"string\", \"bool\", \"i32\", \"i64\", \"f32\",\n        \"f64\",\n    ];\n    let wire_field_count = known_wire_fields\n        .into_iter()\n        .filter(|field| object.contains_key(*field))\n        .count();\n    if wire_field_count != 1 {\n        return Ok(None);\n    }\n    if let Some(value) = object.get(\"wire_hex\") {",
)
replace_once(
    inventory,
    "    #[test]\n    fn encodes_equipment_continuation_slots_and_rejects_duplicates() {",
    "    #[test]\n    fn explicit_wire_component_can_carry_server_semantic_fields() {\n        let items = ItemProtocolRegistry::new([(\"minecraft:diamond_pickaxe\", 7)]).unwrap();\n        let components = DataComponentProtocolRegistry::new([(\"minecraft:enchantments\", 11)])\n            .unwrap();\n        let stack = ItemStack::with_max_count(\"minecraft:diamond_pickaxe\", 1, 1)\n            .unwrap()\n            .with_component(\n                \"minecraft:enchantments\",\n                serde_json::json!({\n                    \"wire_hex\": \"abcd\",\n                    \"levels\": {\"minecraft:efficiency\": 4}\n                }),\n            );\n        let payload = encode_item_stack(Some(&stack), &items, &components)\n            .unwrap()\n            .unwrap();\n        assert!(payload.ends_with(&[11, 0xab, 0xcd]));\n    }\n\n    #[test]\n    fn encodes_equipment_continuation_slots_and_rejects_duplicates() {",
)

mining = ROOT / "crates/rom-server/src/mining_runtime.rs"
replace_once(
    mining,
    "    MAX_MINING_TICKS, MiningContext, MiningSessionError, MiningTool, PlayerUuid, ToolClass,\n    ToolTier, correct_tool, damage_item, ticks_to_break, vanilla_max_durability,\n",
    "    EFFICIENCY_ENCHANTMENT, MAX_MINING_TICKS, MAX_UNBREAKING_LEVEL, MiningContext,\n    MiningSessionError, MiningTool, PlayerUuid, ToolClass, ToolTier, UNBREAKING_ENCHANTMENT,\n    correct_tool, damage_item, item_enchantment_level, ticks_to_break, vanilla_max_durability,\n",
)
replace_once(
    mining,
    "        let selected = self.selected_item(uuid)?;\n        Ok(selected.and_then(|selected| mining_tool_from_item(selected.stack.item())))",
    "        let selected = self.selected_item(uuid)?;\n        Ok(selected.and_then(|selected| mining_tool_from_stack(&selected.stack)))",
)
replace_once(
    mining,
    "        let result = damage_item(&selected.stack, max_damage, 1, 0, seed)?;",
    "        let unbreaking_level = item_enchantment_level(&selected.stack, UNBREAKING_ENCHANTMENT)\n            .min(MAX_UNBREAKING_LEVEL);\n        let result = damage_item(&selected.stack, max_damage, 1, unbreaking_level, seed)?;",
)
replace_once(
    mining,
    "#[must_use]\npub fn mining_tool_from_item(item: &str) -> Option<MiningTool> {",
    "#[must_use]\npub fn mining_tool_from_stack(stack: &rom_game::ItemStack) -> Option<MiningTool> {\n    let mut tool = mining_tool_from_item(stack.item())?;\n    tool.efficiency_level = item_enchantment_level(stack, EFFICIENCY_ENCHANTMENT);\n    Some(tool)\n}\n\n#[must_use]\npub fn mining_tool_from_item(item: &str) -> Option<MiningTool> {",
)
replace_once(
    mining,
    "    #[test]\n    fn server_time_rejects_early_finish() {",
    "    #[test]\n    fn semantic_efficiency_component_changes_selected_mining_tool() {\n        let stack = ItemStack::with_max_count(\"minecraft:diamond_pickaxe\", 1, 1)\n            .unwrap()\n            .with_component(\n                rom_game::ENCHANTMENTS_COMPONENT,\n                serde_json::json!({\n                    \"wire_hex\": \"00\",\n                    \"levels\": {rom_game::EFFICIENCY_ENCHANTMENT: 5}\n                }),\n            );\n        let tool = mining_tool_from_stack(&stack).unwrap();\n        assert_eq!(tool.efficiency_level, 5);\n    }\n\n    #[test]\n    fn server_time_rejects_early_finish() {",
)

print("Integrated semantic Efficiency and Unbreaking mining components.")
