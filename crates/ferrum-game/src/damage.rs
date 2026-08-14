//! Damage calculation primitives used by players and living entities.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageKind {
    Generic,
    PlayerAttack,
    MobAttack,
    Projectile,
    Fall,
    Fire,
    Lava,
    Drowning,
    Starvation,
    Void,
    Explosion,
    Magic,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamageSource {
    pub kind: DamageKind,
    pub amount: f32,
    pub bypass_armor: bool,
    pub bypass_absorption: bool,
    pub scales_with_difficulty: bool,
}

impl DamageSource {
    pub fn new(kind: DamageKind, amount: f32) -> Result<Self, DamageError> {
        if !amount.is_finite() || amount < 0.0 {
            return Err(DamageError::InvalidAmount { amount });
        }
        Ok(Self {
            kind,
            amount,
            bypass_armor: matches!(kind, DamageKind::Void | DamageKind::Starvation),
            bypass_absorption: matches!(kind, DamageKind::Void),
            scales_with_difficulty: matches!(kind, DamageKind::MobAttack | DamageKind::Starvation),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamageMitigation {
    pub armor: f32,
    pub toughness: f32,
    pub resistance_level: u8,
    pub protection_points: u8,
}

impl Default for DamageMitigation {
    fn default() -> Self {
        Self {
            armor: 0.0,
            toughness: 0.0,
            resistance_level: 0,
            protection_points: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DamageResult {
    pub raw: f32,
    pub after_armor: f32,
    pub after_resistance: f32,
    pub final_amount: f32,
}

pub fn calculate_damage(
    source: DamageSource,
    mitigation: DamageMitigation,
) -> Result<DamageResult, DamageError> {
    validate_mitigation(mitigation)?;
    let raw = source.amount;
    let after_armor = if source.bypass_armor {
        raw
    } else {
        apply_armor(raw, mitigation.armor, mitigation.toughness)
    };
    let after_resistance = apply_resistance(after_armor, mitigation.resistance_level);
    let final_amount = apply_protection(after_resistance, mitigation.protection_points);
    Ok(DamageResult {
        raw,
        after_armor,
        after_resistance,
        final_amount,
    })
}

#[must_use]
pub fn apply_armor(amount: f32, armor: f32, toughness: f32) -> f32 {
    if amount <= 0.0 {
        return 0.0;
    }
    let armor = armor.clamp(0.0, 30.0);
    let toughness = toughness.clamp(0.0, 20.0);
    let divisor = 2.0 + toughness / 4.0;
    let reduced_armor = (armor - amount / divisor).max(armor * 0.2).min(20.0);
    amount * (1.0 - reduced_armor / 25.0)
}

#[must_use]
pub fn apply_resistance(amount: f32, level: u8) -> f32 {
    let reduction = (f32::from(level) * 0.2).clamp(0.0, 1.0);
    amount * (1.0 - reduction)
}

#[must_use]
pub fn apply_protection(amount: f32, points: u8) -> f32 {
    let points = points.min(20);
    amount * (1.0 - f32::from(points) * 0.04)
}

#[must_use]
pub fn fall_damage(fall_distance: f32, safe_fall_distance: f32, multiplier: f32) -> f32 {
    if !fall_distance.is_finite()
        || !safe_fall_distance.is_finite()
        || !multiplier.is_finite()
        || multiplier <= 0.0
    {
        return 0.0;
    }
    ((fall_distance - safe_fall_distance).ceil() * multiplier).max(0.0)
}

#[must_use]
pub fn difficulty_multiplier(kind: DamageKind, difficulty: u8) -> f32 {
    match kind {
        DamageKind::MobAttack => match difficulty {
            0 => 0.0,
            1 => 0.5,
            2 => 1.0,
            _ => 1.5,
        },
        DamageKind::Starvation if difficulty == 0 => 0.0,
        _ => 1.0,
    }
}

#[must_use]
pub fn knockback_vector(
    attacker: [f64; 3],
    victim: [f64; 3],
    horizontal_strength: f64,
    vertical_strength: f64,
    resistance: f64,
) -> [f64; 3] {
    if attacker.into_iter().chain(victim).any(|v| !v.is_finite())
        || !horizontal_strength.is_finite()
        || !vertical_strength.is_finite()
        || !resistance.is_finite()
    {
        return [0.0; 3];
    }
    let dx = victim[0] - attacker[0];
    let dz = victim[2] - attacker[2];
    let length = (dx * dx + dz * dz).sqrt();
    if length < 1.0e-9 {
        return [0.0, vertical_strength.max(0.0), 0.0];
    }
    let scale = (1.0 - resistance.clamp(0.0, 1.0)) * horizontal_strength.max(0.0);
    [
        dx / length * scale,
        vertical_strength.max(0.0),
        dz / length * scale,
    ]
}

fn validate_mitigation(mitigation: DamageMitigation) -> Result<(), DamageError> {
    if !mitigation.armor.is_finite() || !mitigation.toughness.is_finite() {
        return Err(DamageError::NonFiniteMitigation);
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq)]
pub enum DamageError {
    #[error("damage amount {amount} must be finite and non-negative")]
    InvalidAmount { amount: f32 },
    #[error("damage mitigation values must be finite")]
    NonFiniteMitigation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_reduces_damage() {
        let source = DamageSource::new(DamageKind::PlayerAttack, 10.0).unwrap();
        let result = calculate_damage(
            source,
            DamageMitigation {
                armor: 20.0,
                toughness: 8.0,
                ..DamageMitigation::default()
            },
        )
        .unwrap();
        assert!(result.final_amount < result.raw);
    }

    #[test]
    fn void_bypasses_armor() {
        let source = DamageSource::new(DamageKind::Void, 6.0).unwrap();
        let result = calculate_damage(
            source,
            DamageMitigation {
                armor: 30.0,
                toughness: 20.0,
                ..DamageMitigation::default()
            },
        )
        .unwrap();
        assert_eq!(result.after_armor, 6.0);
    }

    #[test]
    fn fall_damage_uses_safe_distance() {
        assert_eq!(fall_damage(3.0, 3.0, 1.0), 0.0);
        assert_eq!(fall_damage(7.2, 3.0, 1.0), 5.0);
    }

    #[test]
    fn knockback_points_away_from_attacker() {
        let velocity = knockback_vector([0.0; 3], [3.0, 0.0, 4.0], 0.4, 0.4, 0.0);
        assert!(velocity[0] > 0.0);
        assert!(velocity[2] > 0.0);
        assert_eq!(velocity[1], 0.4);
    }
}
