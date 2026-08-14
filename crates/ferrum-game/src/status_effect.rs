//! Authoritative status-effect state.

use crate::validate_resource_location;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const MAX_EFFECT_DURATION_TICKS: u32 = 1_728_000;
pub const MAX_EFFECT_AMPLIFIER: u8 = 127;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEffect {
    pub id: String,
    pub amplifier: u8,
    pub duration_ticks: u32,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
}
impl StatusEffect {
    pub fn new(
        id: impl Into<String>,
        amplifier: u8,
        duration_ticks: u32,
    ) -> Result<Self, StatusEffectError> {
        let id = id.into();
        if !validate_resource_location(&id) {
            return Err(StatusEffectError::InvalidId(id));
        }
        if amplifier > MAX_EFFECT_AMPLIFIER {
            return Err(StatusEffectError::AmplifierTooLarge(amplifier));
        }
        if duration_ticks == 0 || duration_ticks > MAX_EFFECT_DURATION_TICKS {
            return Err(StatusEffectError::InvalidDuration(duration_ticks));
        }
        Ok(Self {
            id,
            amplifier,
            duration_ticks,
            ambient: false,
            show_particles: true,
            show_icon: true,
        })
    }
    pub fn tick(&mut self) {
        self.duration_ticks = self.duration_ticks.saturating_sub(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectUpdate {
    Added,
    Replaced,
    Extended,
    Ignored,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEffectStore {
    active: BTreeMap<String, StatusEffect>,
}
impl StatusEffectStore {
    pub fn apply(&mut self, incoming: StatusEffect) -> EffectUpdate {
        match self.active.get_mut(&incoming.id) {
            None => {
                self.active.insert(incoming.id.clone(), incoming);
                EffectUpdate::Added
            }
            Some(current) if incoming.amplifier > current.amplifier => {
                *current = incoming;
                EffectUpdate::Replaced
            }
            Some(current)
                if incoming.amplifier == current.amplifier
                    && incoming.duration_ticks > current.duration_ticks =>
            {
                current.duration_ticks = incoming.duration_ticks;
                current.ambient = incoming.ambient;
                current.show_particles = incoming.show_particles;
                current.show_icon = incoming.show_icon;
                EffectUpdate::Extended
            }
            _ => EffectUpdate::Ignored,
        }
    }
    pub fn remove(&mut self, id: &str) -> Option<StatusEffect> {
        self.active.remove(id)
    }
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&StatusEffect> {
        self.active.get(id)
    }
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.active.contains_key(id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &StatusEffect> {
        self.active.values()
    }
    pub fn tick(&mut self) -> Vec<StatusEffect> {
        for effect in self.active.values_mut() {
            effect.tick();
        }
        let ids = self
            .active
            .iter()
            .filter_map(|(id, e)| (e.duration_ticks == 0).then_some(id.clone()))
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| self.active.remove(&id))
            .collect()
    }
    #[must_use]
    pub fn movement_multiplier(&self) -> f64 {
        let speed = self
            .get("minecraft:speed")
            .map_or(0.0, |e| 0.2 * f64::from(e.amplifier + 1));
        let slow = self
            .get("minecraft:slowness")
            .map_or(0.0, |e| 0.15 * f64::from(e.amplifier + 1));
        (1.0 + speed) * (1.0 - slow).max(0.0)
    }
    #[must_use]
    pub fn jump_bonus(&self) -> f64 {
        self.get("minecraft:jump_boost")
            .map_or(0.0, |e| 0.1 * f64::from(e.amplifier + 1))
    }
    #[must_use]
    pub fn haste_multiplier(&self) -> f64 {
        self.get("minecraft:haste")
            .map_or(1.0, |e| 1.0 + 0.2 * f64::from(e.amplifier + 1))
    }
    #[must_use]
    pub fn mining_fatigue_multiplier(&self) -> f64 {
        match self.get("minecraft:mining_fatigue").map(|e| e.amplifier) {
            None => 1.0,
            Some(0) => 0.3,
            Some(1) => 0.09,
            Some(2) => 0.0027,
            Some(_) => 0.00081,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StatusEffectError {
    #[error("invalid status effect id {0}")]
    InvalidId(String),
    #[error("status effect amplifier {0} is too large")]
    AmplifierTooLarge(u8),
    #[error("invalid status effect duration {0}")]
    InvalidDuration(u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stronger_replaces() {
        let mut s = StatusEffectStore::default();
        s.apply(StatusEffect::new("minecraft:speed", 0, 20).unwrap());
        assert_eq!(
            s.apply(StatusEffect::new("minecraft:speed", 1, 10).unwrap()),
            EffectUpdate::Replaced
        );
    }
    #[test]
    fn equal_extends() {
        let mut s = StatusEffectStore::default();
        s.apply(StatusEffect::new("minecraft:haste", 0, 20).unwrap());
        assert_eq!(
            s.apply(StatusEffect::new("minecraft:haste", 0, 40).unwrap()),
            EffectUpdate::Extended
        );
    }
    #[test]
    fn expires() {
        let mut s = StatusEffectStore::default();
        s.apply(StatusEffect::new("minecraft:speed", 0, 1).unwrap());
        assert_eq!(s.tick().len(), 1);
        assert!(!s.contains("minecraft:speed"));
    }
}
