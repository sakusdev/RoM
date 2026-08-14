//! Persistent gameplay components attached to a player.
//!
//! `Vitals` remains the wire/persistence-friendly source of health and hunger
//! numbers. This component owns the richer systems that need state across ticks:
//! attributes, active status effects, hunger timing, and fall-distance tracking.

use serde::{Deserialize, Serialize};

use crate::{AttributeMap, HungerState, HungerTick, StatusEffect, StatusEffectStore, Vitals};

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
        self.status_effects.haste_multiplier()
            * self.status_effects.mining_fatigue_multiplier()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, StatusEffect};

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
        gameplay.attributes.insert(
            Attribute::new("minecraft:max_health", 40.0, 1.0, 1024.0).unwrap(),
        );
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
}
