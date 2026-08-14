//! Block breaking progress, tool suitability, and durability helpers.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolClass {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Shears,
    Sword,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ToolTier {
    Wood,
    Gold,
    Stone,
    Iron,
    Diamond,
    Netherite,
}
impl ToolTier {
    #[must_use]
    pub const fn speed(self) -> f64 {
        match self {
            Self::Wood => 2.0,
            Self::Gold => 12.0,
            Self::Stone => 4.0,
            Self::Iron => 6.0,
            Self::Diamond => 8.0,
            Self::Netherite => 9.0,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MiningTool {
    pub class: ToolClass,
    pub tier: ToolTier,
    pub efficiency_level: u8,
    pub durability: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BlockMining {
    pub hardness: f64,
    pub preferred_tool: ToolClass,
    pub required_tier: Option<ToolTier>,
    pub requires_correct_tool: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MiningContext {
    pub on_ground: bool,
    pub underwater: bool,
    pub haste: f64,
    pub fatigue: f64,
}
impl Default for MiningContext {
    fn default() -> Self {
        Self {
            on_ground: true,
            underwater: false,
            haste: 1.0,
            fatigue: 1.0,
        }
    }
}
#[must_use]
pub fn correct_tool(tool: Option<MiningTool>, block: BlockMining) -> bool {
    let Some(tool) = tool else {
        return !block.requires_correct_tool;
    };
    if tool.class != block.preferred_tool {
        return !block.requires_correct_tool;
    }
    block.required_tier.is_none_or(|tier| tool.tier >= tier)
}
#[must_use]
pub fn destroy_speed(tool: Option<MiningTool>, block: BlockMining, ctx: MiningContext) -> f64 {
    if block.hardness < 0.0 {
        return 0.0;
    }
    let preferred = tool.is_some_and(|t| t.class == block.preferred_tool);
    let mut speed = if preferred {
        tool.map_or(1.0, |t| {
            let mut speed = t.tier.speed();
            if t.efficiency_level > 0 {
                let efficiency = u32::from(t.efficiency_level);
                speed += f64::from(efficiency.saturating_mul(efficiency)) + 1.0;
            }
            speed
        })
    } else {
        1.0
    };
    speed *= ctx.haste.max(0.0) * ctx.fatigue.max(0.0);
    if ctx.underwater {
        speed /= 5.0;
    }
    if !ctx.on_ground {
        speed /= 5.0;
    }
    speed
}
#[must_use]
pub fn progress_per_tick(tool: Option<MiningTool>, block: BlockMining, ctx: MiningContext) -> f64 {
    if block.hardness <= 0.0 {
        return if block.hardness == 0.0 { 1.0 } else { 0.0 };
    }
    let divisor = if correct_tool(tool, block) {
        30.0
    } else {
        100.0
    };
    destroy_speed(tool, block, ctx) / block.hardness / divisor
}
#[must_use]
pub fn ticks_to_break(
    tool: Option<MiningTool>,
    block: BlockMining,
    ctx: MiningContext,
) -> Option<u32> {
    let progress = progress_per_tick(tool, block, ctx);
    if progress <= 0.0 || !progress.is_finite() {
        return None;
    }
    Some((1.0 / progress).ceil().clamp(1.0, f64::from(u32::MAX)) as u32)
}
#[must_use]
pub const fn durability_cost(successful_break: bool, creative: bool) -> u32 {
    if successful_break && !creative { 1 } else { 0 }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn stone() -> BlockMining {
        BlockMining {
            hardness: 1.5,
            preferred_tool: ToolClass::Pickaxe,
            required_tier: Some(ToolTier::Wood),
            requires_correct_tool: true,
        }
    }
    #[test]
    fn pickaxe_is_faster() {
        let tool = MiningTool {
            class: ToolClass::Pickaxe,
            tier: ToolTier::Iron,
            efficiency_level: 0,
            durability: 100,
        };
        assert!(
            destroy_speed(Some(tool), stone(), MiningContext::default())
                > destroy_speed(None, stone(), MiningContext::default())
        );
    }
    #[test]
    fn air_and_water_slow_breaking() {
        let tool = MiningTool {
            class: ToolClass::Pickaxe,
            tier: ToolTier::Iron,
            efficiency_level: 0,
            durability: 100,
        };
        let normal = destroy_speed(Some(tool), stone(), MiningContext::default());
        let slow = destroy_speed(
            Some(tool),
            stone(),
            MiningContext {
                on_ground: false,
                underwater: true,
                ..MiningContext::default()
            },
        );
        assert!(slow < normal);
    }
    #[test]
    fn high_efficiency_levels_do_not_overflow() {
        let tool = MiningTool {
            class: ToolClass::Pickaxe,
            tier: ToolTier::Diamond,
            efficiency_level: u8::MAX,
            durability: 1,
        };
        let speed = destroy_speed(Some(tool), stone(), MiningContext::default());
        assert!(speed.is_finite());
        assert!(speed > 65_000.0);
    }
}
