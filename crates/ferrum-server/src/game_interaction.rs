use ferrum_game::{
    EntityId, EntityUuid, GameEvent, GameMode, GameStateError, GameplayTickError, HOTBAR_START,
    ItemStack, PlayerUuid, Transform, Velocity, spawn_item_entity,
};

use crate::game_runtime::{GameRuntimeError, SharedGameRuntime};

const BLOCK_DROP_PICKUP_DELAY_TICKS: u16 = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedItemSnapshot {
    pub slot: usize,
    pub stack: ItemStack,
    pub game_mode: GameMode,
}

impl SharedGameRuntime {
    pub fn selected_item(
        &self,
        uuid: PlayerUuid,
    ) -> Result<Option<SelectedItemSnapshot>, GameRuntimeError> {
        self.with_state(|state| {
            let Some(player) = state.player(uuid) else {
                return None;
            };
            let slot = HOTBAR_START + usize::from(player.inventory.selected_hotbar());
            player
                .inventory
                .selected_stack()
                .cloned()
                .map(|stack| SelectedItemSnapshot {
                    slot,
                    stack,
                    game_mode: player.game_mode,
                })
        })
    }

    /// Consumes items from the selected hotbar stack and immediately publishes
    /// the authoritative inventory delta. Creative players never consume items.
    pub fn consume_selected_item(
        &self,
        uuid: PlayerUuid,
        count: u32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let events = self.with_state_mut(|state| {
            let player = state
                .player_mut(uuid)
                .ok_or_else(|| GameRuntimeError::State(GameStateError::UnknownPlayer { uuid }))?;
            if player.game_mode == GameMode::Creative {
                return Ok(Vec::new());
            }

            let slot = HOTBAR_START + usize::from(player.inventory.selected_hotbar());
            let Some(stack) = player
                .inventory
                .slot(slot)
                .map_err(|error| GameRuntimeError::State(GameStateError::Inventory(error)))?
                .cloned()
            else {
                return Ok(Vec::new());
            };
            if stack.count() < count {
                return Ok(Vec::new());
            }

            let replacement = if stack.count() == count {
                None
            } else {
                Some(
                    stack
                        .copy_with_count(stack.count() - count)
                        .map_err(|error| {
                            GameRuntimeError::State(GameStateError::Inventory(error))
                        })?,
                )
            };
            player
                .inventory
                .set_slot(slot, replacement.clone())
                .map_err(|error| GameRuntimeError::State(GameStateError::Inventory(error)))?;

            Ok(vec![
                GameEvent::InventorySlotChanged {
                    uuid,
                    slot,
                    stack: replacement,
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

    /// Checks whether the currently selected stack can satisfy a placement
    /// consumption without mutating state.
    pub fn can_consume_selected_item(
        &self,
        uuid: PlayerUuid,
        count: u32,
    ) -> Result<bool, GameRuntimeError> {
        if count == 0 {
            return Ok(true);
        }
        Ok(self.selected_item(uuid)?.is_some_and(|selected| {
            selected.game_mode == GameMode::Creative || selected.stack.count() >= count
        }))
    }

    /// Creates a real item entity at a block position. This is used by block
    /// breaking so drops enter the same merge/pickup/lifetime simulation as
    /// death and container drops.
    pub fn spawn_world_item(
        &self,
        source: PlayerUuid,
        position: [f64; 3],
        stack: ItemStack,
    ) -> Result<EntityId, GameRuntimeError> {
        self.with_state_mut(|state| {
            let mut candidate = mix_world_item_uuid(
                source.get(),
                state.time().game_time,
                position,
                stack.item(),
            );
            loop {
                let uuid = EntityUuid::new(candidate);
                if state.entities().id_by_uuid(uuid).is_none() {
                    let transform = Transform::new(
                        [position[0] + 0.5, position[1] + 0.35, position[2] + 0.5],
                        0.0,
                        0.0,
                        false,
                    )
                    .map_err(|error| {
                        GameRuntimeError::Gameplay(GameplayTickError::Entity(error))
                    })?;
                    let velocity = Velocity::new([0.0, 0.18, 0.0]).map_err(|error| {
                        GameRuntimeError::Gameplay(GameplayTickError::Entity(error))
                    })?;
                    return spawn_item_entity(
                        state.entities_mut(),
                        uuid,
                        transform,
                        velocity,
                        stack,
                        BLOCK_DROP_PICKUP_DELAY_TICKS,
                    )
                    .map_err(|error| {
                        GameRuntimeError::Gameplay(GameplayTickError::ItemEntity(error))
                    });
                }
                candidate = candidate.wrapping_add(0x9e37_79b9_7f4a_7c15_6a09_e667_f3bc_c909);
            }
        })
    }
}

fn mix_world_item_uuid(source: u128, game_time: u64, position: [f64; 3], item: &str) -> u128 {
    let mut value = source ^ (u128::from(game_time) << 64);
    for coordinate in position {
        value ^= u128::from(coordinate.to_bits());
        value = value.rotate_left(23).wrapping_mul(0x100000001b3);
    }
    for byte in item.bytes() {
        value ^= u128::from(byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    value ^ (value >> 47)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_game::{PlayerUuid, Transform, item_entity_data};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn survival_placement_consumes_selected_stack() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(1);
        runtime.connect_player(uuid, "Steve", spawn()).unwrap();
        runtime
            .with_state_mut(|state| {
                state.give_item(uuid, ItemStack::new("minecraft:stone", 2).unwrap())?;
                state.select_hotbar(uuid, 0)?;
                let player = state.player_mut(uuid).unwrap();
                player.inventory.swap_slots(9, HOTBAR_START)?;
                Ok(())
            })
            .unwrap();

        runtime.consume_selected_item(uuid, 1).unwrap();
        let remaining = runtime.selected_item(uuid).unwrap().unwrap().stack.count();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn creative_placement_does_not_consume() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(2);
        runtime.connect_player(uuid, "Alex", spawn()).unwrap();
        runtime
            .with_state_mut(|state| {
                state.set_game_mode(uuid, GameMode::Creative)?;
                let player = state.player_mut(uuid).unwrap();
                player.inventory.set_slot(
                    HOTBAR_START,
                    Some(ItemStack::new("minecraft:stone", 1).unwrap()),
                )?;
                Ok(())
            })
            .unwrap();

        runtime.consume_selected_item(uuid, 1).unwrap();
        assert_eq!(
            runtime.selected_item(uuid).unwrap().unwrap().stack.count(),
            1
        );
    }

    #[test]
    fn block_drop_enters_world_item_simulation() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(3);
        runtime.connect_player(uuid, "Notch", spawn()).unwrap();
        let entity_id = runtime
            .spawn_world_item(
                uuid,
                [10.0, 64.0, -3.0],
                ItemStack::new("minecraft:cobblestone", 1).unwrap(),
            )
            .unwrap();
        let data = runtime
            .with_state(|state| item_entity_data(state.entities(), entity_id).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(data.stack.item(), "minecraft:cobblestone");
    }
}
