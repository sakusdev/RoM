use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::validate_resource_location;

pub const MAX_ATTRIBUTE_MODIFIERS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeId(String);

impl AttributeId {
    pub fn new(value: impl Into<String>) -> Result<Self, AttributeError> {
        let value = value.into();
        if !validate_resource_location(&value) {
            return Err(AttributeError::InvalidAttributeId { value });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeOperation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeModifier {
    pub id: String,
    pub amount: f64,
    pub operation: AttributeOperation,
}

impl AttributeModifier {
    pub fn new(
        id: impl Into<String>,
        amount: f64,
        operation: AttributeOperation,
    ) -> Result<Self, AttributeError> {
        let id = id.into();
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err(AttributeError::InvalidModifierId { id });
        }
        if !amount.is_finite() {
            return Err(AttributeError::NonFiniteModifier { amount });
        }
        Ok(Self {
            id,
            amount,
            operation,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeInstance {
    base: f64,
    minimum: f64,
    maximum: f64,
    modifiers: BTreeMap<String, AttributeModifier>,
}

impl AttributeInstance {
    pub fn new(base: f64, minimum: f64, maximum: f64) -> Result<Self, AttributeError> {
        if !base.is_finite() || !minimum.is_finite() || !maximum.is_finite() {
            return Err(AttributeError::NonFiniteBounds);
        }
        if minimum > maximum {
            return Err(AttributeError::InvertedBounds { minimum, maximum });
        }
        if !(minimum..=maximum).contains(&base) {
            return Err(AttributeError::BaseOutOfBounds {
                base,
                minimum,
                maximum,
            });
        }
        Ok(Self {
            base,
            minimum,
            maximum,
            modifiers: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn base(&self) -> f64 {
        self.base
    }

    pub fn set_base(&mut self, base: f64) -> Result<f64, AttributeError> {
        if !base.is_finite() || !(self.minimum..=self.maximum).contains(&base) {
            return Err(AttributeError::BaseOutOfBounds {
                base,
                minimum: self.minimum,
                maximum: self.maximum,
            });
        }
        Ok(std::mem::replace(&mut self.base, base))
    }

    pub fn insert_modifier(
        &mut self,
        modifier: AttributeModifier,
    ) -> Result<Option<AttributeModifier>, AttributeError> {
        if !self.modifiers.contains_key(&modifier.id)
            && self.modifiers.len() >= MAX_ATTRIBUTE_MODIFIERS
        {
            return Err(AttributeError::TooManyModifiers {
                limit: MAX_ATTRIBUTE_MODIFIERS,
            });
        }
        Ok(self.modifiers.insert(modifier.id.clone(), modifier))
    }

    pub fn remove_modifier(&mut self, id: &str) -> Option<AttributeModifier> {
        self.modifiers.remove(id)
    }

    #[must_use]
    pub fn modifiers(&self) -> &BTreeMap<String, AttributeModifier> {
        &self.modifiers
    }

    #[must_use]
    pub fn value(&self) -> f64 {
        let add_value = self
            .modifiers
            .values()
            .filter(|modifier| modifier.operation == AttributeOperation::AddValue)
            .map(|modifier| modifier.amount)
            .sum::<f64>();
        let add_base = self
            .modifiers
            .values()
            .filter(|modifier| modifier.operation == AttributeOperation::AddMultipliedBase)
            .map(|modifier| modifier.amount)
            .sum::<f64>();
        let mut value = self.base + add_value + self.base * add_base;
        for modifier in self
            .modifiers
            .values()
            .filter(|modifier| modifier.operation == AttributeOperation::AddMultipliedTotal)
        {
            value *= 1.0 + modifier.amount;
        }
        value.clamp(self.minimum, self.maximum)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeSet {
    values: BTreeMap<AttributeId, AttributeInstance>,
}

impl Default for AttributeSet {
    fn default() -> Self {
        Self::player_defaults()
    }
}

impl AttributeSet {
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn player_defaults() -> Self {
        let mut attributes = Self::new();
        for (id, base, minimum, maximum) in [
            ("minecraft:max_health", 20.0, 1.0, 1_024.0),
            ("minecraft:movement_speed", 0.1, 0.0, 1_024.0),
            ("minecraft:attack_damage", 1.0, 0.0, 2_048.0),
            ("minecraft:attack_speed", 4.0, 0.0, 1_024.0),
            ("minecraft:armor", 0.0, 0.0, 30.0),
            ("minecraft:armor_toughness", 0.0, 0.0, 20.0),
            ("minecraft:knockback_resistance", 0.0, 0.0, 1.0),
            ("minecraft:safe_fall_distance", 3.0, 0.0, 1_024.0),
            ("minecraft:gravity", 0.08, 0.0, 1.0),
            ("minecraft:step_height", 0.6, 0.0, 10.0),
            ("minecraft:block_interaction_range", 4.5, 0.0, 64.0),
            ("minecraft:entity_interaction_range", 3.0, 0.0, 64.0),
        ] {
            attributes
                .insert(
                    AttributeId::new(id).expect("built-in attribute id is valid"),
                    AttributeInstance::new(base, minimum, maximum)
                        .expect("built-in attribute bounds are valid"),
                )
                .expect("built-in attribute set stays below its limit");
        }
        attributes
    }

    pub fn insert(
        &mut self,
        id: AttributeId,
        attribute: AttributeInstance,
    ) -> Result<Option<AttributeInstance>, AttributeError> {
        if !self.values.contains_key(&id) && self.values.len() >= 256 {
            return Err(AttributeError::TooManyAttributes { limit: 256 });
        }
        Ok(self.values.insert(id, attribute))
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AttributeInstance> {
        self.values
            .iter()
            .find_map(|(key, value)| (key.as_str() == id).then_some(value))
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut AttributeInstance> {
        self.values
            .iter_mut()
            .find_map(|(key, value)| (key.as_str() == id).then_some(value))
    }

    #[must_use]
    pub fn value(&self, id: &str) -> Option<f64> {
        self.get(id).map(AttributeInstance::value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AttributeId, &AttributeInstance)> {
        self.values.iter()
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum AttributeError {
    #[error("invalid attribute resource location {value}")]
    InvalidAttributeId { value: String },
    #[error("attribute bounds must be finite")]
    NonFiniteBounds,
    #[error("attribute minimum {minimum} exceeds maximum {maximum}")]
    InvertedBounds { minimum: f64, maximum: f64 },
    #[error("attribute base {base} is outside {minimum}..={maximum}")]
    BaseOutOfBounds {
        base: f64,
        minimum: f64,
        maximum: f64,
    },
    #[error("attribute modifier id is invalid: {id}")]
    InvalidModifierId { id: String },
    #[error("attribute modifier amount {amount} is not finite")]
    NonFiniteModifier { amount: f64 },
    #[error("attribute modifier count exceeds {limit}")]
    TooManyModifiers { limit: usize },
    #[error("attribute count exceeds {limit}")]
    TooManyAttributes { limit: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_operations_in_vanilla_order() {
        let mut attribute = AttributeInstance::new(10.0, 0.0, 100.0).unwrap();
        attribute
            .insert_modifier(
                AttributeModifier::new("flat", 2.0, AttributeOperation::AddValue).unwrap(),
            )
            .unwrap();
        attribute
            .insert_modifier(
                AttributeModifier::new("base", 0.5, AttributeOperation::AddMultipliedBase).unwrap(),
            )
            .unwrap();
        attribute
            .insert_modifier(
                AttributeModifier::new("total", 0.25, AttributeOperation::AddMultipliedTotal)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(attribute.value(), 21.25);
    }

    #[test]
    fn player_defaults_expose_gameplay_attributes() {
        let attributes = AttributeSet::player_defaults();
        assert_eq!(attributes.value("minecraft:max_health"), Some(20.0));
        assert_eq!(
            attributes.value("minecraft:block_interaction_range"),
            Some(4.5)
        );
    }
}
