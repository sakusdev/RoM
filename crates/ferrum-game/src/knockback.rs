//! Authoritative knockback application for player entities.

use crate::{EntityId, GameState, GameStateError, PlayerUuid, Velocity, knockback_vector};

pub const MAX_KNOCKBACK_SPEED: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KnockbackOutcome {
    pub entity_id: EntityId,
    pub resistance: f64,
    pub impulse: [f64; 3],
    pub previous_velocity: Velocity,
    pub current_velocity: Velocity,
}

impl GameState {
    /// Applies an impulse away from the attacker to the authoritative player
    /// entity. Knockback resistance comes from the player's attribute map.
    pub fn knockback_player(
        &mut self,
        uuid: PlayerUuid,
        attacker_position: [f64; 3],
        horizontal_strength: f64,
        vertical_strength: f64,
    ) -> Result<KnockbackOutcome, GameStateError> {
        let (entity_id, victim_position, resistance) = {
            let player = self
                .player(uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if !player.connected {
                return Err(GameStateError::PlayerNotConnected { uuid });
            }
            let entity_id = player
                .entity_id
                .ok_or(GameStateError::PlayerMissingEntity { uuid })?;
            let entity = self
                .entities()
                .get(entity_id)
                .ok_or(crate::EntityError::UnknownEntity { id: entity_id })?;
            let resistance = player
                .gameplay
                .attributes
                .value("minecraft:knockback_resistance")
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            (entity_id, entity.transform.position, resistance)
        };

        let impulse = knockback_vector(
            attacker_position,
            victim_position,
            horizontal_strength,
            vertical_strength,
            resistance,
        );
        let previous_velocity = self
            .entities()
            .get(entity_id)
            .ok_or(crate::EntityError::UnknownEntity { id: entity_id })?
            .velocity;
        let current_velocity = Velocity::new([
            (previous_velocity.0[0] + impulse[0]).clamp(-MAX_KNOCKBACK_SPEED, MAX_KNOCKBACK_SPEED),
            (previous_velocity.0[1] + impulse[1]).clamp(-MAX_KNOCKBACK_SPEED, MAX_KNOCKBACK_SPEED),
            (previous_velocity.0[2] + impulse[2]).clamp(-MAX_KNOCKBACK_SPEED, MAX_KNOCKBACK_SPEED),
        ])?;
        self.entities_mut()
            .set_velocity(entity_id, current_velocity)?;

        Ok(KnockbackOutcome {
            entity_id,
            resistance,
            impulse,
            previous_velocity,
            current_velocity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, AttributeModifier, AttributeOperation, Transform};

    fn spawn() -> Transform {
        Transform::new([4.0, 65.0, 0.0], 0.0, 0.0, true).unwrap()
    }

    fn state() -> (GameState, PlayerUuid) {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(0x4b);
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        (state, uuid)
    }

    #[test]
    fn pushes_away_from_attacker() {
        let (mut state, uuid) = state();
        let outcome = state
            .knockback_player(uuid, [0.0, 65.0, 0.0], 0.4, 0.4)
            .unwrap();
        assert!(outcome.current_velocity.0[0] > 0.0);
        assert_eq!(outcome.current_velocity.0[1], 0.4);
        assert_eq!(outcome.current_velocity.0[2], 0.0);
    }

    #[test]
    fn resistance_scales_horizontal_impulse() {
        let (mut state, uuid) = state();
        state
            .player_mut(uuid)
            .unwrap()
            .gameplay
            .attributes
            .insert(Attribute::new("minecraft:knockback_resistance", 0.75, 0.0, 1.0).unwrap());
        let outcome = state
            .knockback_player(uuid, [0.0, 65.0, 0.0], 0.4, 0.4)
            .unwrap();
        assert!((outcome.impulse[0] - 0.1).abs() < 1.0e-9);
    }

    #[test]
    fn repeated_impulses_are_speed_bounded() {
        let (mut state, uuid) = state();
        for _ in 0..100 {
            state
                .knockback_player(uuid, [0.0, 65.0, 0.0], 2.0, 2.0)
                .unwrap();
        }
        let entity_id = state.player(uuid).unwrap().entity_id.unwrap();
        let velocity = state.entities().get(entity_id).unwrap().velocity;
        assert!(
            velocity
                .0
                .into_iter()
                .all(|value| value.abs() <= MAX_KNOCKBACK_SPEED)
        );
    }

    #[test]
    fn attribute_modifiers_feed_resistance_value() {
        let (mut state, uuid) = state();
        let attribute = state
            .player_mut(uuid)
            .unwrap()
            .gameplay
            .attributes
            .get_mut("minecraft:knockback_resistance")
            .unwrap();
        attribute.insert_modifier(
            AttributeModifier::new("rom:test_resistance", 0.5, AttributeOperation::AddValue)
                .unwrap(),
        );
        let outcome = state
            .knockback_player(uuid, [0.0, 65.0, 0.0], 1.0, 0.0)
            .unwrap();
        assert!((outcome.resistance - 0.5).abs() < 1.0e-9);
    }
}
