//! Deterministic loot-table primitives for block and entity drops.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{InventoryError, ItemStack, validate_resource_location};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LootEntry {
    pub item: String,
    pub min_count: u32,
    pub max_count: u32,
    pub weight: u32,
    pub requires_silk_touch: bool,
    pub forbidden_with_silk_touch: bool,
    pub fortune_bonus_per_level: u32,
}

impl LootEntry {
    pub fn new(
        item: impl Into<String>,
        min_count: u32,
        max_count: u32,
        weight: u32,
    ) -> Result<Self, LootError> {
        let item = item.into();
        if !validate_resource_location(&item) {
            return Err(LootError::InvalidItem { item });
        }
        if min_count == 0 || max_count < min_count || max_count > 64 {
            return Err(LootError::InvalidCountRange {
                min: min_count,
                max: max_count,
            });
        }
        if weight == 0 {
            return Err(LootError::ZeroWeight);
        }
        Ok(Self {
            item,
            min_count,
            max_count,
            weight,
            requires_silk_touch: false,
            forbidden_with_silk_touch: false,
            fortune_bonus_per_level: 0,
        })
    }

    #[must_use]
    pub fn requiring_silk_touch(mut self) -> Self {
        self.requires_silk_touch = true;
        self
    }

    #[must_use]
    pub fn forbidden_with_silk_touch(mut self) -> Self {
        self.forbidden_with_silk_touch = true;
        self
    }

    #[must_use]
    pub fn with_fortune_bonus(mut self, bonus_per_level: u32) -> Self {
        self.fortune_bonus_per_level = bonus_per_level;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LootPool {
    pub rolls: u32,
    pub entries: Vec<LootEntry>,
}

impl LootPool {
    pub fn new(rolls: u32, entries: Vec<LootEntry>) -> Result<Self, LootError> {
        if rolls == 0 || rolls > 128 {
            return Err(LootError::InvalidRollCount { rolls });
        }
        if entries.is_empty() {
            return Err(LootError::EmptyPool);
        }
        let total_weight = entries
            .iter()
            .try_fold(0u32, |sum, entry| sum.checked_add(entry.weight))
            .ok_or(LootError::WeightOverflow)?;
        if total_weight == 0 {
            return Err(LootError::ZeroWeight);
        }
        Ok(Self { rolls, entries })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LootTable {
    pub pools: Vec<LootPool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LootContext {
    pub silk_touch: bool,
    pub fortune_level: u8,
    pub seed: u64,
}

impl LootTable {
    pub fn evaluate(&self, context: LootContext) -> Result<Vec<ItemStack>, LootError> {
        let mut rng = SplitMix64::new(context.seed);
        let mut raw = Vec::new();
        for pool in &self.pools {
            for _ in 0..pool.rolls {
                let eligible = pool
                    .entries
                    .iter()
                    .filter(|entry| {
                        (!entry.requires_silk_touch || context.silk_touch)
                            && (!entry.forbidden_with_silk_touch || !context.silk_touch)
                    })
                    .collect::<Vec<_>>();
                if eligible.is_empty() {
                    continue;
                }
                let total_weight = eligible.iter().map(|entry| entry.weight).sum::<u32>();
                let mut ticket = rng.next_u32() % total_weight;
                let mut selected = eligible[0];
                for entry in eligible {
                    if ticket < entry.weight {
                        selected = entry;
                        break;
                    }
                    ticket -= entry.weight;
                }
                let count_range = selected.max_count - selected.min_count + 1;
                let base_count = selected.min_count + rng.next_u32() % count_range;
                let fortune_bonus = selected
                    .fortune_bonus_per_level
                    .saturating_mul(u32::from(context.fortune_level));
                let count = base_count.saturating_add(fortune_bonus).max(1);
                raw.push((selected.item.clone(), count));
            }
        }
        merge_raw_drops(raw)
    }
}

pub fn simple_block_drop(block: &str) -> Result<LootTable, LootError> {
    Ok(LootTable {
        pools: vec![LootPool::new(1, vec![LootEntry::new(block, 1, 1, 1)?])?],
    })
}

fn merge_raw_drops(raw: Vec<(String, u32)>) -> Result<Vec<ItemStack>, LootError> {
    let mut merged = Vec::<ItemStack>::new();
    for (item, mut count) in raw {
        for stack in &mut merged {
            if stack.item() == item && stack.count() < stack.max_count() && count > 0 {
                let moved = stack.remaining_capacity().min(count);
                let replacement = stack.copy_with_count(stack.count() + moved)?;
                *stack = replacement;
                count -= moved;
            }
        }
        while count > 0 {
            let part = count.min(64);
            merged.push(ItemStack::new(item.clone(), part)?);
            count -= part;
        }
    }
    Ok(merged)
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
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }
}

#[derive(Debug, Error)]
pub enum LootError {
    #[error("invalid loot item {item}")]
    InvalidItem { item: String },
    #[error("invalid loot count range {min}..={max}")]
    InvalidCountRange { min: u32, max: u32 },
    #[error("loot entry weight must be non-zero")]
    ZeroWeight,
    #[error("loot pool must contain at least one entry")]
    EmptyPool,
    #[error("loot pool roll count {rolls} is outside 1..=128")]
    InvalidRollCount { rolls: u32 },
    #[error("loot pool weight overflowed")]
    WeightOverflow,
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_drop_returns_block_item() {
        let table = simple_block_drop("minecraft:stone").unwrap();
        let drops = table
            .evaluate(LootContext {
                silk_touch: false,
                fortune_level: 0,
                seed: 1,
            })
            .unwrap();
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].item(), "minecraft:stone");
    }

    #[test]
    fn silk_touch_conditions_are_applied() {
        let table = LootTable {
            pools: vec![LootPool::new(
                1,
                vec![
                    LootEntry::new("minecraft:diamond_ore", 1, 1, 1)
                        .unwrap()
                        .requiring_silk_touch(),
                    LootEntry::new("minecraft:diamond", 1, 1, 1)
                        .unwrap()
                        .forbidden_with_silk_touch(),
                ],
            )
            .unwrap()],
        };
        let silk = table
            .evaluate(LootContext {
                silk_touch: true,
                fortune_level: 0,
                seed: 2,
            })
            .unwrap();
        assert_eq!(silk[0].item(), "minecraft:diamond_ore");
    }

    #[test]
    fn evaluation_is_deterministic() {
        let table = LootTable {
            pools: vec![LootPool::new(
                8,
                vec![
                    LootEntry::new("minecraft:coal", 1, 3, 3).unwrap(),
                    LootEntry::new("minecraft:flint", 1, 1, 1).unwrap(),
                ],
            )
            .unwrap()],
        };
        let context = LootContext {
            silk_touch: false,
            fortune_level: 0,
            seed: 99,
        };
        assert_eq!(table.evaluate(context).unwrap(), table.evaluate(context).unwrap());
    }
}
