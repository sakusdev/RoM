//! Player fall-distance accumulation and landing damage.
//!
//! Movement remains server-authoritative: the previous transform comes from the
//! entity store, distance is accumulated in the persistent player gameplay
//! component, and landing damage is routed through `GameState::damage_player`
//! so invulnerability, death events, keepInventory, and drops stay consistent.

use crate::{fall_damage, GameEvent, GameState, GameStateError, PlayerUuid, Transform};

const DEFAULT_SAFE_FALL_DISTANCE: f32 = 3.0;
const DEFAULT_FALL_DAMAGE_MULTIPLIER: f32 = 1.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FallMovementOutcome {
    pub downward_distance: f32,
    pub accumulated_distance: f32,
    pub landed: bool,
    pub damage: f32,
}

impl GameState {
    /// Moves a player while maintaining persistent fall distance and applying
    /// landing damage through the normal damage/death path.
    pub fn move_player_with_gameplay(
        &mut self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let previous = self
            .player(uuid)
            .and_then(|player| player.entity_id)
            .and_then(|entity_id| self.entities().get(entity_id))
            .map(|entity| entity.transform)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;

        // First perform the ordinary authoritative movement validation/update.
        // Fall state is mutated only after this succeeds.
        let mut events = self.move_player(uuid, transform)?;
        let outcome = self.update_player_fall_state(uuid, previous, transform)?;

        if outcome.damage > 0.0 {
            events.extend(self.damage_player(uuid, outcome.damage)?);
        }
        Ok(events)
    }

    fn update_player_fall_state(
        &mut self,
        uuid: PlayerUuid,
        previous: Transform,
        current: Transform,
    ) -> Result<FallMovementOutcome, GameStateError> {
        let downward_distance = if current.position[1] < previous.position[1] {
            (previous.position[1] - current.position[1]).min(f64::from(f32::MAX)) as f32
        } else {
            0.0
        };
        let landed = !previous.on_ground && current.on_ground;

        let player = self
            .player_mut(uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;

        // Include the final downward segment carried by the landing packet.
        if downward_distance > 0.0 && (!previous.on_ground || !current.on_ground || landed) {
            player.gameplay.add_fall_distance(downward_distance);
        }

        if previous.on_ground && current.on_ground {
            player.gameplay.reset_fall_distance();
        }

        let accumulated_distance = player.gameplay.fall_distance();
        if !landed {
            return Ok(FallMovementOutcome {
                downward_distance,
                accumulated_distance,
                landed: false,
                damage: 0.0,
            });
        }

        let safe_fall_distance = player
            .gameplay
            .attributes
            .value("minecraft:safe_fall_distance")
            .unwrap_or(f64::from(DEFAULT_SAFE_FALL_DISTANCE))
            .clamp(0.0, f64::from(f32::MAX)) as f32;
        let multiplier = player
            .gameplay
            .attributes
            .value("minecraft:fall_damage_multiplier")
            .unwrap_or(f64::from(DEFAULT_FALL_DAMAGE_MULTIPLIER))
            .clamp(0.0, 100.0) as f32;

        player.gameplay.reset_fall_distance();
        let damage = fall_damage(accumulated_distance, safe_fall_distance, multiplier)
            .unwrap_or(0.0);

        Ok(FallMovementOutcome {
            downward_distance,
            accumulated_distance,
            landed: true,
            damage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GameMode, ItemStack};

    fn at(y: f64, on_ground: bool) -> Transform {
        Transform::new([0.5, y, 0.5], 0.0, 0.0, on_ground).unwrap()
    }

    fn state_at(y: f64) -> (GameState, PlayerUuid) {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(0xf011);
        state.connect_player(uuid, "Steve", at(y, true)).unwrap();
        (state, uuid)
    }

    #[test]
    fn short_fall_does_not_damage() {
        let (mut state, uuid) = state_at(65.0);
        state.move_player_with_gameplay(uuid, at(65.0, false)).unwrap();
        let events = state
            .move_player_with_gameplay(uuid, at(62.0, true))
            .unwrap();
        assert_eq!(state.player(uuid).unwrap().vitals.health, 20.0);
        assert!(!events.iter().any(|event| matches!(event, GameEvent::PlayerDamaged { .. })));
    }

    #[test]
    fn eight_block_fall_applies_five_damage() {
        let (mut state, uuid) = state_at(70.0);
        state.move_player_with_gameplay(uuid, at(70.0, false)).unwrap();
        let events = state
            .move_player_with_gameplay(uuid, at(62.0, true))
            .unwrap();
        assert_eq!(state.player(uuid).unwrap().vitals.health, 15.0);
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::PlayerDamaged { amount, .. } if *amount == 5.0
        )));
    }

    #[test]
    fn fall_distance_accumulates_across_airborne_packets() {
        let (mut state, uuid) = state_at(80.0);
        state.move_player_with_gameplay(uuid, at(80.0, false)).unwrap();
        state.move_player_with_gameplay(uuid, at(77.5, false)).unwrap();
        state.move_player_with_gameplay(uuid, at(74.0, false)).unwrap();
        assert_eq!(state.player(uuid).unwrap().gameplay.fall_distance(), 6.0);
        state.move_player_with_gameplay(uuid, at(72.0, true)).unwrap();
        assert_eq!(state.player(uuid).unwrap().gameplay.fall_distance(), 0.0);
        assert_eq!(state.player(uuid).unwrap().vitals.health, 15.0);
    }

    #[test]
    fn creative_uses_same_damage_path_and_stays_invulnerable() {
        let (mut state, uuid) = state_at(100.0);
        state.set_game_mode(uuid, GameMode::Creative).unwrap();
        state.move_player_with_gameplay(uuid, at(100.0, false)).unwrap();
        state.move_player_with_gameplay(uuid, at(60.0, true)).unwrap();
        assert_eq!(state.player(uuid).unwrap().vitals.health, 20.0);
        assert_eq!(state.player(uuid).unwrap().gameplay.fall_distance(), 0.0);
    }

    #[test]
    fn lethal_fall_uses_normal_death_and_drop_pipeline() {
        let (mut state, uuid) = state_at(100.0);
        state
            .give_item(uuid, ItemStack::new("minecraft:diamond", 2).unwrap())
            .unwrap();
        state.move_player_with_gameplay(uuid, at(100.0, false)).unwrap();
        let events = state
            .move_player_with_gameplay(uuid, at(60.0, true))
            .unwrap();
        assert!(state.player(uuid).unwrap().vitals.is_dead());
        assert!(events.iter().any(|event| matches!(event, GameEvent::PlayerKilled { .. })));
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::ItemsDropped { stacks, .. } if !stacks.is_empty()
        )));
    }

    #[test]
    fn ground_motion_clears_stale_fall_distance() {
        let (mut state, uuid) = state_at(65.0);
        state.player_mut(uuid).unwrap().gameplay.add_fall_distance(12.0);
        state.move_player_with_gameplay(uuid, at(65.0, true)).unwrap();
        assert_eq!(state.player(uuid).unwrap().gameplay.fall_distance(), 0.0);
    }
}
