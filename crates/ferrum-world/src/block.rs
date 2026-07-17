use std::collections::BTreeMap;

use thiserror::Error;

use crate::{BlockStateId, VoxelShape};

pub const MAX_BLOCK_BEHAVIORS: usize = 1_048_576;
pub const MAX_BLOCK_DROPS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Sword,
    Shears,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolTier {
    Hand,
    Wood,
    Stone,
    Iron,
    Diamond,
    Netherite,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolProfile {
    pub kind: ToolKind,
    pub tier: ToolTier,
    pub mining_speed: f64,
    pub durability_cost: u32,
}

impl ToolProfile {
    pub fn new(
        kind: ToolKind,
        tier: ToolTier,
        mining_speed: f64,
        durability_cost: u32,
    ) -> Result<Self, BlockBehaviorError> {
        if !mining_speed.is_finite() || mining_speed <= 0.0 {
            return Err(BlockBehaviorError::InvalidMiningSpeed { mining_speed });
        }
        Ok(Self {
            kind,
            tier,
            mining_speed,
            durability_cost,
        })
    }

    #[must_use]
    pub const fn hand() -> Self {
        Self {
            kind: ToolKind::None,
            tier: ToolTier::Hand,
            mining_speed: 1.0,
            durability_cost: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDrop {
    pub item: String,
    pub minimum: u32,
    pub maximum: u32,
    pub requires_correct_tool: bool,
}

impl BlockDrop {
    pub fn new(
        item: impl Into<String>,
        minimum: u32,
        maximum: u32,
    ) -> Result<Self, BlockBehaviorError> {
        let item = item.into();
        if !is_resource_location(&item) {
            return Err(BlockBehaviorError::InvalidResourceLocation { value: item });
        }
        if minimum == 0 || minimum > maximum || maximum > 64 {
            return Err(BlockBehaviorError::InvalidDropRange { minimum, maximum });
        }
        Ok(Self {
            item,
            minimum,
            maximum,
            requires_correct_tool: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockBehavior {
    pub name: String,
    pub state: BlockStateId,
    pub hardness: f64,
    pub replaceable: bool,
    pub solid: bool,
    pub fluid: bool,
    pub required_tool: ToolKind,
    pub minimum_tier: ToolTier,
    pub collision: VoxelShape,
    pub drops: Vec<BlockDrop>,
}

impl BlockBehavior {
    pub fn validate(&self) -> Result<(), BlockBehaviorError> {
        if !is_resource_location(&self.name) {
            return Err(BlockBehaviorError::InvalidResourceLocation {
                value: self.name.clone(),
            });
        }
        if !self.hardness.is_finite() || self.hardness < -1.0 {
            return Err(BlockBehaviorError::InvalidHardness {
                hardness: self.hardness,
            });
        }
        if self.drops.len() > MAX_BLOCK_DROPS {
            return Err(BlockBehaviorError::TooManyDrops {
                actual: self.drops.len(),
                limit: MAX_BLOCK_DROPS,
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn can_harvest(&self, tool: ToolProfile) -> bool {
        if self.required_tool == ToolKind::None {
            return true;
        }
        tool.kind == self.required_tool && tool.tier >= self.minimum_tier
    }

    #[must_use]
    pub fn break_time_ticks(&self, tool: ToolProfile, haste_level: u8, fatigue_level: u8) -> u32 {
        if self.hardness < 0.0 {
            return u32::MAX;
        }
        if self.hardness == 0.0 {
            return 0;
        }
        let correct_tool = self.can_harvest(tool);
        let mut speed = if tool.kind == self.required_tool {
            tool.mining_speed
        } else {
            1.0
        };
        speed *= 1.0 + 0.2 * f64::from(haste_level);
        if fatigue_level > 0 {
            speed *= match fatigue_level {
                1 => 0.3,
                2 => 0.09,
                3 => 0.0027,
                _ => 0.00081,
            };
        }
        let divisor = if correct_tool { 30.0 } else { 100.0 };
        ((self.hardness * divisor / speed).ceil().max(1.0)) as u32
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockBehaviorRegistry {
    by_state: BTreeMap<BlockStateId, BlockBehavior>,
    by_name: BTreeMap<String, BlockStateId>,
}

impl BlockBehaviorRegistry {
    pub fn insert(
        &mut self,
        behavior: BlockBehavior,
    ) -> Result<Option<BlockBehavior>, BlockBehaviorError> {
        behavior.validate()?;
        if !self.by_state.contains_key(&behavior.state)
            && self.by_state.len() >= MAX_BLOCK_BEHAVIORS
        {
            return Err(BlockBehaviorError::TooManyBehaviors {
                limit: MAX_BLOCK_BEHAVIORS,
            });
        }
        if let Some(existing_state) = self.by_name.get(&behavior.name)
            && *existing_state != behavior.state
        {
            return Err(BlockBehaviorError::DuplicateBlockName {
                name: behavior.name,
            });
        }
        if let Some(previous) = self.by_state.get(&behavior.state)
            && previous.name != behavior.name
        {
            self.by_name.remove(&previous.name);
        }
        self.by_name.insert(behavior.name.clone(), behavior.state);
        Ok(self.by_state.insert(behavior.state, behavior))
    }

    #[must_use]
    pub fn by_state(&self, state: BlockStateId) -> Option<&BlockBehavior> {
        self.by_state.get(&state)
    }

    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&BlockBehavior> {
        self.by_name
            .get(name)
            .and_then(|state| self.by_state.get(state))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_state.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_state.is_empty()
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum BlockBehaviorError {
    #[error("invalid resource location {value}")]
    InvalidResourceLocation { value: String },
    #[error("block hardness {hardness} must be finite and at least -1")]
    InvalidHardness { hardness: f64 },
    #[error("mining speed {mining_speed} must be finite and positive")]
    InvalidMiningSpeed { mining_speed: f64 },
    #[error("block drop range {minimum}..={maximum} is invalid")]
    InvalidDropRange { minimum: u32, maximum: u32 },
    #[error("block has {actual} drops; limit is {limit}")]
    TooManyDrops { actual: usize, limit: usize },
    #[error("block behavior count exceeds {limit}")]
    TooManyBehaviors { limit: usize },
    #[error("block name {name} is registered for multiple states")]
    DuplicateBlockName { name: String },
}

fn is_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b'/')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_tools_harvest_and_break_faster() {
        let behavior = BlockBehavior {
            name: "minecraft:stone".to_owned(),
            state: BlockStateId::new(1),
            hardness: 1.5,
            replaceable: false,
            solid: true,
            fluid: false,
            required_tool: ToolKind::Pickaxe,
            minimum_tier: ToolTier::Wood,
            collision: VoxelShape::full_cube(),
            drops: vec![BlockDrop::new("minecraft:cobblestone", 1, 1).unwrap()],
        };
        let hand = behavior.break_time_ticks(ToolProfile::hand(), 0, 0);
        let pickaxe = behavior.break_time_ticks(
            ToolProfile::new(ToolKind::Pickaxe, ToolTier::Stone, 4.0, 1).unwrap(),
            0,
            0,
        );
        assert!(
            behavior
                .can_harvest(ToolProfile::new(ToolKind::Pickaxe, ToolTier::Wood, 2.0, 1).unwrap())
        );
        assert!(pickaxe < hand);
    }
}
