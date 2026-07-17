use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::validate_resource_location;

pub const MAX_STATUS_EFFECTS: usize = 64;
pub const MAX_STATUS_EFFECT_DURATION_TICKS: u32 = 20 * 60 * 60 * 24;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StatusEffectId(String);

impl StatusEffectId {
    pub fn new(value: impl Into<String>) -> Result<Self, StatusEffectError> {
        let value = value.into();
        if !validate_resource_location(&value) {
            return Err(StatusEffectError::InvalidEffectId { value });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEffectInstance {
    pub effect: StatusEffectId,
    pub amplifier: u8,
    pub duration_ticks: u32,
    pub ambient: bool,
    pub visible: bool,
    pub show_icon: bool,
}

impl StatusEffectInstance {
    pub fn new(
        effect: StatusEffectId,
        amplifier: u8,
        duration_ticks: u32,
    ) -> Result<Self, StatusEffectError> {
        if duration_ticks == 0 || duration_ticks > MAX_STATUS_EFFECT_DURATION_TICKS {
            return Err(StatusEffectError::InvalidDuration { duration_ticks });
        }
        Ok(Self {
            effect,
            amplifier,
            duration_ticks,
            ambient: false,
            visible: true,
            show_icon: true,
        })
    }

    #[must_use]
    pub const fn level(&self) -> u16 {
        self.amplifier as u16 + 1
    }

    fn tick(&mut self) -> bool {
        self.duration_ticks = self.duration_ticks.saturating_sub(1);
        self.duration_ticks == 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEffectSet {
    effects: BTreeMap<StatusEffectId, StatusEffectInstance>,
}

impl StatusEffectSet {
    pub fn insert(
        &mut self,
        effect: StatusEffectInstance,
    ) -> Result<Option<StatusEffectInstance>, StatusEffectError> {
        if !self.effects.contains_key(&effect.effect) && self.effects.len() >= MAX_STATUS_EFFECTS {
            return Err(StatusEffectError::TooManyEffects {
                limit: MAX_STATUS_EFFECTS,
            });
        }
        let replace = self.effects.get(&effect.effect).is_none_or(|current| {
            effect.amplifier > current.amplifier
                || (effect.amplifier == current.amplifier
                    && effect.duration_ticks >= current.duration_ticks)
        });
        if replace {
            Ok(self.effects.insert(effect.effect.clone(), effect))
        } else {
            Ok(None)
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<StatusEffectInstance> {
        let key = self
            .effects
            .keys()
            .find(|effect| effect.as_str() == id)
            .cloned()?;
        self.effects.remove(&key)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&StatusEffectInstance> {
        self.effects
            .iter()
            .find_map(|(effect, instance)| (effect.as_str() == id).then_some(instance))
    }

    pub fn tick(&mut self) -> Vec<StatusEffectInstance> {
        let expired = self
            .effects
            .iter_mut()
            .filter_map(|(id, effect)| effect.tick().then_some(id.clone()))
            .collect::<Vec<_>>();
        expired
            .into_iter()
            .filter_map(|id| self.effects.remove(&id))
            .collect()
    }

    #[must_use]
    pub fn movement_multiplier(&self) -> f64 {
        let speed = self
            .get("minecraft:speed")
            .map_or(0.0, |effect| 0.2 * f64::from(effect.level()));
        let slowness = self
            .get("minecraft:slowness")
            .map_or(0.0, |effect| 0.15 * f64::from(effect.level()));
        (1.0 + speed - slowness).max(0.0)
    }

    #[must_use]
    pub fn damage_resistance_level(&self) -> u8 {
        self.get("minecraft:resistance")
            .map_or(0, |effect| effect.level().min(u16::from(u8::MAX)) as u8)
    }

    #[must_use]
    pub fn jump_boost_level(&self) -> u8 {
        self.get("minecraft:jump_boost")
            .map_or(0, |effect| effect.level().min(u16::from(u8::MAX)) as u8)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&StatusEffectId, &StatusEffectInstance)> {
        self.effects.iter()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StatusEffectError {
    #[error("invalid status-effect resource location {value}")]
    InvalidEffectId { value: String },
    #[error(
        "status-effect duration {duration_ticks} must be between 1 and {MAX_STATUS_EFFECT_DURATION_TICKS} ticks"
    )]
    InvalidDuration { duration_ticks: u32 },
    #[error("status-effect count exceeds {limit}")]
    TooManyEffects { limit: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(id: &str, amplifier: u8, duration: u32) -> StatusEffectInstance {
        StatusEffectInstance::new(StatusEffectId::new(id).unwrap(), amplifier, duration).unwrap()
    }

    #[test]
    fn stronger_and_longer_effects_replace_weaker_ones() {
        let mut effects = StatusEffectSet::default();
        effects.insert(effect("minecraft:speed", 0, 100)).unwrap();
        effects.insert(effect("minecraft:speed", 0, 20)).unwrap();
        assert_eq!(effects.get("minecraft:speed").unwrap().duration_ticks, 100);
        effects.insert(effect("minecraft:speed", 1, 20)).unwrap();
        assert_eq!(effects.get("minecraft:speed").unwrap().amplifier, 1);
    }

    #[test]
    fn ticking_expires_effects_deterministically() {
        let mut effects = StatusEffectSet::default();
        effects
            .insert(effect("minecraft:resistance", 0, 2))
            .unwrap();
        assert!(effects.tick().is_empty());
        assert_eq!(effects.tick().len(), 1);
        assert!(effects.is_empty());
    }
}
