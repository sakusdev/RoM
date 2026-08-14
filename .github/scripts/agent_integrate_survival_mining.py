from pathlib import Path

path = Path("crates/ferrum-server/src/play_runtime.rs")
text = path.read_text()

old_import = "use ferrum_game::{CommandSource, GameEvent, PlayerUuid as GamePlayerUuid, Transform};"
new_import = """use ferrum_game::{
    BlockMining, BlockPos as GameBlockPos, CommandSource, GameEvent, PlayerUuid as GamePlayerUuid,
    ToolClass, ToolTier, Transform,
};"""
if old_import in text:
    text = text.replace(old_import, new_import, 1)
elif new_import not in text:
    raise SystemExit("ferrum_game import marker not found")

play_marker = "use ferrum_protocol::{"
status_import = "use ferrum_play::PlayerActionStatus;\n"
if status_import not in text:
    if play_marker not in text:
        raise SystemExit("ferrum_play status import marker not found")
    text = text.replace(play_marker, status_import + play_marker, 1)

type_marker = "type LocalWorldRuntime = DeterministicRuntime<ChunkStore, WorldEvent>;\n"
helper = '''

fn mining_properties_for_state(state: BlockStateId, world: &RomPackWorld) -> Option<BlockMining> {
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
'''
if "fn mining_properties_for_state(" not in text:
    if type_marker not in text:
        raise SystemExit("local runtime type marker not found")
    text = text.replace(type_marker, type_marker + helper, 1)

start = text.index("                Some(PacketKind::PlayerAction) => {")
end = text.index("                Some(PacketKind::UseItemOn) => {", start)
replacement = '''                Some(PacketKind::PlayerAction) => {
                    let action = decode_player_action(packet_reader.take_remaining())?;
                    let sequence = action.sequence;
                    let in_reach = is_block_interaction_within_reach(&player, action.position);
                    let game_position = GameBlockPos {
                        x: action.position.x,
                        y: action.position.y,
                        z: action.position.z,
                    };
                    let broken_state = if in_reach {
                        shared_world.interaction_block_state(BlockPos {
                            x: action.position.x,
                            y: action.position.y,
                            z: action.position.z,
                        })?
                    } else {
                        None
                    };
                    let block = broken_state.and_then(|state| {
                        mining_properties_for_state(state, shared_world.world_profile())
                    });

                    match action.status {
                        PlayerActionStatus::StartDestroyBlock => {
                            if in_reach
                                && let Some(gameplay) = gameplay
                                && let Some(block) = block
                            {
                                let _ = gameplay.runtime.begin_mining(
                                    gameplay.player_uuid,
                                    game_position,
                                    block,
                                )?;
                            }
                        }
                        PlayerActionStatus::AbortDestroyBlock => {
                            if let Some(gameplay) = gameplay {
                                let _ = gameplay
                                    .runtime
                                    .abort_mining(gameplay.player_uuid, game_position)?;
                            }
                        }
                        PlayerActionStatus::StopDestroyBlock => {
                            let completed = if let Some(gameplay) = gameplay {
                                gameplay
                                    .runtime
                                    .finish_mining(gameplay.player_uuid, game_position)?
                            } else {
                                true
                            };
                            if completed
                                && in_reach
                                && let Some(event) = player_action_to_world_event(
                                    action,
                                    BlockStateId::new(shared_world.world_profile().block_states.air),
                                )
                                && is_break_target_mutable(shared_world, event)?
                            {
                                let harvestable = if let (Some(gameplay), Some(block)) =
                                    (gameplay, block)
                                {
                                    gameplay
                                        .runtime
                                        .can_harvest_block(gameplay.player_uuid, block)?
                                } else {
                                    true
                                };
                                let applied = shared_world.apply_event(connection, event)?;
                                send_world_updates(writer, profile, &applied, play_reader)?;
                                if !applied.is_empty()
                                    && let Some(gameplay) = gameplay
                                {
                                    if harvestable
                                        && let Some(state) = broken_state
                                    {
                                        gameplay.spawn_block_drop(
                                            action.position,
                                            state,
                                            shared_world.world_profile(),
                                        )?;
                                    }
                                    let seed = gameplay.runtime.with_state(|state| {
                                        state.time().game_time
                                            ^ u64::from(action.sequence.unsigned_abs())
                                    })?;
                                    let _ = gameplay
                                        .runtime
                                        .damage_selected_tool_after_break(gameplay.player_uuid, seed)?;
                                }
                            }
                        }
                        PlayerActionStatus::DropAllItems
                        | PlayerActionStatus::DropItem
                        | PlayerActionStatus::ReleaseUseItem
                        | PlayerActionStatus::SwapItemWithOffhand
                        | PlayerActionStatus::Stab => {}
                    }
                    send_block_changed_ack(writer, profile, sequence, play_reader)?;
                }
'''
text = text[:start] + replacement + text[end:]
path.write_text(text)
