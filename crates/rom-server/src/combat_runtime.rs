use rom_game::{GameEvent, GameMode, HOTBAR_START, PlayerUuid};

use crate::game_runtime::{GameRuntimeError, SharedGameRuntime};

pub const PLAYER_ATTACK_REACH: f64 = 3.0;
pub const PLAYER_ATTACK_KNOCKBACK: f64 = 0.4;
pub const PLAYER_ATTACK_VERTICAL_KNOCKBACK: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerAttackOutcome {
    pub target: PlayerUuid,
    pub damage: f32,
    pub killed: bool,
}

impl SharedGameRuntime {
    pub fn attack_player_entity(
        &self,
        attacker_uuid: PlayerUuid,
        protocol_entity_id: u32,
    ) -> Result<Option<PlayerAttackOutcome>, GameRuntimeError> {
        let snapshot = self.with_state(|state| {
            let attacker = state.player(attacker_uuid)?;
            if !attacker.connected
                || attacker.game_mode == GameMode::Spectator
                || attacker.vitals.is_dead()
            {
                return None;
            }
            let attacker_entity = attacker.entity_id?;
            if attacker_entity.get() == protocol_entity_id {
                return None;
            }
            let attacker_position = state.entities().get(attacker_entity)?.transform.position;
            let (target_uuid, target) = state.players().iter().find(|(_, player)| {
                player.connected
                    && player
                        .entity_id
                        .is_some_and(|entity_id| entity_id.get() == protocol_entity_id)
            })?;
            if target.abilities.invulnerable || target.vitals.is_dead() {
                return None;
            }
            let target_entity = target.entity_id?;
            let target_position = state.entities().get(target_entity)?.transform.position;
            let dx = target_position[0] - attacker_position[0];
            let dy = target_position[1] - attacker_position[1];
            let dz = target_position[2] - attacker_position[2];
            let distance_squared = dx * dx + dy * dy + dz * dz;
            if !distance_squared.is_finite()
                || distance_squared > PLAYER_ATTACK_REACH * PLAYER_ATTACK_REACH
            {
                return None;
            }
            let selected = HOTBAR_START + usize::from(attacker.inventory.selected_hotbar());
            let item = attacker
                .inventory
                .slot(selected)
                .ok()
                .flatten()
                .map(|stack| stack.item());
            Some((
                *target_uuid,
                attacker_position,
                attack_damage_for_item(item),
            ))
        })?;
        let Some((target_uuid, attacker_position, damage)) = snapshot else {
            return Ok(None);
        };

        let events = self.damage_player(target_uuid, damage)?;
        let killed = events.iter().any(
            |event| matches!(event, GameEvent::PlayerKilled { uuid, .. } if *uuid == target_uuid),
        );
        // Player entities remain authoritative through death until respawn, so velocity
        // can be replicated even for the lethal attack just like ordinary knockback.
        let _ = self.knockback_player(
            target_uuid,
            attacker_position,
            PLAYER_ATTACK_KNOCKBACK,
            PLAYER_ATTACK_VERTICAL_KNOCKBACK,
        )?;
        Ok(Some(PlayerAttackOutcome {
            target: target_uuid,
            damage,
            killed,
        }))
    }
}

#[must_use]
pub fn attack_damage_for_item(item: Option<&str>) -> f32 {
    match item {
        Some("minecraft:wooden_sword" | "minecraft:golden_sword") => 4.0,
        Some("minecraft:stone_sword") => 5.0,
        Some("minecraft:iron_sword") => 6.0,
        Some("minecraft:diamond_sword") => 7.0,
        Some("minecraft:netherite_sword") => 8.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rom_game::{ItemStack, Transform, Velocity};

    fn transform(x: f64) -> Transform {
        Transform::new([x, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn close_player_attack_applies_damage_and_knockback() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let attacker = PlayerUuid::new(100);
        let target = PlayerUuid::new(101);
        runtime
            .connect_player(attacker, "Attacker", transform(0.5))
            .unwrap();
        runtime
            .connect_player(target, "Target", transform(2.5))
            .unwrap();
        runtime
            .with_state_mut(|state| {
                let player = state.player_mut(attacker).unwrap();
                player
                    .inventory
                    .set_slot(
                        HOTBAR_START,
                        Some(ItemStack::with_max_count("minecraft:diamond_sword", 1, 1).unwrap()),
                    )
                    .map_err(|error| {
                        GameRuntimeError::State(rom_game::GameStateError::Inventory(error))
                    })?;
                Ok(())
            })
            .unwrap();
        let target_entity = runtime
            .with_state(|state| state.player(target).unwrap().entity_id.unwrap())
            .unwrap();

        let outcome = runtime
            .attack_player_entity(attacker, target_entity.get())
            .unwrap()
            .unwrap();
        assert_eq!(outcome.damage, 7.0);
        assert!(!outcome.killed);
        let (health, velocity) = runtime
            .with_state(|state| {
                let player = state.player(target).unwrap();
                let velocity = state
                    .entities()
                    .get(player.entity_id.unwrap())
                    .unwrap()
                    .velocity;
                (player.vitals.health, velocity)
            })
            .unwrap();
        assert_eq!(health, 13.0);
        assert_ne!(velocity, Velocity::default());
    }

    #[test]
    fn rejects_self_far_and_invulnerable_targets() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let attacker = PlayerUuid::new(110);
        let target = PlayerUuid::new(111);
        runtime
            .connect_player(attacker, "Attacker", transform(0.5))
            .unwrap();
        runtime
            .connect_player(target, "FarTarget", transform(10.5))
            .unwrap();
        let attacker_entity = runtime
            .with_state(|state| state.player(attacker).unwrap().entity_id.unwrap())
            .unwrap();
        let target_entity = runtime
            .with_state(|state| state.player(target).unwrap().entity_id.unwrap())
            .unwrap();
        assert!(
            runtime
                .attack_player_entity(attacker, attacker_entity.get())
                .unwrap()
                .is_none()
        );
        assert!(
            runtime
                .attack_player_entity(attacker, target_entity.get())
                .unwrap()
                .is_none()
        );

        runtime
            .with_state_mut(|state| {
                let attacker_entity = state.player(attacker).unwrap().entity_id.unwrap();
                state
                    .entities_mut()
                    .get_mut(attacker_entity)
                    .unwrap()
                    .transform = transform(9.5);
                state
                    .player_mut(target)
                    .unwrap()
                    .set_game_mode(GameMode::Creative);
                Ok(())
            })
            .unwrap();
        assert!(
            runtime
                .attack_player_entity(attacker, target_entity.get())
                .unwrap()
                .is_none()
        );
    }
}
