use ferrum_game::{GameEvent, GameMode, GameStateError, HOTBAR_START, ItemStack, PlayerUuid};

use crate::game_runtime::{GameRuntimeError, SharedGameRuntime};

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_game::{PlayerUuid, Transform};

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
}
