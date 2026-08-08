use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Difficulty, EntityId, Velocity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageKind {
    Generic,
    PlayerAttack,
    MobAttack,
    Projectile,
    Fall,
    Fire,
    Drowning,
    Explosion,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageSource {
    pub kind: DamageKind,
    pub attacker: Option<EntityId>,
    pub direct_entity: Option<EntityId>,
    pub bypasses_armor: bool,
    pub bypasses_invulnerability: bool,
}

impl DamageSource {
    #[must_use]
    pub const fn generic(kind: DamageKind) -> Self {
        Self {
            kind,
            attacker: None,
            direct_entity: None,
            bypasses_armor: false,
            bypasses_invulnerability: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageContext {
    pub raw_damage: f32,
    pub armor: f64,
    pub armor_toughness: f64,
    pub resistance_level: u8,
    pub difficulty: Difficulty,
    pub source: DamageSource,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageResult {
    pub raw_damage: f32,
    pub after_difficulty: f32,
    pub after_armor: f32,
    pub final_damage: f32,
}

pub fn calculate_damage(context: DamageContext) -> Result<DamageResult, CombatError> {
    if !context.raw_damage.is_finite() || context.raw_damage < 0.0 {
        return Err(CombatError::InvalidDamage {
            damage: context.raw_damage,
        });
    }
    for (name, value) in [
        ("armor", context.armor),
        ("armor_toughness", context.armor_toughness),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(CombatError::InvalidAttribute { name, value });
        }
    }

    let after_difficulty = match context.source.kind {
        DamageKind::MobAttack | DamageKind::Projectile => match context.difficulty {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => context.raw_damage.min(context.raw_damage * 0.5 + 1.0),
            Difficulty::Normal => context.raw_damage,
            Difficulty::Hard => context.raw_damage * 1.5,
        },
        _ => context.raw_damage,
    };
    let after_armor = if context.source.bypasses_armor {
        after_difficulty
    } else {
        reduce_by_armor(after_difficulty, context.armor, context.armor_toughness)
    };
    let resistance = f32::from(context.resistance_level.min(4)) * 0.2;
    let final_damage = (after_armor * (1.0 - resistance)).max(0.0);
    Ok(DamageResult {
        raw_damage: context.raw_damage,
        after_difficulty,
        after_armor,
        final_damage,
    })
}

#[must_use]
pub fn reduce_by_armor(damage: f32, armor: f64, toughness: f64) -> f32 {
    if damage <= 0.0 {
        return 0.0;
    }
    let damage_f64 = f64::from(damage);
    let divisor = 2.0 + toughness / 4.0;
    let effective = (armor - damage_f64 / divisor).max(armor / 5.0).min(20.0);
    (damage_f64 * (1.0 - effective / 25.0)) as f32
}

pub fn fall_damage(
    fall_distance: f32,
    safe_fall_distance: f64,
    jump_boost_level: u8,
) -> Result<f32, CombatError> {
    if !fall_distance.is_finite() || fall_distance < 0.0 {
        return Err(CombatError::InvalidFallDistance { fall_distance });
    }
    if !safe_fall_distance.is_finite() || safe_fall_distance < 0.0 {
        return Err(CombatError::InvalidAttribute {
            name: "safe_fall_distance",
            value: safe_fall_distance,
        });
    }
    let excess = f64::from(fall_distance) - safe_fall_distance - f64::from(jump_boost_level);
    Ok(excess.max(0.0).ceil() as f32)
}

pub fn knockback_velocity(
    current: Velocity,
    direction_xz: [f64; 2],
    strength: f64,
    resistance: f64,
) -> Result<Velocity, CombatError> {
    if !strength.is_finite() || strength < 0.0 {
        return Err(CombatError::InvalidKnockback { strength });
    }
    if !resistance.is_finite() || !(0.0..=1.0).contains(&resistance) {
        return Err(CombatError::InvalidAttribute {
            name: "knockback_resistance",
            value: resistance,
        });
    }
    let length = direction_xz[0].hypot(direction_xz[1]);
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(CombatError::InvalidDirection);
    }
    let applied = strength * (1.0 - resistance);
    let x = current.0[0] * 0.5 + direction_xz[0] / length * applied;
    let z = current.0[2] * 0.5 + direction_xz[1] / length * applied;
    let y = (current.0[1] * 0.5 + applied).min(0.4);
    Velocity::new([x, y, z]).map_err(CombatError::Entity)
}

#[derive(Debug, Error)]
pub enum CombatError {
    #[error("damage {damage} must be finite and non-negative")]
    InvalidDamage { damage: f32 },
    #[error("fall distance {fall_distance} must be finite and non-negative")]
    InvalidFallDistance { fall_distance: f32 },
    #[error("combat attribute {name} has invalid value {value}")]
    InvalidAttribute { name: &'static str, value: f64 },
    #[error("knockback strength {strength} must be finite and non-negative")]
    InvalidKnockback { strength: f64 },
    #[error("knockback direction must be finite and non-zero")]
    InvalidDirection,
    #[error(transparent)]
    Entity(#[from] crate::EntityError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_and_resistance_reduce_damage() {
        let result = calculate_damage(DamageContext {
            raw_damage: 10.0,
            armor: 20.0,
            armor_toughness: 8.0,
            resistance_level: 1,
            difficulty: Difficulty::Normal,
            source: DamageSource::generic(DamageKind::PlayerAttack),
        })
        .unwrap();
        assert!(result.final_damage < result.after_armor);
        assert!(result.after_armor < result.raw_damage);
    }

    #[test]
    fn fall_damage_respects_safe_distance_and_jump_boost() {
        assert_eq!(fall_damage(3.0, 3.0, 0).unwrap(), 0.0);
        assert_eq!(fall_damage(8.2, 3.0, 0).unwrap(), 6.0);
        assert_eq!(fall_damage(8.2, 3.0, 2).unwrap(), 4.0);
    }

    #[test]
    fn knockback_is_normalized_and_resisted() {
        let velocity = knockback_velocity(Velocity::default(), [3.0, 4.0], 0.4, 0.5).unwrap();
        assert!((velocity.0[0] - 0.12).abs() < 1.0e-9);
        assert!((velocity.0[2] - 0.16).abs() < 1.0e-9);
        assert!((velocity.0[1] - 0.2).abs() < 1.0e-9);
    }
}
