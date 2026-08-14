from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise RuntimeError(f"marker not found in {path}: {old[:100]!r}")
    if text.count(old) != 1:
        raise RuntimeError(f"marker not unique in {path}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


mining = ROOT / "crates/rom-server/src/mining_runtime.rs"
replace_once(
    mining,
    "use thiserror::Error;\n\nuse crate::game_runtime::{GameRuntimeError, SharedGameRuntime};",
    "use rom_pack::RomPackWorld;\nuse rom_world::BlockStateId;\nuse thiserror::Error;\n\nuse crate::game_runtime::{GameRuntimeError, SharedGameRuntime};",
)
marker = '''#[must_use]\npub fn mining_tool_from_item(item: &str) -> Option<MiningTool> {'''
profile_fn = '''#[must_use]\npub fn mining_properties_for_state(\n    state: BlockStateId,\n    world: &RomPackWorld,\n) -> Option<BlockMining> {\n    let raw = state.get();\n    if raw == world.block_states.air {\n        return None;\n    }\n    if raw == world.block_states.bedrock {\n        return Some(BlockMining {\n            hardness: -1.0,\n            preferred_tool: ToolClass::Pickaxe,\n            required_tier: None,\n            requires_correct_tool: true,\n        });\n    }\n    if raw == world.block_states.stone {\n        return Some(BlockMining {\n            hardness: 1.5,\n            preferred_tool: ToolClass::Pickaxe,\n            required_tier: Some(ToolTier::Wood),\n            requires_correct_tool: true,\n        });\n    }\n    if raw == world.block_states.dirt {\n        return Some(BlockMining {\n            hardness: 0.5,\n            preferred_tool: ToolClass::Shovel,\n            required_tier: None,\n            requires_correct_tool: false,\n        });\n    }\n    if raw == world.block_states.grass {\n        return Some(BlockMining {\n            hardness: 0.6,\n            preferred_tool: ToolClass::Shovel,\n            required_tier: None,\n            requires_correct_tool: false,\n        });\n    }\n    Some(BlockMining {\n        hardness: 1.0,\n        preferred_tool: ToolClass::None,\n        required_tier: None,\n        requires_correct_tool: false,\n    })\n}\n\n#[must_use]\npub fn mining_tool_from_item(item: &str) -> Option<MiningTool> {'''
replace_once(mining, marker, profile_fn)
replace_once(
    mining,
    "    use rom_game::{ItemStack, Transform, item_damage};",
    "    use rom_game::{ItemStack, Transform, item_damage};\n    use rom_pack::{RomPackBiomes, RomPackBlockStates};",
)
insert_before_test = '''    #[test]\n    fn maps_vanilla_tool_names() {'''
new_tests = '''    fn test_world() -> RomPackWorld {\n        RomPackWorld {\n            data_version: 0,\n            overworld_min_section_y: -4,\n            overworld_section_count: 24,\n            dimension: "minecraft:overworld".to_owned(),\n            dimension_type_id: 0,\n            sea_level: 63,\n            floor_y: 63,\n            spawn_x: 0,\n            spawn_z: 0,\n            block_states: RomPackBlockStates {\n                air: 0,\n                stone: 1,\n                grass: 9,\n                dirt: 10,\n                bedrock: 85,\n            },\n            biomes: RomPackBiomes { plains: 40 },\n        }\n    }\n\n    #[test]\n    fn block_profiles_are_server_domain_data() {\n        let world = test_world();\n        assert!(mining_properties_for_state(BlockStateId::new(0), &world).is_none());\n        let stone = mining_properties_for_state(BlockStateId::new(1), &world).unwrap();\n        assert_eq!(stone.hardness, 1.5);\n        assert_eq!(stone.preferred_tool, ToolClass::Pickaxe);\n        assert_eq!(stone.required_tier, Some(ToolTier::Wood));\n        assert!(stone.requires_correct_tool);\n        let bedrock = mining_properties_for_state(BlockStateId::new(85), &world).unwrap();\n        assert!(bedrock.hardness < 0.0);\n    }\n\n    #[test]\n    fn maps_vanilla_tool_names() {'''
replace_once(mining, insert_before_test, new_tests)

play = ROOT / "crates/rom-server/src/play_runtime.rs"
replace_once(
    play,
    "use rom_game::{\n    BlockMining, BlockPos as GameBlockPos, CommandSource, GameEvent, PlayerUuid as GamePlayerUuid,\n    ToolClass, ToolTier, Transform,\n};",
    "use rom_game::{\n    BlockPos as GameBlockPos, CommandSource, GameEvent, PlayerUuid as GamePlayerUuid, Transform,\n};",
)
old_fn = '''fn mining_properties_for_state(state: BlockStateId, world: &RomPackWorld) -> Option<BlockMining> {\n    let raw = state.get();\n    if raw == world.block_states.air {\n        return None;\n    }\n    if raw == world.block_states.bedrock {\n        return Some(BlockMining {\n            hardness: -1.0,\n            preferred_tool: ToolClass::Pickaxe,\n            required_tier: None,\n            requires_correct_tool: true,\n        });\n    }\n    if raw == world.block_states.stone {\n        return Some(BlockMining {\n            hardness: 1.5,\n            preferred_tool: ToolClass::Pickaxe,\n            required_tier: Some(ToolTier::Wood),\n            requires_correct_tool: true,\n        });\n    }\n    if raw == world.block_states.dirt {\n        return Some(BlockMining {\n            hardness: 0.5,\n            preferred_tool: ToolClass::Shovel,\n            required_tier: None,\n            requires_correct_tool: false,\n        });\n    }\n    if raw == world.block_states.grass {\n        return Some(BlockMining {\n            hardness: 0.6,\n            preferred_tool: ToolClass::Shovel,\n            required_tier: None,\n            requires_correct_tool: false,\n        });\n    }\n    Some(BlockMining {\n        hardness: 1.0,\n        preferred_tool: ToolClass::None,\n        required_tier: None,\n        requires_correct_tool: false,\n    })\n}\n\n'''
replace_once(play, old_fn, "")
replace_once(
    play,
    "    game_runtime::SharedGameRuntime,\n    play_connection::PlayReaderEndpoint,",
    "    game_runtime::SharedGameRuntime,\n    mining_runtime::mining_properties_for_state,\n    play_connection::PlayReaderEndpoint,",
)

print("Moved block mining profiles out of the protocol loop.")
