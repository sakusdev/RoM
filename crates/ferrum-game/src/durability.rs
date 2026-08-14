//! Item durability and deterministic Unbreaking handling.
//!
//! Durability is represented through the vanilla `minecraft:damage` component
//! so this stays compatible with the existing generic `ItemStack` component
//! storage instead of introducing a second source of truth.

use serde_json::json;
use thiserror::Error;

use crate::ItemStack;

pub const DAMAGE_COMPONENT: &str = "minecraft:damage";
pub const MAX_UNBREAKING_LEVEL: u8 = 127;

#[derive(Debug, Clone, PartialEq)]
pub struct DurabilityResult {
    pub stack: Option<ItemStack>,
    pub previous_damage: u32,
    pub current_damage: u32,
    pub attempted_damage: u32,
    pub applied_damage: u32,
    pub prevented_damage: u32,
    pub broken: bool,
}

#[must_use]
pub fn item_damage(stack: &ItemStack) -> u32 {
    stack
        .components()
        .get(DAMAGE_COMPONENT)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}

pub fn damage_item(
    stack: &ItemStack,
    max_damage: u32,
    amount: u32,
    unbreaking_level: u8,
    seed: u64,
) -> Result<DurabilityResult, DurabilityError> {
    if max_damage == 0 {
        return Err(DurabilityError::ZeroMaxDamage);
    }
    if unbreaking_level > MAX_UNBREAKING_LEVEL {
        return Err(DurabilityError::UnbreakingTooLarge { unbreaking_level });
    }
    if stack.count() != 1 {
        return Err(DurabilityError::StackableDamageableItem {
            count: stack.count(),
        });
    }

    let previous_damage = item_damage(stack).min(max_damage);
    if amount == 0 || previous_damage >= max_damage {
        return Ok(DurabilityResult {
            stack: (previous_damage < max_damage).then(|| stack.clone()),
            previous_damage,
            current_damage: previous_damage,
            attempted_damage: amount,
            applied_damage: 0,
            prevented_damage: amount,
            broken: previous_damage >= max_damage,
        });
    }

    let mut rng = SplitMix64::new(seed);
    let mut applied_damage = 0u32;
    for _ in 0..amount {
        if should_apply_damage(unbreaking_level, &mut rng) {
            applied_damage = applied_damage.saturating_add(1);
        }
    }
    let prevented_damage = amount.saturating_sub(applied_damage);
    let current_damage = previous_damage
        .saturating_add(applied_damage)
        .min(max_damage);
    let broken = current_damage >= max_damage;
    let stack = if broken {
        None
    } else {
        Some(
            stack
                .clone()
                .with_component(DAMAGE_COMPONENT, json!(current_damage)),
        )
    };

    Ok(DurabilityResult {
        stack,
        previous_damage,
        current_damage,
        attempted_damage: amount,
        applied_damage,
        prevented_damage,
        broken,
    })
}

fn should_apply_damage(unbreaking_level: u8, rng: &mut SplitMix64) -> bool {
    if unbreaking_level == 0 {
        return true;
    }
    let denominator = u32::from(unbreaking_level) + 1;
    rng.next_u32() % denominator == 0
}

#[must_use]
pub fn vanilla_max_durability(item: &str) -> Option<u32> {
    let suffix = item.strip_prefix("minecraft:")?;
    let value = match suffix {
        "wooden_sword" | "wooden_pickaxe" | "wooden_axe" | "wooden_shovel" | "wooden_hoe" => 59,
        "golden_sword" | "golden_pickaxe" | "golden_axe" | "golden_shovel" | "golden_hoe" => 32,
        "stone_sword" | "stone_pickaxe" | "stone_axe" | "stone_shovel" | "stone_hoe" => 131,
        "iron_sword" | "iron_pickaxe" | "iron_axe" | "iron_shovel" | "iron_hoe" => 250,
        "diamond_sword" | "diamond_pickaxe" | "diamond_axe" | "diamond_shovel" | "diamond_hoe" => {
            1561
        }
        "netherite_sword" | "netherite_pickaxe" | "netherite_axe" | "netherite_shovel"
        | "netherite_hoe" => 2031,
        "bow" => 384,
        "crossbow" => 465,
        "fishing_rod" => 64,
        "flint_and_steel" => 64,
        "shears" => 238,
        "shield" => 336,
        "trident" => 250,
        "elytra" => 432,
        "leather_helmet" => 55,
        "leather_chestplate" => 80,
        "leather_leggings" => 75,
        "leather_boots" => 65,
        "golden_helmet" => 77,
        "golden_chestplate" => 112,
        "golden_leggings" => 105,
        "golden_boots" => 91,
        "chainmail_helmet" => 165,
        "chainmail_chestplate" => 240,
        "chainmail_leggings" => 225,
        "chainmail_boots" => 195,
        "iron_helmet" => 165,
        "iron_chestplate" => 240,
        "iron_leggings" => 225,
        "iron_boots" => 195,
        "diamond_helmet" => 363,
        "diamond_chestplate" => 528,
        "diamond_leggings" => 495,
        "diamond_boots" => 429,
        "netherite_helmet" => 407,
        "netherite_chestplate" => 592,
        "netherite_leggings" => 555,
        "netherite_boots" => 481,
        _ => return None,
    };
    Some(value)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DurabilityError {
    #[error("maximum durability must be greater than zero")]
    ZeroMaxDamage,
    #[error("Unbreaking level {unbreaking_level} exceeds {MAX_UNBREAKING_LEVEL}")]
    UnbreakingTooLarge { unbreaking_level: u8 },
    #[error("damageable item stack must contain exactly one item, got {count}")]
    StackableDamageableItem { count: u32 },
}

#[derive(Debug, Clone, Copy)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sword() -> ItemStack {
        ItemStack::with_max_count("minecraft:diamond_sword", 1, 1).unwrap()
    }

    #[test]
    fn damage_component_defaults_to_zero() {
        assert_eq!(item_damage(&sword()), 0);
    }

    #[test]
    fn ordinary_damage_advances_component() {
        let result = damage_item(&sword(), 10, 3, 0, 1).unwrap();
        assert_eq!(result.previous_damage, 0);
        assert_eq!(result.current_damage, 3);
        assert_eq!(result.applied_damage, 3);
        assert_eq!(item_damage(result.stack.as_ref().unwrap()), 3);
    }

    #[test]
    fn item_breaks_at_max_damage() {
        let stack = sword().with_component(DAMAGE_COMPONENT, json!(9));
        let result = damage_item(&stack, 10, 1, 0, 1).unwrap();
        assert!(result.broken);
        assert!(result.stack.is_none());
    }

    #[test]
    fn unbreaking_is_deterministic() {
        let a = damage_item(&sword(), 100, 50, 3, 42).unwrap();
        let b = damage_item(&sword(), 100, 50, 3, 42).unwrap();
        assert_eq!(a, b);
        assert!(a.prevented_damage > 0);
    }

    #[test]
    fn vanilla_durability_covers_core_tools() {
        assert_eq!(vanilla_max_durability("minecraft:wooden_pickaxe"), Some(59));
        assert_eq!(
            vanilla_max_durability("minecraft:diamond_pickaxe"),
            Some(1561)
        );
        assert_eq!(
            vanilla_max_durability("minecraft:netherite_sword"),
            Some(2031)
        );
        assert_eq!(vanilla_max_durability("minecraft:stone"), None);
    }

    #[test]
    fn rejects_stacked_damageable_items() {
        let stack = ItemStack::new("minecraft:diamond_sword", 2).unwrap();
        assert_eq!(
            damage_item(&stack, 100, 1, 0, 0).unwrap_err(),
            DurabilityError::StackableDamageableItem { count: 2 }
        );
    }
}
