//! Persistent gameplay components attached to a player.
//!
//! `Vitals` remains the wire/persistence-friendly source of health and hunger
//! numbers. This component owns the richer systems that need state across ticks:
//! attributes, active status effects, hunger timing, and fall-distance tracking.

use serde::{Deserialize, Serialize};

use crate::{
    AttributeMap, Difficulty, GameEvent, GameRuleValue, GameState, GameStateError, HungerState,
    HungerTick, StatusEffect, StatusEffectStore, Vitals,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerGameplay {
    #[serde(default = "AttributeMap::player_defaults")]
    pub attributes: AttributeMap,
    #[serde(default)]
    pub status_effects: StatusEffectStore,
    #[serde(default)]
    hunger_timer: u16,
    #[serde(default)]
    fall_distance: f32,
}

impl Default for PlayerGameplay {
    fn default() -> Self {
        Self {
            attributes: AttributeMap::player_defaults(),
            status_effects: StatusEffectStore::default(),
            hunger_timer: 0,
            fall_distance: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerGameplayTick {
    pub expired_effects: Vec<StatusEffect>,
    pub health_delta: f32,
    pub starvation_damage: f32,
}

impl Default for PlayerGameplayTick {
    fn default() -> Self {
        Self {
            expired_effects: Vec::new(),
            health_delta: 0.0,
            starvation_damage: 0.0,
        }
    }
}

impl PlayerGameplay {
    #[must_use]
    pub const fn hunger_timer(&self) -> u16 {
        self.hunger_timer
    }

    #[must_use]
    pub const fn fall_distance(&self) -> f32 {
        self.fall_distance
    }

    pub fn reset_fall_distance(&mut self) {
        self.fall_distance = 0.0;
    }

    pub fn add_fall_distance(&mut self, delta: f32) {
        if delta.is_finite() && delta > 0.0 {
            self.fall_distance = (self.fall_distance + delta).min(1024.0);
        }
    }

    #[must_use]
    pub fn max_health(&self) -> f32 {
        self.attributes
            .value("minecraft:max_health")
            .unwrap_or(20.0)
            .clamp(1.0, f64::from(f32::MAX)) as f32
    }

    #[must_use]
    pub fn movement_speed_multiplier(&self) -> f64 {
        self.status_effects.movement_multiplier()
    }

    #[must_use]
    pub fn mining_haste_multiplier(&self) -> f64 {
        self.status_effects.haste_multiplier() * self.status_effects.mining_fatigue_multiplier()
    }

    /// Advances effects and hunger state by one server tick.
    ///
    /// The result describes health-side consequences; callers remain responsible
    /// for applying damage through the normal authoritative damage path so death
    /// drops and replication cannot be bypassed.
    pub fn tick(&mut self, vitals: &mut Vitals, natural_regeneration: bool) -> PlayerGameplayTick {
        let expired_effects = self.status_effects.tick();
        let mut hunger = HungerState {
            food: vitals.food,
            saturation: vitals.saturation,
            exhaustion: vitals.exhaustion,
            tick_timer: self.hunger_timer,
        };
        let hunger_tick = hunger.tick(vitals.health, self.max_health(), natural_regeneration);
        vitals.food = hunger.food;
        vitals.saturation = hunger.saturation;
        vitals.exhaustion = hunger.exhaustion;
        self.hunger_timer = hunger.tick_timer;

        match hunger_tick {
            HungerTick::None => PlayerGameplayTick {
                expired_effects,
                ..PlayerGameplayTick::default()
            },
            HungerTick::Heal(amount) => PlayerGameplayTick {
                expired_effects,
                health_delta: amount,
                starvation_damage: 0.0,
            },
            HungerTick::Starve(amount) => PlayerGameplayTick {
                expired_effects,
                health_delta: 0.0,
                starvation_damage: amount,
            },
        }
    }
}

impl GameState {
    /// Advances persistent player-only gameplay systems after the world entity
    /// tick. Health consequences are routed through the normal authoritative
    /// heal/damage methods so death, inventory drops, and replication stay intact.
    pub fn tick_player_gameplay(&mut self) -> Result<Vec<GameEvent>, GameStateError> {
        let natural_regeneration = matches!(
            self.game_rules().get("naturalRegeneration"),
            None | Some(GameRuleValue::Boolean(true))
        );
        let difficulty = self.difficulty();
        let players = self
            .players()
            .iter()
            .filter_map(|(uuid, player)| {
                (player.connected && !player.vitals.is_dead()).then_some(*uuid)
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();

        for uuid in players {
            let (before, after_hunger, tick) = {
                let player = self
                    .player_mut(uuid)
                    .ok_or(GameStateError::UnknownPlayer { uuid })?;
                let before = player.vitals;
                let tick = player.gameplay.tick(&mut player.vitals, natural_regeneration);
                (before, player.vitals, tick)
            };

            let mut emitted_health_event = false;
            if tick.health_delta > 0.0 {
                let max_health = self
                    .player(uuid)
                    .map(|player| player.gameplay.max_health())
                    .unwrap_or(20.0);
                let current = self
                    .player(uuid)
                    .ok_or(GameStateError::UnknownPlayer { uuid })?
                    .vitals
                    .health;
                let heal = tick.health_delta.min((max_health - current).max(0.0));
                if heal > 0.0 {
                    events.extend(self.heal_player(uuid, heal)?);
                    emitted_health_event = true;
                }
            }

            let starvation = starvation_damage_for(
                difficulty,
                after_hunger.health,
                tick.starvation_damage,
            );
            if starvation > 0.0 {
                events.extend(self.damage_player(uuid, starvation)?);
                emitted_health_event = true;
            }

            if !emitted_health_event && before != after_hunger {
                events.push(GameEvent::PlayerVitalsChanged {
                    uuid,
                    vitals: after_hunger,
                });
            }
        }
        Ok(events)
    }
}

#[must_use]
fn starvation_damage_for(difficulty: Difficulty, health: f32, requested: f32) -> f32 {
    if requested <= 0.0 || !requested.is_finite() {
        return 0.0;
    }
    match difficulty {
        Difficulty::Peaceful => 0.0,
        Difficulty::Easy if health <= 10.0 => 0.0,
        Difficulty::Normal if health <= 1.0 => 0.0,
        Difficulty::Easy | Difficulty::Normal | Difficulty::Hard => requested,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, PlayerUuid, StatusEffect, Transform};

    #[test]
    fn defaults_have_vanilla_health_attribute() {
        let gameplay = PlayerGameplay::default();
        assert_eq!(gameplay.max_health(), 20.0);
    }

    #[test]
    fn hunger_timer_is_persistent_while_vitals_hold_food_values() {
        let mut gameplay = PlayerGameplay::default();
        let mut vitals = Vitals {
            health: 10.0,
            ..Vitals::default()
        };
        for _ in 0..79 {
            let result = gameplay.tick(&mut vitals, true);
            assert_eq!(result.health_delta, 0.0);
        }
        let result = gameplay.tick(&mut vitals, true);
        assert_eq!(result.health_delta, 1.0);
        assert_eq!(gameplay.hunger_timer(), 0);
    }

    #[test]
    fn effects_expire_from_persistent_store() {
        let mut gameplay = PlayerGameplay::default();
        gameplay
            .status_effects
            .apply(StatusEffect::new("minecraft:speed", 0, 1).unwrap());
        let mut vitals = Vitals::default();
        let result = gameplay.tick(&mut vitals, false);
        assert_eq!(result.expired_effects.len(), 1);
        assert!(!gameplay.status_effects.contains("minecraft:speed"));
    }

    #[test]
    fn max_health_uses_attribute_value() {
        let mut gameplay = PlayerGameplay::default();
        gameplay
            .attributes
            .insert(Attribute::new("minecraft:max_health", 40.0, 1.0, 1024.0).unwrap());
        assert_eq!(gameplay.max_health(), 40.0);
    }

    #[test]
    fn fall_distance_rejects_bad_deltas_and_caps_growth() {
        let mut gameplay = PlayerGameplay::default();
        gameplay.add_fall_distance(f32::NAN);
        gameplay.add_fall_distance(-1.0);
        assert_eq!(gameplay.fall_distance(), 0.0);
        gameplay.add_fall_distance(2000.0);
        assert_eq!(gameplay.fall_distance(), 1024.0);
        gameplay.reset_fall_distance();
        assert_eq!(gameplay.fall_distance(), 0.0);
    }

    #[test]
    fn starvation_respects_difficulty_health_floors() {
        assert_eq!(starvation_damage_for(Difficulty::Peaceful, 20.0, 1.0), 0.0);
        assert_eq!(starvation_damage_for(Difficulty::Easy, 10.0, 1.0), 0.0);
        assert_eq!(starvation_damage_for(Difficulty::Normal, 1.0, 1.0), 0.0);
        assert_eq!(starvation_damage_for(Difficulty::Hard, 1.0, 1.0), 1.0);
    }

    #[test]
    fn game_state_ticks_natural_regeneration() {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(7);
        state
            .connect_player(
                uuid,
                "Steve",
                Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
            )
            .unwrap();
        let player = state.player_mut(uuid).unwrap();
        player.vitals.health = 10.0;
        for _ in 0..80 {
            state.tick_player_gameplay().unwrap();
        }
        assert_eq!(state.player(uuid).unwrap().vitals.health, 11.0);
    }
}
