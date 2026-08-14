//! Semantic access to enchantments stored in generic ItemStack components.

use crate::ItemStack;

pub const ENCHANTMENTS_COMPONENT: &str = "minecraft:enchantments";
pub const EFFICIENCY_ENCHANTMENT: &str = "minecraft:efficiency";
pub const UNBREAKING_ENCHANTMENT: &str = "minecraft:unbreaking";

/// Reads a server-semantic enchantment level from the `minecraft:enchantments`
/// component. The component may also carry an explicit wire representation;
/// protocol codecs ignore the semantic `levels` field when that wire payload is
/// present. Unknown or malformed semantic data is treated as level zero.
#[must_use]
pub fn item_enchantment_level(stack: &ItemStack, enchantment: &str) -> u8 {
    stack
        .components()
        .get(ENCHANTMENTS_COMPONENT)
        .and_then(serde_json::Value::as_object)
        .and_then(|component| component.get("levels"))
        .and_then(serde_json::Value::as_object)
        .and_then(|levels| levels.get(enchantment))
        .and_then(serde_json::Value::as_u64)
        .map(|level| level.min(u64::from(u8::MAX)) as u8)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_semantic_enchantment_levels_and_clamps_extremes() {
        let stack = ItemStack::with_max_count("minecraft:diamond_pickaxe", 1, 1)
            .unwrap()
            .with_component(
                ENCHANTMENTS_COMPONENT,
                json!({
                    "wire_hex": "00",
                    "levels": {
                        EFFICIENCY_ENCHANTMENT: 5,
                        UNBREAKING_ENCHANTMENT: 999,
                    }
                }),
            );
        assert_eq!(item_enchantment_level(&stack, EFFICIENCY_ENCHANTMENT), 5);
        assert_eq!(
            item_enchantment_level(&stack, UNBREAKING_ENCHANTMENT),
            u8::MAX
        );
        assert_eq!(item_enchantment_level(&stack, "minecraft:fortune"), 0);
    }
}
