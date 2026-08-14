use rom_game::{
    BlockMining, BlockPos, DurabilityError, EFFICIENCY_ENCHANTMENT, GameEvent, GameMode,
    GameStateError, HOTBAR_START, MAX_MINING_TICKS, MAX_UNBREAKING_LEVEL, MiningContext,
    MiningSessionError, MiningTool, PlayerUuid, ToolClass, ToolTier, UNBREAKING_ENCHANTMENT,
    correct_tool, damage_item, item_enchantment_level, ticks_to_break, vanilla_max_durability,
};
use rom_pack::RomPackWorld;
use rom_world::BlockStateId;
use thiserror::Error;

use crate::game_runtime::{GameRuntimeError, SharedGameRuntime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MiningStart {
    pub required_ticks: u32,
    pub started_at_tick: u64,
}

#[derive(Debug, Error)]
pub enum MiningRuntimeError {
    #[error(transparent)]
    Runtime(#[from] GameRuntimeError),
    #[error(transparent)]
    Session(#[from] MiningSessionError),
    #[error(transparent)]
    Durability(#[from] DurabilityError),
}

impl SharedGameRuntime {
    pub fn selected_mining_tool(
        &self,
        uuid: PlayerUuid,
    ) -> Result<Option<MiningTool>, MiningRuntimeError> {
        let selected = self.selected_item(uuid)?;
        Ok(selected.and_then(|selected| mining_tool_from_stack(&selected.stack)))
    }

    pub fn mining_context(&self, uuid: PlayerUuid) -> Result<MiningContext, MiningRuntimeError> {
        self.with_state(|state| {
            let player = state.player(uuid)?;
            let on_ground = player
                .entity_id
                .and_then(|entity_id| state.entities().get(entity_id))
                .is_some_and(|entity| entity.transform.on_ground);
            Some(MiningContext {
                on_ground,
                underwater: false,
                haste: player.gameplay.mining_haste_multiplier(),
                fatigue: 1.0,
            })
        })?
        .ok_or_else(|| GameRuntimeError::State(GameStateError::UnknownPlayer { uuid }).into())
    }

    pub fn can_harvest_block(
        &self,
        uuid: PlayerUuid,
        block: BlockMining,
    ) -> Result<bool, MiningRuntimeError> {
        let mode = self
            .with_state(|state| state.player(uuid).map(|player| player.game_mode))?
            .ok_or(GameRuntimeError::State(GameStateError::UnknownPlayer {
                uuid,
            }))?;
        if mode == GameMode::Creative {
            return Ok(true);
        }
        Ok(correct_tool(self.selected_mining_tool(uuid)?, block))
    }

    pub fn begin_mining(
        &self,
        uuid: PlayerUuid,
        position: BlockPos,
        target_token: u64,
        block: BlockMining,
    ) -> Result<Option<MiningStart>, MiningRuntimeError> {
        let tool = self.selected_mining_tool(uuid)?;
        let context = self.mining_context(uuid)?;
        let game_mode = self
            .with_state(|state| state.player(uuid).map(|player| player.game_mode))?
            .ok_or(GameRuntimeError::State(GameStateError::UnknownPlayer {
                uuid,
            }))?;

        if game_mode == GameMode::Spectator || block.hardness < 0.0 {
            return Ok(None);
        }
        let required_ticks = if game_mode == GameMode::Creative {
            1
        } else {
            let Some(required_ticks) = ticks_to_break(tool, block, context) else {
                return Ok(None);
            };
            required_ticks.min(MAX_MINING_TICKS)
        };
        let started_at_tick = self.with_state(|state| state.time().game_time)?;
        let session_result = self.with_state_mut(|state| {
            let player = state.player_mut(uuid).ok_or(GameRuntimeError::State(
                GameStateError::UnknownPlayer { uuid },
            ))?;
            Ok(player
                .gameplay
                .begin_mining(position, target_token, started_at_tick, required_ticks)
                .map(|_| ()))
        })?;
        session_result?;
        Ok(Some(MiningStart {
            required_ticks,
            started_at_tick,
        }))
    }

    pub fn abort_mining(
        &self,
        uuid: PlayerUuid,
        position: BlockPos,
    ) -> Result<bool, MiningRuntimeError> {
        Ok(self.with_state_mut(|state| {
            let player = state.player_mut(uuid).ok_or(GameRuntimeError::State(
                GameStateError::UnknownPlayer { uuid },
            ))?;
            Ok(player.gameplay.abort_mining(position))
        })?)
    }

    pub fn finish_mining(
        &self,
        uuid: PlayerUuid,
        position: BlockPos,
        target_token: u64,
    ) -> Result<bool, MiningRuntimeError> {
        let current_tick = self.with_state(|state| state.time().game_time)?;
        let outcome = self.with_state_mut(|state| {
            let player = state.player_mut(uuid).ok_or(GameRuntimeError::State(
                GameStateError::UnknownPlayer { uuid },
            ))?;
            Ok(player
                .gameplay
                .finish_mining(position, target_token, current_tick))
        })?;
        match outcome {
            Ok(_) => Ok(true),
            Err(
                MiningSessionError::NoActiveSession
                | MiningSessionError::WrongTarget { .. }
                | MiningSessionError::TargetChanged { .. }
                | MiningSessionError::TooEarly { .. },
            ) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn damage_selected_tool_after_break(
        &self,
        uuid: PlayerUuid,
        seed: u64,
    ) -> Result<Vec<GameEvent>, MiningRuntimeError> {
        let Some(selected) = self.selected_item(uuid)? else {
            return Ok(Vec::new());
        };
        if selected.game_mode == GameMode::Creative {
            return Ok(Vec::new());
        }
        let Some(max_damage) = vanilla_max_durability(selected.stack.item()) else {
            return Ok(Vec::new());
        };
        let unbreaking_level = item_enchantment_level(&selected.stack, UNBREAKING_ENCHANTMENT)
            .min(MAX_UNBREAKING_LEVEL);
        let result = damage_item(&selected.stack, max_damage, 1, unbreaking_level, seed)?;
        let events = self.with_state_mut(|state| {
            let player = state.player_mut(uuid).ok_or(GameRuntimeError::State(
                GameStateError::UnknownPlayer { uuid },
            ))?;
            let slot = HOTBAR_START + usize::from(player.inventory.selected_hotbar());
            player
                .inventory
                .set_slot(slot, result.stack.clone())
                .map_err(|error| GameRuntimeError::State(GameStateError::Inventory(error)))?;
            Ok(vec![
                GameEvent::InventorySlotChanged {
                    uuid,
                    slot,
                    stack: result.stack.clone(),
                },
                GameEvent::ContainerContentChanged {
                    uuid,
                    snapshot: player.inventory_session.snapshot(&player.inventory),
                },
            ])
        })?;
        self.publish(&events)?;
        Ok(events)
    }
}

#[must_use]
pub fn mining_properties_for_state(
    state: BlockStateId,
    world: &RomPackWorld,
) -> Option<BlockMining> {
    let raw = state.get();
    if raw == world.block_states.air {
        return None;
    }
    if raw == world.block_states.bedrock {
        return Some(BlockMining {
            hardness: -1.0,
            preferred_tool: ToolClass::Pickaxe,
            required_tier: None,
            requires_correct_tool: true,
        });
    }
    if raw == world.block_states.stone {
        return Some(BlockMining {
            hardness: 1.5,
            preferred_tool: ToolClass::Pickaxe,
            required_tier: Some(ToolTier::Wood),
            requires_correct_tool: true,
        });
    }
    if raw == world.block_states.dirt {
        return Some(BlockMining {
            hardness: 0.5,
            preferred_tool: ToolClass::Shovel,
            required_tier: None,
            requires_correct_tool: false,
        });
    }
    if raw == world.block_states.grass {
        return Some(BlockMining {
            hardness: 0.6,
            preferred_tool: ToolClass::Shovel,
            required_tier: None,
            requires_correct_tool: false,
        });
    }
    Some(BlockMining {
        hardness: 1.0,
        preferred_tool: ToolClass::None,
        required_tier: None,
        requires_correct_tool: false,
    })
}

#[must_use]
pub fn mining_tool_from_stack(stack: &rom_game::ItemStack) -> Option<MiningTool> {
    let mut tool = mining_tool_from_item(stack.item())?;
    tool.efficiency_level = item_enchantment_level(stack, EFFICIENCY_ENCHANTMENT);
    Some(tool)
}

#[must_use]
pub fn mining_tool_from_item(item: &str) -> Option<MiningTool> {
    let name = item.strip_prefix("minecraft:")?;
    let (tier, suffix) = if let Some(suffix) = name.strip_prefix("wooden_") {
        (ToolTier::Wood, suffix)
    } else if let Some(suffix) = name.strip_prefix("golden_") {
        (ToolTier::Gold, suffix)
    } else if let Some(suffix) = name.strip_prefix("stone_") {
        (ToolTier::Stone, suffix)
    } else if let Some(suffix) = name.strip_prefix("iron_") {
        (ToolTier::Iron, suffix)
    } else if let Some(suffix) = name.strip_prefix("diamond_") {
        (ToolTier::Diamond, suffix)
    } else {
        (ToolTier::Netherite, name.strip_prefix("netherite_")?)
    };
    let class = match suffix {
        "pickaxe" => ToolClass::Pickaxe,
        "axe" => ToolClass::Axe,
        "shovel" => ToolClass::Shovel,
        "hoe" => ToolClass::Hoe,
        "sword" => ToolClass::Sword,
        _ => return None,
    };
    let max_damage = vanilla_max_durability(item)?;
    Some(MiningTool {
        class,
        tier,
        efficiency_level: 0,
        durability: max_damage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rom_game::{ItemStack, Transform, item_damage};
    use rom_pack::{RomPackBiomes, RomPackBlockStates};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    fn test_world() -> RomPackWorld {
        RomPackWorld {
            data_version: 0,
            overworld_min_section_y: -4,
            overworld_section_count: 24,
            dimension: "minecraft:overworld".to_owned(),
            dimension_type_id: 0,
            sea_level: 63,
            floor_y: 63,
            spawn_x: 0,
            spawn_z: 0,
            block_states: RomPackBlockStates {
                air: 0,
                stone: 1,
                grass: 9,
                dirt: 10,
                bedrock: 85,
            },
            biomes: RomPackBiomes { plains: 40 },
        }
    }

    #[test]
    fn block_profiles_are_server_domain_data() {
        let world = test_world();
        assert!(mining_properties_for_state(BlockStateId::new(0), &world).is_none());
        let stone = mining_properties_for_state(BlockStateId::new(1), &world).unwrap();
        assert_eq!(stone.hardness, 1.5);
        assert_eq!(stone.preferred_tool, ToolClass::Pickaxe);
        assert_eq!(stone.required_tier, Some(ToolTier::Wood));
        assert!(stone.requires_correct_tool);
        let bedrock = mining_properties_for_state(BlockStateId::new(85), &world).unwrap();
        assert!(bedrock.hardness < 0.0);
    }

    #[test]
    fn maps_vanilla_tool_names() {
        let tool = mining_tool_from_item("minecraft:diamond_pickaxe").unwrap();
        assert_eq!(tool.class, ToolClass::Pickaxe);
        assert_eq!(tool.tier, ToolTier::Diamond);
        assert!(mining_tool_from_item("minecraft:stick").is_none());
    }

    #[test]
    fn semantic_efficiency_component_changes_selected_mining_tool() {
        let stack = ItemStack::with_max_count("minecraft:diamond_pickaxe", 1, 1)
            .unwrap()
            .with_component(
                rom_game::ENCHANTMENTS_COMPONENT,
                serde_json::json!({
                    "wire_hex": "00",
                    "levels": {rom_game::EFFICIENCY_ENCHANTMENT: 5}
                }),
            );
        let tool = mining_tool_from_stack(&stack).unwrap();
        assert_eq!(tool.efficiency_level, 5);
    }

    #[test]
    fn server_time_rejects_early_finish() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(44);
        runtime.connect_player(uuid, "Miner", spawn()).unwrap();
        let block = BlockMining {
            hardness: 1.5,
            preferred_tool: ToolClass::Pickaxe,
            required_tier: Some(ToolTier::Wood),
            requires_correct_tool: true,
        };
        runtime
            .begin_mining(uuid, BlockPos { x: 0, y: 64, z: 0 }, 1, block)
            .unwrap()
            .unwrap();
        assert!(
            !runtime
                .finish_mining(uuid, BlockPos { x: 0, y: 64, z: 0 }, 1)
                .unwrap()
        );
    }

    #[test]
    fn stone_requires_a_pickaxe_to_harvest() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(46);
        runtime.connect_player(uuid, "Miner", spawn()).unwrap();
        let stone = BlockMining {
            hardness: 1.5,
            preferred_tool: ToolClass::Pickaxe,
            required_tier: Some(ToolTier::Wood),
            requires_correct_tool: true,
        };
        assert!(!runtime.can_harvest_block(uuid, stone).unwrap());
    }

    #[test]
    fn successful_break_damages_selected_tool() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(45);
        runtime.connect_player(uuid, "Miner", spawn()).unwrap();
        runtime
            .with_state_mut(|state| {
                let player = state.player_mut(uuid).unwrap();
                player
                    .inventory
                    .set_slot(
                        HOTBAR_START,
                        Some(ItemStack::with_max_count("minecraft:iron_pickaxe", 1, 1).unwrap()),
                    )
                    .map_err(|error| GameRuntimeError::State(GameStateError::Inventory(error)))?;
                Ok(())
            })
            .unwrap();
        runtime.damage_selected_tool_after_break(uuid, 7).unwrap();
        let selected = runtime.selected_item(uuid).unwrap().unwrap();
        assert_eq!(item_damage(&selected.stack), 1);
    }
}
