//! Per-player authoritative gameplay ticking.
//!
//! This layer advances persistent player components without coupling them to
//! packet encoding. Health changes are emitted as regular game events so the
//! server replicator can reuse the same paths as commands and combat.

use crate::{Difficulty, GameEvent, GameRuleValue, GameState, PlayerUuid, Vitals};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerTickStats {
    pub players_ticked: usize,
    pub effects_expired: usize,
    pub natural_regenerations: usize,
    pub starvation_hits: usize,
}

impl GameState {
    /// Advances persistent gameplay systems for all connected players.
    ///
    /// This method intentionally does not advance world time or entity age. It
    /// is called exactly once from `tick_gameplay` after the world tick starts.
    pub(crate) fn tick_player_gameplay(&mut self, events: &mut Vec<GameEvent>) -> PlayerTickStats {
        let natural_regeneration = matches!(
            self.game_rules.get("naturalRegeneration"),
            None | Some(GameRuleValue::Boolean(true))
        );
        let difficulty = self.difficulty;
        let uuids = self
            .players
            .iter()
            .filter_map(|(uuid, player)| {
                (player.connected && !player.vitals.is_dead()).then_some(*uuid)
            })
            .collect::<Vec<_>>();

        let mut stats = PlayerTickStats::default();
        for uuid in uuids {
            let Some(player) = self.players.get_mut(&uuid) else {
                continue;
            };
            stats.players_ticked = stats.players_ticked.saturating_add(1);

            let previous = player.vitals;
            let tick = player.gameplay.tick(
                &mut player.vitals,
                natural_regeneration && difficulty != Difficulty::Peaceful,
            );
            stats.effects_expired = stats
                .effects_expired
                .saturating_add(tick.expired_effects.len());

            if difficulty == Difficulty::Peaceful {
                peaceful_recovery(&mut player.vitals, player.gameplay.max_health());
            } else {
                if tick.health_delta > 0.0 {
                    let before = player.vitals.health;
                    let _ = player
                        .vitals
                        .heal_to(tick.health_delta, player.gameplay.max_health());
                    if player.vitals.health > before {
                        stats.natural_regenerations = stats.natural_regenerations.saturating_add(1);
                    }
                }
                if tick.starvation_damage > 0.0
                    && starvation_can_hurt(difficulty, player.vitals.health)
                {
                    let _ = player.vitals.damage(tick.starvation_damage);
                    stats.starvation_hits = stats.starvation_hits.saturating_add(1);
                }
            }

            if player.vitals != previous {
                events.push(GameEvent::PlayerVitalsChanged {
                    uuid,
                    vitals: player.vitals,
                });
            }
        }
        stats
    }
}

fn peaceful_recovery(vitals: &mut Vitals, max_health: f32) {
    // Vanilla-like peaceful recovery is intentionally conservative here: food
    // is restored immediately while health uses the normal 20 TPS cadence in
    // future tuning. Keeping this deterministic prevents wall-clock coupling.
    vitals.food = 20;
    vitals.saturation = vitals.saturation.max(5.0).min(20.0);
    if vitals.health > max_health {
        vitals.health = max_health;
    }
}

#[must_use]
fn starvation_can_hurt(difficulty: Difficulty, health: f32) -> bool {
    match difficulty {
        Difficulty::Peaceful => false,
        Difficulty::Easy => health > 10.0,
        Difficulty::Normal => health > 1.0,
        Difficulty::Hard => health > 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GameState, StatusEffect, Transform};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn status_effects_tick_and_expire() {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(1);
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state
            .players
            .get_mut(&uuid)
            .unwrap()
            .gameplay
            .status_effects
            .apply(StatusEffect::new("minecraft:speed", 0, 1).unwrap());

        let mut events = Vec::new();
        let stats = state.tick_player_gameplay(&mut events);
        assert_eq!(stats.effects_expired, 1);
        assert!(
            !state.players[&uuid]
                .gameplay
                .status_effects
                .contains("minecraft:speed")
        );
    }

    #[test]
    fn natural_regeneration_emits_vitals_event() {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(2);
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        {
            let player = state.players.get_mut(&uuid).unwrap();
            player.vitals.health = 10.0;
            player.gameplay.tick(&mut player.vitals, true);
            for _ in 0..78 {
                player.gameplay.tick(&mut player.vitals, true);
            }
        }

        let mut events = Vec::new();
        let stats = state.tick_player_gameplay(&mut events);
        assert_eq!(stats.natural_regenerations, 1);
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::PlayerVitalsChanged { uuid: event_uuid, .. } if *event_uuid == uuid
        )));
    }

    #[test]
    fn easy_starvation_stops_at_ten_health() {
        assert!(starvation_can_hurt(Difficulty::Easy, 10.01));
        assert!(!starvation_can_hurt(Difficulty::Easy, 10.0));
    }

    #[test]
    fn normal_starvation_stops_at_one_health() {
        assert!(starvation_can_hurt(Difficulty::Normal, 1.01));
        assert!(!starvation_can_hurt(Difficulty::Normal, 1.0));
    }
}
