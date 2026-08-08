use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AttributeSet, EntityId, Inventory, InventorySession, StatusEffectSet,
    validate_resource_location,
};

pub const MAX_TOTAL_EXPERIENCE: u64 = i32::MAX as u64;
pub const MAX_EXPERIENCE_LEVEL: u32 = i32::MAX as u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerUuid([u8; 16]);

impl PlayerUuid {
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value.to_be_bytes())
    }

    #[must_use]
    pub const fn from_bytes(value: [u8; 16]) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u128 {
        u128::from_be_bytes(self.0)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn parse(value: &str) -> Result<Self, PlayerError> {
        match value {
            "survival" | "s" | "0" => Ok(Self::Survival),
            "creative" | "c" | "1" => Ok(Self::Creative),
            "adventure" | "a" | "2" => Ok(Self::Adventure),
            "spectator" | "sp" | "3" => Ok(Self::Spectator),
            _ => Err(PlayerError::UnknownGameMode {
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Abilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub allow_flying: bool,
    pub instant_break: bool,
    pub fly_speed: f32,
    pub walk_speed: f32,
}

impl Abilities {
    #[must_use]
    pub fn for_game_mode(game_mode: GameMode) -> Self {
        match game_mode {
            GameMode::Survival | GameMode::Adventure => Self {
                invulnerable: false,
                flying: false,
                allow_flying: false,
                instant_break: false,
                fly_speed: 0.05,
                walk_speed: 0.1,
            },
            GameMode::Creative => Self {
                invulnerable: true,
                flying: false,
                allow_flying: true,
                instant_break: true,
                fly_speed: 0.05,
                walk_speed: 0.1,
            },
            GameMode::Spectator => Self {
                invulnerable: true,
                flying: true,
                allow_flying: true,
                instant_break: false,
                fly_speed: 0.1,
                walk_speed: 0.1,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vitals {
    pub health: f32,
    pub absorption: f32,
    pub food: u8,
    pub saturation: f32,
    pub exhaustion: f32,
    pub air: i32,
    pub fire_ticks: i32,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            health: 20.0,
            absorption: 0.0,
            food: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            air: 300,
            fire_ticks: 0,
        }
    }
}

impl Vitals {
    pub fn damage(&mut self, amount: f32) -> Result<f32, PlayerError> {
        if !amount.is_finite() || amount < 0.0 {
            return Err(PlayerError::InvalidDamage { amount });
        }
        let absorbed = self.absorption.min(amount);
        self.absorption -= absorbed;
        let remaining = amount - absorbed;
        self.health = (self.health - remaining).max(0.0);
        Ok(self.health)
    }

    pub fn heal(&mut self, amount: f32) -> Result<f32, PlayerError> {
        self.heal_to_max(amount, 20.0)
    }

    pub fn heal_to_max(&mut self, amount: f32, max_health: f32) -> Result<f32, PlayerError> {
        if !amount.is_finite() || amount < 0.0 {
            return Err(PlayerError::InvalidHealing { amount });
        }
        if !max_health.is_finite() || max_health <= 0.0 {
            return Err(PlayerError::InvalidMaximumHealth { max_health });
        }
        self.health = (self.health + amount).min(max_health);
        Ok(self.health)
    }

    #[must_use]
    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Experience {
    pub level: u32,
    pub progress: f32,
    pub total: u64,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            level: 0,
            progress: 0.0,
            total: 0,
        }
    }
}

impl Experience {
    pub fn add_points(&mut self, points: u32) -> Result<(), PlayerError> {
        self.validate()?;
        if points == 0 {
            return Ok(());
        }

        self.total = self
            .total
            .saturating_add(u64::from(points))
            .min(MAX_TOTAL_EXPERIENCE);
        self.progress += points as f32 / self.points_to_next_level() as f32;
        while self.progress >= 1.0 {
            let previous_requirement = self.points_to_next_level() as f32;
            let points_into_next_level = (self.progress - 1.0) * previous_requirement;
            if self.level == MAX_EXPERIENCE_LEVEL {
                self.progress = 0.0;
                break;
            }
            self.level += 1;
            self.progress = points_into_next_level / self.points_to_next_level() as f32;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), PlayerError> {
        if self.level > MAX_EXPERIENCE_LEVEL {
            return Err(PlayerError::ExperienceLevelOutOfRange { level: self.level });
        }
        if !self.progress.is_finite() || !(0.0..1.0).contains(&self.progress) {
            return Err(PlayerError::InvalidExperienceProgress {
                progress: self.progress,
            });
        }
        if self.total > MAX_TOTAL_EXPERIENCE {
            return Err(PlayerError::TotalExperienceOutOfRange { total: self.total });
        }
        Ok(())
    }

    #[must_use]
    pub fn points_to_next_level(&self) -> u64 {
        let level = u64::from(self.level);
        if level >= 30 {
            112 + (level - 30) * 9
        } else if level >= 15 {
            37 + (level - 15) * 5
        } else {
            7 + level * 2
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub uuid: PlayerUuid,
    pub name: String,
    pub entity_id: Option<EntityId>,
    pub game_mode: GameMode,
    pub previous_game_mode: Option<GameMode>,
    pub dimension: String,
    pub inventory: Inventory,
    #[serde(skip, default)]
    pub inventory_session: InventorySession,
    pub abilities: Abilities,
    pub vitals: Vitals,
    pub experience: Experience,
    #[serde(skip, default)]
    pub experience_pickup_delay_ticks: u8,
    #[serde(default)]
    pub attributes: AttributeSet,
    #[serde(default)]
    pub status_effects: StatusEffectSet,
    #[serde(default)]
    pub fall_distance: f32,
    pub permission_level: u8,
    pub connected: bool,
}

impl PlayerState {
    pub fn new(
        uuid: PlayerUuid,
        name: impl Into<String>,
        entity_id: EntityId,
        dimension: impl Into<String>,
    ) -> Result<Self, PlayerError> {
        let name = name.into();
        validate_username(&name)?;
        let dimension = dimension.into();
        if !validate_resource_location(&dimension) {
            return Err(PlayerError::InvalidDimension { dimension });
        }
        Ok(Self {
            uuid,
            name,
            entity_id: Some(entity_id),
            game_mode: GameMode::Survival,
            previous_game_mode: None,
            dimension,
            inventory: Inventory::new(),
            inventory_session: InventorySession::default(),
            abilities: Abilities::for_game_mode(GameMode::Survival),
            vitals: Vitals::default(),
            experience: Experience::default(),
            experience_pickup_delay_ticks: 0,
            attributes: AttributeSet::player_defaults(),
            status_effects: StatusEffectSet::default(),
            fall_distance: 0.0,
            permission_level: 0,
            connected: true,
        })
    }

    pub fn set_game_mode(&mut self, game_mode: GameMode) -> GameMode {
        let previous = self.game_mode;
        if previous != game_mode {
            self.previous_game_mode = Some(previous);
            self.game_mode = game_mode;
            self.abilities = Abilities::for_game_mode(game_mode);
            if matches!(game_mode, GameMode::Creative | GameMode::Spectator) {
                self.fall_distance = 0.0;
            }
        }
        previous
    }

    #[must_use]
    pub fn attribute_value(&self, id: &str) -> Option<f64> {
        self.attributes.value(id)
    }

    #[must_use]
    pub fn max_health(&self) -> f32 {
        self.attribute_value("minecraft:max_health")
            .unwrap_or(20.0)
            .clamp(1.0, f64::from(f32::MAX)) as f32
    }

    pub fn tick_status_effects(&mut self) -> Vec<crate::StatusEffectInstance> {
        self.status_effects.tick()
    }

    pub fn tick_experience_pickup_delay(&mut self) {
        self.experience_pickup_delay_ticks = self.experience_pickup_delay_ticks.saturating_sub(1);
    }

    pub fn set_permission_level(&mut self, level: u8) -> Result<(), PlayerError> {
        if level > 4 {
            return Err(PlayerError::InvalidPermissionLevel { level });
        }
        self.permission_level = level;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        self.entity_id = None;
        self.inventory_session.reset();
    }

    pub fn reconnect(&mut self, entity_id: EntityId) {
        self.connected = true;
        self.entity_id = Some(entity_id);
        self.inventory_session.reset();
    }
}

pub fn validate_username(name: &str) -> Result<(), PlayerError> {
    if !(3..=16).contains(&name.len())
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(PlayerError::InvalidUsername {
            name: name.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq)]
pub enum PlayerError {
    #[error("invalid Minecraft username {name}")]
    InvalidUsername { name: String },
    #[error("invalid dimension resource location {dimension}")]
    InvalidDimension { dimension: String },
    #[error("unknown game mode {value}")]
    UnknownGameMode { value: String },
    #[error("permission level {level} is outside 0..=4")]
    InvalidPermissionLevel { level: u8 },
    #[error("damage amount {amount} must be finite and non-negative")]
    InvalidDamage { amount: f32 },
    #[error("healing amount {amount} must be finite and non-negative")]
    InvalidHealing { amount: f32 },
    #[error("maximum health {max_health} must be finite and positive")]
    InvalidMaximumHealth { max_health: f32 },
    #[error("experience progress {progress} must be finite and in 0..1")]
    InvalidExperienceProgress { progress: f32 },
    #[error("experience level {level} exceeds the protocol maximum")]
    ExperienceLevelOutOfRange { level: u32 },
    #[error("total experience {total} exceeds the protocol maximum")]
    TotalExperienceOutOfRange { total: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity_id() -> EntityId {
        EntityId::new(1).unwrap()
    }

    #[test]
    fn validates_java_usernames() {
        assert!(validate_username("Steve").is_ok());
        assert!(validate_username("abc_123").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("not-valid!").is_err());
    }

    #[test]
    fn game_mode_updates_abilities_and_previous_mode() {
        let mut player = PlayerState::new(
            PlayerUuid::new(1),
            "Steve",
            entity_id(),
            "minecraft:overworld",
        )
        .unwrap();
        assert_eq!(player.set_game_mode(GameMode::Creative), GameMode::Survival);
        assert_eq!(player.previous_game_mode, Some(GameMode::Survival));
        assert!(player.abilities.allow_flying);
        assert!(player.abilities.instant_break);
    }

    #[test]
    fn damage_consumes_absorption_before_health() {
        let mut vitals = Vitals {
            absorption: 4.0,
            ..Vitals::default()
        };
        assert_eq!(vitals.damage(6.0).unwrap(), 18.0);
        assert_eq!(vitals.absorption, 0.0);
        assert_eq!(vitals.damage(100.0).unwrap(), 0.0);
        assert!(vitals.is_dead());
    }

    #[test]
    fn disconnect_and_reconnect_manage_entity_binding() {
        let mut player = PlayerState::new(
            PlayerUuid::new(1),
            "Alex",
            entity_id(),
            "minecraft:overworld",
        )
        .unwrap();
        player.disconnect();
        assert!(!player.connected);
        assert_eq!(player.entity_id, None);
        player.reconnect(EntityId::new(2).unwrap());
        assert!(player.connected);
        assert_eq!(player.entity_id, Some(EntityId::new(2).unwrap()));
    }

    #[test]
    fn uuid_json_is_safe_for_full_128_bit_values() {
        let uuid = PlayerUuid::new(u128::MAX);
        let json = serde_json::to_string(&uuid).unwrap();
        let decoded: PlayerUuid = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, uuid);
        assert_eq!(decoded.as_bytes(), &[0xff; 16]);
    }

    #[test]
    fn experience_points_cross_vanilla_level_boundaries() {
        let mut experience = Experience::default();
        experience.add_points(50).unwrap();
        assert_eq!(experience.level, 4);
        assert_eq!(experience.total, 50);
        assert!((experience.progress - (10.0 / 15.0)).abs() < f32::EPSILON);
        assert_eq!(experience.points_to_next_level(), 15);
    }
}
