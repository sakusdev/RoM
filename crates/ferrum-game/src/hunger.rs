//! Hunger, saturation, exhaustion, and natural regeneration helpers.
use serde::{Deserialize, Serialize};

pub const MAX_FOOD: u8 = 20;
pub const MAX_EXHAUSTION: f32 = 4.0;
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HungerState {
    pub food: u8,
    pub saturation: f32,
    pub exhaustion: f32,
    pub tick_timer: u16,
}
impl Default for HungerState {
    fn default() -> Self {
        Self {
            food: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            tick_timer: 0,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HungerTick {
    None,
    Heal(f32),
    Starve(f32),
}
impl HungerState {
    pub fn eat(&mut self, nutrition: u8, saturation_modifier: f32) {
        self.food = self.food.saturating_add(nutrition).min(MAX_FOOD);
        let gain = f32::from(nutrition) * saturation_modifier * 2.0;
        self.saturation = (self.saturation + gain).min(f32::from(self.food));
    }
    pub fn add_exhaustion(&mut self, amount: f32) {
        if amount.is_finite() && amount > 0.0 {
            self.exhaustion = (self.exhaustion + amount).min(40.0);
        }
    }
    pub fn process_exhaustion(&mut self) {
        while self.exhaustion >= MAX_EXHAUSTION {
            self.exhaustion -= MAX_EXHAUSTION;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.food = self.food.saturating_sub(1);
            }
        }
    }
    pub fn tick(&mut self, health: f32, max_health: f32, natural_regeneration: bool) -> HungerTick {
        self.process_exhaustion();
        self.tick_timer = self.tick_timer.saturating_add(1);
        if natural_regeneration && self.food >= 18 && health < max_health && self.tick_timer >= 80 {
            self.tick_timer = 0;
            self.add_exhaustion(6.0);
            return HungerTick::Heal(1.0);
        }
        if self.food == 0 && self.tick_timer >= 80 {
            self.tick_timer = 0;
            return HungerTick::Starve(1.0);
        }
        HungerTick::None
    }
}
#[must_use]
pub const fn sprint_allowed(food: u8) -> bool {
    food > 6
}
#[must_use]
pub const fn natural_regeneration_allowed(food: u8) -> bool {
    food >= 18
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaustion_uses_saturation_first() {
        let mut h = HungerState {
            saturation: 1.0,
            ..HungerState::default()
        };
        h.add_exhaustion(8.0);
        h.process_exhaustion();
        assert_eq!(h.saturation, 0.0);
        assert_eq!(h.food, 19);
    }
    #[test]
    fn eating_caps_food() {
        let mut h = HungerState {
            food: 18,
            ..HungerState::default()
        };
        h.eat(8, 0.6);
        assert_eq!(h.food, 20);
    }
    #[test]
    fn regen_after_timer() {
        let mut h = HungerState {
            tick_timer: 79,
            ..HungerState::default()
        };
        assert_eq!(h.tick(10.0, 20.0, true), HungerTick::Heal(1.0));
    }
}
