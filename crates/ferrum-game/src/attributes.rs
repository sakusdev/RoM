use crate::validate_resource_location;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

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
        if !validate_resource_location(&id) {
            return Err(AttributeError::InvalidId(id));
        }
        if !amount.is_finite() {
            return Err(AttributeError::NonFinite);
        }
        Ok(Self {
            id,
            amount,
            operation,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    pub id: String,
    base: f64,
    min: f64,
    max: f64,
    modifiers: BTreeMap<String, AttributeModifier>,
}

impl Attribute {
    pub fn new(
        id: impl Into<String>,
        base: f64,
        min: f64,
        max: f64,
    ) -> Result<Self, AttributeError> {
        let id = id.into();
        if !validate_resource_location(&id) {
            return Err(AttributeError::InvalidId(id));
        }
        if !base.is_finite() || !min.is_finite() || !max.is_finite() {
            return Err(AttributeError::NonFinite);
        }
        if min > max {
            return Err(AttributeError::InvalidBounds);
        }
        Ok(Self {
            id,
            base: base.clamp(min, max),
            min,
            max,
            modifiers: BTreeMap::new(),
        })
    }
    #[must_use]
    pub const fn base(&self) -> f64 {
        self.base
    }
    pub fn set_base(&mut self, value: f64) -> Result<(), AttributeError> {
        if !value.is_finite() {
            return Err(AttributeError::NonFinite);
        }
        self.base = value.clamp(self.min, self.max);
        Ok(())
    }
    pub fn insert_modifier(&mut self, modifier: AttributeModifier) -> Option<AttributeModifier> {
        self.modifiers.insert(modifier.id.clone(), modifier)
    }
    pub fn remove_modifier(&mut self, id: &str) -> Option<AttributeModifier> {
        self.modifiers.remove(id)
    }
    #[must_use]
    pub fn value(&self) -> f64 {
        let mut value = self.base;
        for m in self
            .modifiers
            .values()
            .filter(|m| m.operation == AttributeOperation::AddValue)
        {
            value += m.amount;
        }
        let after_add = value;
        for m in self
            .modifiers
            .values()
            .filter(|m| m.operation == AttributeOperation::AddMultipliedBase)
        {
            value += after_add * m.amount;
        }
        for m in self
            .modifiers
            .values()
            .filter(|m| m.operation == AttributeOperation::AddMultipliedTotal)
        {
            value *= 1.0 + m.amount;
        }
        value.clamp(self.min, self.max)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AttributeMap {
    entries: BTreeMap<String, Attribute>,
}

impl AttributeMap {
    pub fn insert(&mut self, value: Attribute) {
        self.entries.insert(value.id.clone(), value);
    }
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Attribute> {
        self.entries.get(id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Attribute> {
        self.entries.get_mut(id)
    }
    #[must_use]
    pub fn value(&self, id: &str) -> Option<f64> {
        self.get(id).map(Attribute::value)
    }
    pub fn player_defaults() -> Self {
        let mut out = Self::default();
        for a in [
            Attribute::new("minecraft:max_health", 20.0, 1.0, 1024.0).unwrap(),
            Attribute::new("minecraft:movement_speed", 0.1, 0.0, 1024.0).unwrap(),
            Attribute::new("minecraft:attack_damage", 1.0, 0.0, 2048.0).unwrap(),
            Attribute::new("minecraft:attack_speed", 4.0, 0.0, 1024.0).unwrap(),
            Attribute::new("minecraft:armor", 0.0, 0.0, 30.0).unwrap(),
            Attribute::new("minecraft:armor_toughness", 0.0, 0.0, 20.0).unwrap(),
            Attribute::new("minecraft:knockback_resistance", 0.0, 0.0, 1.0).unwrap(),
            Attribute::new("minecraft:safe_fall_distance", 3.0, 0.0, 1024.0).unwrap(),
            Attribute::new("minecraft:fall_damage_multiplier", 1.0, 0.0, 100.0).unwrap(),
            Attribute::new("minecraft:block_interaction_range", 4.5, 0.0, 64.0).unwrap(),
            Attribute::new("minecraft:entity_interaction_range", 3.0, 0.0, 64.0).unwrap(),
        ] {
            out.insert(a);
        }
        out
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum AttributeError {
    #[error("invalid attribute id {0}")]
    InvalidId(String),
    #[error("attribute value is not finite")]
    NonFinite,
    #[error("attribute bounds are invalid")]
    InvalidBounds,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operation_order_matches_expected_math() {
        let mut a = Attribute::new("minecraft:test", 10.0, 0.0, 100.0).unwrap();
        a.insert_modifier(
            AttributeModifier::new("rom:add", 2.0, AttributeOperation::AddValue).unwrap(),
        );
        a.insert_modifier(
            AttributeModifier::new("rom:base", 0.5, AttributeOperation::AddMultipliedBase).unwrap(),
        );
        a.insert_modifier(
            AttributeModifier::new("rom:total", 0.25, AttributeOperation::AddMultipliedTotal)
                .unwrap(),
        );
        assert_eq!(a.value(), 22.5);
    }
    #[test]
    fn defaults_exist() {
        let a = AttributeMap::player_defaults();
        assert_eq!(a.value("minecraft:max_health"), Some(20.0));
        assert_eq!(a.value("minecraft:block_interaction_range"), Some(4.5));
    }
}
