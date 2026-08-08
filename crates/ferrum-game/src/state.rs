use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AttributeError, CombatError, ContainerClick, ContainerError, ContainerMutation,
    ContainerSnapshot, DamageContext, DamageKind, DamageSource, Difficulty, Entity, EntityError,
    EntityId, EntityPayload, EntityStore, EntityType, EntityUuid, EquipmentSlot, Experience,
    GameMode, InventoryError, ItemEntityData, ItemStack, LivingEntityData, PlayerError,
    PlayerState, PlayerUuid, StatusEffectError, StatusEffectInstance, Transform, Velocity, Vitals,
    calculate_damage, fall_damage, knockback_velocity,
};

pub const MAX_HOSTILE_MOBS: usize = 70;
pub const MIN_HOSTILE_MOB_SPAWN_DISTANCE: f64 = 24.0;
pub const MAX_HOSTILE_MOB_SPAWN_DISTANCE: f64 = 128.0;
pub const BASE_PLAYER_ATTACK_KNOCKBACK: f64 = 0.4;
const EXPERIENCE_ORB_FOLLOW_DISTANCE: f64 = 8.0;
const EXPERIENCE_ORB_FOLLOW_ACCELERATION: f64 = 0.1;
const EXPERIENCE_ORB_PICKUP_RADIUS: f64 = 1.5;
const PLAYER_EYE_HEIGHT: f64 = 1.62;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GameRuleValue {
    Boolean(bool),
    Integer(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameTime {
    pub game_time: u64,
    pub day_time: i64,
    pub daylight_cycle: bool,
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            game_time: 0,
            day_time: 0,
            daylight_cycle: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameEvent {
    PlayerConnected {
        uuid: PlayerUuid,
        name: String,
        entity_id: EntityId,
    },
    PlayerDisconnected {
        uuid: PlayerUuid,
        name: String,
        entity_id: Option<EntityId>,
    },
    PlayerMoved {
        uuid: PlayerUuid,
        entity_id: EntityId,
        transform: Transform,
    },
    PlayerTeleported {
        uuid: PlayerUuid,
        entity_id: EntityId,
        transform: Transform,
    },
    PlayerGameModeChanged {
        uuid: PlayerUuid,
        previous: GameMode,
        current: GameMode,
    },
    InventoryChanged {
        uuid: PlayerUuid,
        inserted: u32,
        item: String,
    },
    InventorySlotChanged {
        uuid: PlayerUuid,
        slot: usize,
        stack: Option<ItemStack>,
    },
    ContainerContentChanged {
        uuid: PlayerUuid,
        snapshot: ContainerSnapshot,
    },
    InventoryInteractionRejected {
        uuid: PlayerUuid,
        reason: String,
        snapshot: ContainerSnapshot,
    },
    ItemsDropped {
        uuid: PlayerUuid,
        stacks: Vec<ItemStack>,
    },
    EntitySpawned {
        entity: Entity,
    },
    EntityMoved {
        entity_id: EntityId,
        transform: Transform,
        velocity: Velocity,
    },
    LivingEntityDamaged {
        entity_id: EntityId,
        amount: f32,
        source: DamageSource,
        previous_health: f32,
        current_health: f32,
    },
    LivingEntityKilled {
        entity_id: EntityId,
        entity_type: String,
        source: DamageSource,
        dropped_stacks: usize,
    },
    EntityRemoved {
        entity_id: EntityId,
    },
    ItemEntityChanged {
        entity_id: EntityId,
        stack: ItemStack,
    },
    ItemPickedUp {
        uuid: PlayerUuid,
        entity_id: EntityId,
        item: String,
        inserted: u32,
    },
    ExperienceOrbPickedUp {
        uuid: PlayerUuid,
        entity_id: EntityId,
        value: u32,
    },
    PlayerExperienceChanged {
        uuid: PlayerUuid,
        experience: Experience,
    },
    PlayerVelocityChanged {
        uuid: PlayerUuid,
        entity_id: EntityId,
        velocity: Velocity,
    },
    PlayerAttributeChanged {
        uuid: PlayerUuid,
        attribute: String,
        value: f64,
    },
    PlayerStatusEffectChanged {
        uuid: PlayerUuid,
        effect: String,
        active: bool,
    },
    SelectedHotbarChanged {
        uuid: PlayerUuid,
        previous: u8,
        current: u8,
    },
    PlayerDamaged {
        uuid: PlayerUuid,
        entity_id: EntityId,
        amount: f32,
        source: DamageSource,
        previous: Vitals,
        current: Vitals,
    },
    PlayerVitalsChanged {
        uuid: PlayerUuid,
        vitals: Vitals,
    },
    PlayerKilled {
        uuid: PlayerUuid,
        entity_id: EntityId,
        name: String,
    },
    PlayerRespawned {
        uuid: PlayerUuid,
        entity_id: EntityId,
        transform: Transform,
        game_mode: GameMode,
        previous_game_mode: Option<GameMode>,
    },
    TimeChanged {
        day_time: i64,
    },
    SaveRequested,
    ShutdownRequested,
    Broadcast {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub(crate) players: BTreeMap<PlayerUuid, PlayerState>,
    pub(crate) player_names: BTreeMap<String, PlayerUuid>,
    pub(crate) entities: EntityStore,
    pub(crate) time: GameTime,
    pub(crate) difficulty: Difficulty,
    pub(crate) game_rules: BTreeMap<String, GameRuleValue>,
    pub(crate) dimension: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new("minecraft:overworld").expect("the built-in overworld resource location is valid")
    }
}

impl GameState {
    pub fn new(dimension: impl Into<String>) -> Result<Self, GameStateError> {
        let dimension = dimension.into();
        if !crate::validate_resource_location(&dimension) {
            return Err(GameStateError::InvalidDimension { dimension });
        }
        let mut game_rules = BTreeMap::new();
        game_rules.insert("doDaylightCycle".to_owned(), GameRuleValue::Boolean(true));
        game_rules.insert("doMobSpawning".to_owned(), GameRuleValue::Boolean(true));
        game_rules.insert("keepInventory".to_owned(), GameRuleValue::Boolean(false));
        game_rules.insert("randomTickSpeed".to_owned(), GameRuleValue::Integer(3));
        Ok(Self {
            players: BTreeMap::new(),
            player_names: BTreeMap::new(),
            entities: EntityStore::new(),
            time: GameTime::default(),
            difficulty: Difficulty::Normal,
            game_rules,
            dimension,
        })
    }

    #[must_use]
    pub fn players(&self) -> &BTreeMap<PlayerUuid, PlayerState> {
        &self.players
    }

    #[must_use]
    pub const fn entities(&self) -> &EntityStore {
        &self.entities
    }

    pub fn entities_mut(&mut self) -> &mut EntityStore {
        &mut self.entities
    }

    #[must_use]
    pub const fn time(&self) -> GameTime {
        self.time
    }

    #[must_use]
    pub const fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    #[must_use]
    pub fn game_rules(&self) -> &BTreeMap<String, GameRuleValue> {
        &self.game_rules
    }

    #[must_use]
    pub fn dimension(&self) -> &str {
        &self.dimension
    }

    pub fn connect_player(
        &mut self,
        uuid: PlayerUuid,
        name: impl Into<String>,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let name = name.into();
        crate::player::validate_username(&name)?;
        let normalized = normalize_player_name(&name);
        if let Some(existing) = self.player_names.get(&normalized) {
            if *existing != uuid {
                return Err(GameStateError::DuplicatePlayerName { name });
            }
        }

        if self
            .players
            .get(&uuid)
            .is_some_and(|player| player.connected)
        {
            return Err(GameStateError::PlayerAlreadyConnected { uuid });
        }

        let entity_id = self.entities.spawn(
            EntityUuid::new(uuid.get()),
            EntityType::new("minecraft:player")?,
            transform,
        )?;

        if let Some(player) = self.players.get_mut(&uuid) {
            let old_normalized = normalize_player_name(&player.name);
            if old_normalized != normalized {
                self.player_names.remove(&old_normalized);
            }
            player.name.clone_from(&name);
            player.dimension.clone_from(&self.dimension);
            player.reconnect(entity_id);
        } else {
            let player = PlayerState::new(uuid, name.clone(), entity_id, self.dimension.clone())?;
            self.players.insert(uuid, player);
        }
        self.player_names.insert(normalized, uuid);

        Ok(vec![GameEvent::PlayerConnected {
            uuid,
            name,
            entity_id,
        }])
    }

    pub fn disconnect_player(
        &mut self,
        uuid: PlayerUuid,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        if !player.connected {
            return Ok(Vec::new());
        }
        let name = player.name.clone();
        let entity_id = player.entity_id;
        if let Some(id) = entity_id {
            self.entities.despawn(id);
        }
        player.disconnect();
        Ok(vec![GameEvent::PlayerDisconnected {
            uuid,
            name,
            entity_id,
        }])
    }

    pub fn move_player(
        &mut self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let previous_transform = self
            .entities
            .get(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?
            .transform;
        let (landed_distance, safe_fall_distance, jump_boost_level) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            let fall_enabled = !player.abilities.flying
                && matches!(player.game_mode, GameMode::Survival | GameMode::Adventure);
            if !fall_enabled {
                player.fall_distance = 0.0;
                (0.0, 0.0, 0)
            } else if transform.on_ground {
                let landed = if previous_transform.on_ground {
                    0.0
                } else {
                    let final_descent =
                        (previous_transform.position[1] - transform.position[1]).max(0.0);
                    (f64::from(player.fall_distance) + final_descent).min(f64::from(f32::MAX))
                        as f32
                };
                player.fall_distance = 0.0;
                (
                    landed,
                    player
                        .attribute_value("minecraft:safe_fall_distance")
                        .unwrap_or(3.0),
                    player.status_effects.jump_boost_level(),
                )
            } else {
                let downward = (previous_transform.position[1] - transform.position[1]).max(0.0);
                player.fall_distance =
                    (f64::from(player.fall_distance) + downward).min(f64::from(f32::MAX)) as f32;
                (0.0, 0.0, 0)
            }
        };
        self.entities.set_transform(entity_id, transform)?;
        let mut events = vec![GameEvent::PlayerMoved {
            uuid,
            entity_id,
            transform,
        }];
        if landed_distance > 0.0 {
            let amount = fall_damage(landed_distance, safe_fall_distance, jump_boost_level)?;
            if amount > 0.0 {
                events.extend(self.damage_player_with_source(
                    uuid,
                    amount,
                    DamageSource::generic(DamageKind::Fall),
                )?);
            }
        }
        events.extend(self.pickup_nearby_items(uuid, 1.5)?);
        events.extend(self.pickup_nearby_experience_orbs(uuid, 1.5)?);
        Ok(events)
    }

    pub fn teleport_player(
        &mut self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        self.entities.set_transform(entity_id, transform)?;
        Ok(vec![GameEvent::PlayerTeleported {
            uuid,
            entity_id,
            transform,
        }])
    }

    pub fn set_game_mode(
        &mut self,
        uuid: PlayerUuid,
        game_mode: GameMode,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let previous = player.set_game_mode(game_mode);
        if previous == game_mode {
            return Ok(Vec::new());
        }
        Ok(vec![GameEvent::PlayerGameModeChanged {
            uuid,
            previous,
            current: game_mode,
        }])
    }

    pub fn give_item(
        &mut self,
        uuid: PlayerUuid,
        stack: ItemStack,
    ) -> Result<(Option<ItemStack>, Vec<GameEvent>), GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let item = stack.item().to_owned();
        let requested = stack.count();
        let (remainder, changed_slots) = player.inventory.insert_with_changed_slots(stack);
        let inserted = requested - remainder.as_ref().map_or(0, ItemStack::count);
        let mut events = Vec::with_capacity(changed_slots.len().saturating_add(1));
        if inserted > 0 {
            events.push(GameEvent::InventoryChanged {
                uuid,
                inserted,
                item,
            });
            events.extend(
                changed_slots
                    .into_iter()
                    .map(|slot| GameEvent::InventorySlotChanged {
                        uuid,
                        slot,
                        stack: player.inventory.slots()[slot].clone(),
                    }),
            );
        }
        Ok((remainder, events))
    }

    pub fn spawn_item_entity(
        &mut self,
        transform: Transform,
        stack: ItemStack,
        velocity: Velocity,
        owner: Option<PlayerUuid>,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let mut item = ItemEntityData::new(stack);
        item.owner = owner;
        let entity_id = self.entities.spawn_generated(
            EntityType::new("minecraft:item")?,
            transform,
            EntityPayload::Item(item),
        )?;
        self.entities.set_velocity(entity_id, velocity)?;
        let entity = self
            .entities
            .get(entity_id)
            .expect("newly spawned item entity exists")
            .clone();
        Ok(vec![GameEvent::EntitySpawned { entity }])
    }

    pub fn spawn_entity(
        &mut self,
        entity_type: EntityType,
        transform: Transform,
        velocity: Velocity,
        payload: EntityPayload,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        if entity_type.as_str() == "minecraft:player" {
            return Err(GameStateError::PlayerEntityRequiresConnection);
        }
        Velocity::new(velocity.0)?;
        let entity_id = self
            .entities
            .spawn_generated(entity_type, transform, payload)?;
        self.entities.set_velocity(entity_id, velocity)?;
        let entity = self
            .entities
            .get(entity_id)
            .expect("newly spawned entity exists")
            .clone();
        Ok(vec![GameEvent::EntitySpawned { entity }])
    }

    pub fn spawn_hostile_mob(
        &mut self,
        entity_type: EntityType,
        transform: Transform,
        max_health: f32,
        drops: Vec<ItemStack>,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        if entity_type.as_str() == "minecraft:player" {
            return Err(GameStateError::PlayerEntityRequiresConnection);
        }
        Transform::new(
            transform.position,
            transform.yaw,
            transform.pitch,
            transform.on_ground,
        )?;
        let living = LivingEntityData::new(max_health)?
            .with_drops(drops)?
            .with_hostile_ai();
        if !self.can_spawn_hostile_mob(transform) {
            return Ok(Vec::new());
        }
        self.spawn_entity(
            entity_type,
            transform,
            Velocity::default(),
            EntityPayload::Living(living),
        )
    }

    #[must_use]
    pub fn can_spawn_hostile_mob(&self, transform: Transform) -> bool {
        if self.difficulty == Difficulty::Peaceful
            || !matches!(
                self.game_rules.get("doMobSpawning"),
                Some(GameRuleValue::Boolean(true))
            )
            || self
                .entities
                .iter()
                .filter(|(_, entity)| {
                    entity
                        .living()
                        .and_then(|living| living.ai.as_ref())
                        .is_some()
                })
                .count()
                >= MAX_HOSTILE_MOBS
        {
            return false;
        }
        let minimum_squared = MIN_HOSTILE_MOB_SPAWN_DISTANCE.powi(2);
        let maximum_squared = MAX_HOSTILE_MOB_SPAWN_DISTANCE.powi(2);
        self.players.values().any(|player| {
            if !player.connected
                || player.vitals.is_dead()
                || !matches!(player.game_mode, GameMode::Survival | GameMode::Adventure)
            {
                return false;
            }
            let Some(entity_id) = player.entity_id else {
                return false;
            };
            let Some(entity) = self.entities.get(entity_id) else {
                return false;
            };
            let distance_squared = squared_distance(entity.transform.position, transform.position);
            (minimum_squared..=maximum_squared).contains(&distance_squared)
        })
    }

    pub fn move_entity(
        &mut self,
        entity_id: EntityId,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity = self
            .entities
            .get(entity_id)
            .ok_or(EntityError::UnknownEntity { id: entity_id })?;
        if entity.is_player() {
            return Err(GameStateError::PlayerEntityRequiresConnection);
        }
        let velocity = entity.velocity;
        if entity.transform == transform {
            return Ok(Vec::new());
        }
        self.entities.set_transform(entity_id, transform)?;
        Ok(vec![GameEvent::EntityMoved {
            entity_id,
            transform,
            velocity,
        }])
    }

    pub fn set_entity_velocity(
        &mut self,
        entity_id: EntityId,
        velocity: Velocity,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity = self
            .entities
            .get(entity_id)
            .ok_or(EntityError::UnknownEntity { id: entity_id })?;
        if entity.is_player() {
            return Err(GameStateError::PlayerEntityRequiresConnection);
        }
        let transform = entity.transform;
        if entity.velocity == velocity {
            return Ok(Vec::new());
        }
        self.entities.set_velocity(entity_id, velocity)?;
        Ok(vec![GameEvent::EntityMoved {
            entity_id,
            transform,
            velocity,
        }])
    }

    pub fn despawn_entity(
        &mut self,
        entity_id: EntityId,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity = self
            .entities
            .get(entity_id)
            .ok_or(EntityError::UnknownEntity { id: entity_id })?;
        if entity.is_player() {
            return Err(GameStateError::PlayerEntityRequiresConnection);
        }
        self.entities
            .despawn(entity_id)
            .expect("validated entity exists");
        Ok(vec![GameEvent::EntityRemoved { entity_id }])
    }

    pub fn damage_entity(
        &mut self,
        entity_id: EntityId,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        self.damage_entity_with_source(
            entity_id,
            amount,
            DamageSource::generic(DamageKind::Generic),
        )
    }

    pub fn attack_entity(
        &mut self,
        attacker_uuid: PlayerUuid,
        target_entity_id: EntityId,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let attacker_entity_id = self.connected_entity_id(attacker_uuid)?;
        let (attack_damage, interaction_range, attacker_position, can_attack) = {
            let attacker =
                self.players
                    .get(&attacker_uuid)
                    .ok_or(GameStateError::UnknownPlayer {
                        uuid: attacker_uuid,
                    })?;
            let attacker_entity = self.entities.get(attacker_entity_id).ok_or(
                GameStateError::PlayerMissingEntity {
                    uuid: attacker_uuid,
                },
            )?;
            (
                attacker
                    .attribute_value("minecraft:attack_damage")
                    .unwrap_or(1.0) as f32,
                attacker
                    .attribute_value("minecraft:entity_interaction_range")
                    .unwrap_or(3.0),
                attacker_entity.transform.position,
                !attacker.vitals.is_dead() && attacker.game_mode != GameMode::Spectator,
            )
        };
        if !can_attack {
            return Ok(Vec::new());
        }

        let Some(target) = self.entities.get(target_entity_id) else {
            // Entity removals and attacks may cross on the wire. Match Vanilla's
            // stale-target behavior by ignoring an entity that no longer exists.
            return Ok(Vec::new());
        };
        if target_entity_id == attacker_entity_id || !target.is_living() {
            return Err(GameStateError::InvalidAttackTarget {
                entity_id: target_entity_id,
            });
        }
        let target_position = target.transform.position;
        if squared_distance(attacker_position, target_position) > interaction_range.powi(2) {
            return Ok(Vec::new());
        }
        let target_player = target.is_player().then(|| {
            self.players
                .values()
                .find(|player| player.entity_id == Some(target_entity_id))
                .map(|player| player.uuid)
        });
        let target_player = match target_player {
            Some(Some(uuid)) => Some(uuid),
            Some(None) => {
                return Err(GameStateError::InvalidAttackTarget {
                    entity_id: target_entity_id,
                });
            }
            None => None,
        };
        let source = DamageSource {
            kind: DamageKind::PlayerAttack,
            attacker: Some(attacker_entity_id),
            direct_entity: Some(attacker_entity_id),
            bypasses_armor: false,
            bypasses_invulnerability: false,
        };
        let mut events = if let Some(target_uuid) = target_player {
            self.damage_player_with_source(target_uuid, attack_damage, source)?
        } else {
            self.damage_entity_with_source(target_entity_id, attack_damage, source)?
        };
        let survived = events.iter().any(|event| match event {
            GameEvent::PlayerDamaged {
                entity_id, current, ..
            } => *entity_id == target_entity_id && !current.is_dead(),
            GameEvent::LivingEntityDamaged {
                entity_id,
                current_health,
                ..
            } => *entity_id == target_entity_id && *current_health > 0.0,
            _ => false,
        });
        let direction_xz = [
            target_position[0] - attacker_position[0],
            target_position[2] - attacker_position[2],
        ];
        if survived && direction_xz[0].hypot(direction_xz[1]) > f64::EPSILON {
            let knockback = if let Some(target_uuid) = target_player {
                self.apply_knockback(target_uuid, direction_xz, BASE_PLAYER_ATTACK_KNOCKBACK)?
            } else {
                self.apply_entity_knockback(
                    target_entity_id,
                    direction_xz,
                    BASE_PLAYER_ATTACK_KNOCKBACK,
                )?
            };
            events.extend(knockback);
        }
        Ok(events)
    }

    pub fn damage_entity_with_source(
        &mut self,
        entity_id: EntityId,
        amount: f32,
        source: DamageSource,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let difficulty = self.difficulty;
        let (previous_health, current_health, final_damage, entity_type, transform, drops) = {
            let entity = self
                .entities
                .get_mut(entity_id)
                .ok_or(EntityError::UnknownEntity { id: entity_id })?;
            if entity.is_player() {
                return Err(GameStateError::PlayerEntityRequiresConnection);
            }
            let entity_type = entity.entity_type.as_str().to_owned();
            let transform = entity.transform;
            let living = entity
                .living_mut()
                .ok_or(GameStateError::NotLivingEntity { entity_id })?;
            if living.health <= 0.0 {
                return Ok(Vec::new());
            }
            let result = calculate_damage(DamageContext {
                raw_damage: amount,
                armor: living.attributes.value("minecraft:armor").unwrap_or(0.0),
                armor_toughness: living
                    .attributes
                    .value("minecraft:armor_toughness")
                    .unwrap_or(0.0),
                resistance_level: living.status_effects.damage_resistance_level(),
                difficulty,
                source,
            })?;
            if result.final_damage <= 0.0 {
                return Ok(Vec::new());
            }
            let previous_health = living.health;
            living.health = (living.health - result.final_damage).max(0.0);
            (
                previous_health,
                living.health,
                result.final_damage,
                entity_type,
                transform,
                living.drops.clone(),
            )
        };
        let mut events = vec![GameEvent::LivingEntityDamaged {
            entity_id,
            amount: final_damage,
            source,
            previous_health,
            current_health,
        }];
        if current_health > 0.0 {
            return Ok(events);
        }

        self.entities
            .despawn(entity_id)
            .expect("damaged living entity exists until fatal damage is finalized");
        events.push(GameEvent::LivingEntityKilled {
            entity_id,
            entity_type,
            source,
            dropped_stacks: drops.len(),
        });
        events.push(GameEvent::EntityRemoved { entity_id });
        let drop_transform = Transform {
            on_ground: false,
            ..transform
        };
        for (index, stack) in drops.into_iter().enumerate() {
            let horizontal = (f64::from((index % 3) as u8) - 1.0) * 0.05;
            events.extend(self.spawn_item_entity(
                drop_transform,
                stack,
                Velocity::new([horizontal, 0.2, -horizontal])?,
                None,
            )?);
        }
        Ok(events)
    }

    pub fn apply_entity_knockback(
        &mut self,
        entity_id: EntityId,
        direction_xz: [f64; 2],
        strength: f64,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity = self
            .entities
            .get(entity_id)
            .ok_or(EntityError::UnknownEntity { id: entity_id })?;
        if entity.is_player() {
            return Err(GameStateError::PlayerEntityRequiresConnection);
        }
        let living = entity
            .living()
            .ok_or(GameStateError::NotLivingEntity { entity_id })?;
        let resistance = living
            .attributes
            .value("minecraft:knockback_resistance")
            .unwrap_or(0.0);
        let transform = entity.transform;
        let velocity = knockback_velocity(entity.velocity, direction_xz, strength, resistance)?;
        self.entities.set_velocity(entity_id, velocity)?;
        Ok(vec![GameEvent::EntityMoved {
            entity_id,
            transform,
            velocity,
        }])
    }

    pub fn pickup_nearby_items(
        &mut self,
        uuid: PlayerUuid,
        radius: f64,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        if !radius.is_finite() || !(0.0..=16.0).contains(&radius) {
            return Err(GameStateError::InvalidPickupRadius { radius });
        }
        let entity_id = self.connected_entity_id(uuid)?;
        let position = self
            .entities
            .get(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?
            .transform
            .position;
        if self
            .players
            .get(&uuid)
            .is_some_and(|player| player.vitals.is_dead())
        {
            return Ok(Vec::new());
        }
        let radius_squared = radius * radius;
        let candidates = self
            .entities
            .iter()
            .filter_map(|(&candidate_id, entity)| {
                let item = entity.item()?;
                if !item.can_pick_up() {
                    return None;
                }
                let distance_squared = entity
                    .transform
                    .position
                    .into_iter()
                    .zip(position)
                    .map(|(value, origin)| (value - origin).powi(2))
                    .sum::<f64>();
                (distance_squared <= radius_squared).then(|| (candidate_id, item.stack.clone()))
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        let mut inventory_changed = false;
        for (candidate_id, stack) in candidates {
            let requested = stack.count();
            let item_name = stack.item().to_owned();
            let (remainder, changed_slots) = {
                let player = self
                    .players
                    .get_mut(&uuid)
                    .ok_or(GameStateError::UnknownPlayer { uuid })?;
                player.inventory.insert_with_changed_slots(stack)
            };
            let inserted = requested - remainder.as_ref().map_or(0, ItemStack::count);
            if inserted == 0 {
                continue;
            }
            inventory_changed = true;
            let remainder = if let Some(remainder) = remainder {
                let entity = self.entities.get_mut(candidate_id).ok_or(
                    GameStateError::MissingItemEntity {
                        entity_id: candidate_id,
                    },
                )?;
                entity
                    .item_mut()
                    .ok_or(GameStateError::NotItemEntity {
                        entity_id: candidate_id,
                    })?
                    .stack = remainder.clone();
                Some(remainder)
            } else {
                self.entities.despawn(candidate_id);
                None
            };
            events.push(GameEvent::InventoryChanged {
                uuid,
                inserted,
                item: item_name.clone(),
            });
            let player = self
                .players
                .get(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            events.extend(
                changed_slots
                    .into_iter()
                    .map(|slot| GameEvent::InventorySlotChanged {
                        uuid,
                        slot,
                        stack: player.inventory.slots()[slot].clone(),
                    }),
            );
            events.push(GameEvent::ItemPickedUp {
                uuid,
                entity_id: candidate_id,
                item: item_name,
                inserted,
            });
            if let Some(stack) = remainder {
                events.push(GameEvent::ItemEntityChanged {
                    entity_id: candidate_id,
                    stack,
                });
            } else {
                events.push(GameEvent::EntityRemoved {
                    entity_id: candidate_id,
                });
            }
        }
        if inventory_changed {
            let player = self
                .players
                .get(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            events.push(GameEvent::ContainerContentChanged {
                uuid,
                snapshot: player.inventory_session.snapshot(&player.inventory),
            });
        }
        Ok(events)
    }

    pub fn pickup_nearby_experience_orbs(
        &mut self,
        uuid: PlayerUuid,
        radius: f64,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        if !radius.is_finite() || !(0.0..=16.0).contains(&radius) {
            return Err(GameStateError::InvalidPickupRadius { radius });
        }
        let entity_id = self.connected_entity_id(uuid)?;
        let position = self
            .entities
            .get(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?
            .transform
            .position;
        let player = self
            .players
            .get(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        if player.vitals.is_dead()
            || player.game_mode == GameMode::Spectator
            || player.experience_pickup_delay_ticks > 0
        {
            return Ok(Vec::new());
        }

        let radius_squared = radius * radius;
        let candidate = self.entities.iter().find_map(|(&candidate_id, entity)| {
            let EntityPayload::ExperienceOrb { value } = &entity.payload else {
                return None;
            };
            let distance_squared = squared_distance(entity.transform.position, position);
            (distance_squared <= radius_squared).then_some((candidate_id, *value))
        });
        let Some((orb_entity_id, value)) = candidate else {
            return Ok(Vec::new());
        };

        let mut experience = self
            .players
            .get(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?
            .experience;
        experience.add_points(value)?;
        self.entities
            .despawn(orb_entity_id)
            .ok_or(GameStateError::MissingExperienceOrb {
                entity_id: orb_entity_id,
            })?;
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        player.experience_pickup_delay_ticks = 2;
        player.experience = experience;
        Ok(vec![
            GameEvent::ExperienceOrbPickedUp {
                uuid,
                entity_id: orb_entity_id,
                value,
            },
            GameEvent::PlayerExperienceChanged { uuid, experience },
            GameEvent::EntityRemoved {
                entity_id: orb_entity_id,
            },
        ])
    }

    pub fn apply_knockback(
        &mut self,
        uuid: PlayerUuid,
        direction_xz: [f64; 2],
        strength: f64,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let resistance = self
            .players
            .get(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?
            .attribute_value("minecraft:knockback_resistance")
            .unwrap_or(0.0);
        let current = self
            .entities
            .get(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?
            .velocity;
        let velocity = knockback_velocity(current, direction_xz, strength, resistance)?;
        self.entities.set_velocity(entity_id, velocity)?;
        Ok(vec![GameEvent::PlayerVelocityChanged {
            uuid,
            entity_id,
            velocity,
        }])
    }

    pub fn set_player_attribute_base(
        &mut self,
        uuid: PlayerUuid,
        attribute: &str,
        value: f64,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let instance = player.attributes.get_mut(attribute).ok_or_else(|| {
            GameStateError::UnknownAttribute {
                attribute: attribute.to_owned(),
            }
        })?;
        instance.set_base(value)?;
        let current = instance.value();
        if attribute == "minecraft:max_health" {
            player.vitals.health = player.vitals.health.min(current as f32);
        }
        Ok(vec![GameEvent::PlayerAttributeChanged {
            uuid,
            attribute: attribute.to_owned(),
            value: current,
        }])
    }

    pub fn add_status_effect(
        &mut self,
        uuid: PlayerUuid,
        effect: StatusEffectInstance,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let effect_id = effect.effect.as_str().to_owned();
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        player.status_effects.insert(effect)?;
        Ok(vec![GameEvent::PlayerStatusEffectChanged {
            uuid,
            effect: effect_id,
            active: true,
        }])
    }

    pub fn remove_status_effect(
        &mut self,
        uuid: PlayerUuid,
        effect: &str,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        if player.status_effects.remove(effect).is_none() {
            return Ok(Vec::new());
        }
        Ok(vec![GameEvent::PlayerStatusEffectChanged {
            uuid,
            effect: effect.to_owned(),
            active: false,
        }])
    }

    pub fn click_container(
        &mut self,
        uuid: PlayerUuid,
        click: ContainerClick,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let (mut events, dropped) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            let creative = player.game_mode == GameMode::Creative;
            let mutation =
                player
                    .inventory_session
                    .click(&mut player.inventory, click, creative)?;
            let dropped = mutation.dropped.clone();
            (container_events(uuid, &player.inventory, mutation), dropped)
        };
        events.extend(self.spawn_dropped_item_entities(uuid, dropped)?);
        Ok(events)
    }

    pub fn close_container(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {
        let (mut events, dropped) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            let mutation = player
                .inventory_session
                .close_container(&mut player.inventory);
            let dropped = mutation.dropped.clone();
            (container_events(uuid, &player.inventory, mutation), dropped)
        };
        events.extend(self.spawn_dropped_item_entities(uuid, dropped)?);
        Ok(events)
    }

    pub fn open_container(
        &mut self,
        uuid: PlayerUuid,
        container_id: i32,
        slots: Vec<Option<ItemStack>>,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let snapshot =
            player
                .inventory_session
                .open_container(container_id, slots, &player.inventory)?;
        Ok(vec![GameEvent::ContainerContentChanged { uuid, snapshot }])
    }

    pub fn set_creative_inventory_slot(
        &mut self,
        uuid: PlayerUuid,
        slot: i16,
        stack: Option<ItemStack>,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let (mut events, dropped) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            let mutation = player.inventory_session.set_creative_slot(
                &mut player.inventory,
                slot,
                stack,
                player.game_mode == GameMode::Creative,
            )?;
            let dropped = mutation.dropped.clone();
            (container_events(uuid, &player.inventory, mutation), dropped)
        };
        events.extend(self.spawn_dropped_item_entities(uuid, dropped)?);
        Ok(events)
    }

    pub fn clear_inventory(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let before = player.inventory.slots().to_vec();
        player.inventory.clear();
        let mut events = slot_diff_events(uuid, &before, player.inventory.slots());
        events.push(GameEvent::ContainerContentChanged {
            uuid,
            snapshot: player.inventory_session.snapshot(&player.inventory),
        });
        Ok(events)
    }

    pub fn swap_inventory_slots(
        &mut self,
        uuid: PlayerUuid,
        first: usize,
        second: usize,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let before = player.inventory.slots().to_vec();
        player.inventory.swap_slots(first, second)?;
        let mut events = slot_diff_events(uuid, &before, player.inventory.slots());
        events.push(GameEvent::ContainerContentChanged {
            uuid,
            snapshot: player.inventory_session.snapshot(&player.inventory),
        });
        Ok(events)
    }

    pub fn remove_inventory_item(
        &mut self,
        uuid: PlayerUuid,
        item: &str,
        count: u32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let before = player.inventory.slots().to_vec();
        player.inventory.remove_item(item, count);
        let mut events = slot_diff_events(uuid, &before, player.inventory.slots());
        if !events.is_empty() {
            events.push(GameEvent::ContainerContentChanged {
                uuid,
                snapshot: player.inventory_session.snapshot(&player.inventory),
            });
        }
        Ok(events)
    }

    pub fn select_hotbar(
        &mut self,
        uuid: PlayerUuid,
        selected_hotbar: u8,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let previous = player.inventory.selected_hotbar();
        player.inventory.select_hotbar(selected_hotbar)?;
        if previous == selected_hotbar {
            return Ok(Vec::new());
        }
        Ok(vec![GameEvent::SelectedHotbarChanged {
            uuid,
            previous,
            current: selected_hotbar,
        }])
    }

    pub fn consume_equipped_item(
        &mut self,
        uuid: PlayerUuid,
        slot: EquipmentSlot,
        expected_item: &str,
        amount: u32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        self.connected_entity_id(uuid)?;
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let inventory_slot = slot.inventory_index(player.inventory.selected_hotbar());
        let mut stack = player
            .inventory
            .slot(inventory_slot)?
            .cloned()
            .ok_or(GameStateError::MissingEquippedItem { uuid, slot })?;
        if stack.item() != expected_item {
            return Err(GameStateError::UnexpectedEquippedItem {
                uuid,
                slot,
                expected: expected_item.to_owned(),
                actual: stack.item().to_owned(),
            });
        }
        let became_empty = stack.consume(amount)?;
        player
            .inventory
            .set_slot(inventory_slot, (!became_empty).then_some(stack))?;
        Ok(vec![
            GameEvent::InventorySlotChanged {
                uuid,
                slot: inventory_slot,
                stack: player.inventory.slots()[inventory_slot].clone(),
            },
            GameEvent::ContainerContentChanged {
                uuid,
                snapshot: player.inventory_session.snapshot(&player.inventory),
            },
        ])
    }

    pub fn damage_equipped_item(
        &mut self,
        uuid: PlayerUuid,
        slot: EquipmentSlot,
        expected_item: &str,
        amount: u32,
        max_damage: u32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        self.connected_entity_id(uuid)?;
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let inventory_slot = slot.inventory_index(player.inventory.selected_hotbar());
        let mut stack = player
            .inventory
            .slot(inventory_slot)?
            .cloned()
            .ok_or(GameStateError::MissingEquippedItem { uuid, slot })?;
        if stack.item() != expected_item {
            return Err(GameStateError::UnexpectedEquippedItem {
                uuid,
                slot,
                expected: expected_item.to_owned(),
                actual: stack.item().to_owned(),
            });
        }
        let broke = stack.apply_durability_damage(amount, max_damage)?;
        player
            .inventory
            .set_slot(inventory_slot, (!broke).then_some(stack))?;
        Ok(vec![
            GameEvent::InventorySlotChanged {
                uuid,
                slot: inventory_slot,
                stack: player.inventory.slots()[inventory_slot].clone(),
            },
            GameEvent::ContainerContentChanged {
                uuid,
                snapshot: player.inventory_session.snapshot(&player.inventory),
            },
        ])
    }

    pub fn drop_equipped_item(
        &mut self,
        uuid: PlayerUuid,
        slot: EquipmentSlot,
        whole_stack: bool,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let source = self
            .entities
            .get(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?
            .transform;
        let inventory_slot = self
            .players
            .get(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?
            .inventory
            .selected_hotbar();
        let inventory_slot = slot.inventory_index(inventory_slot);
        let stack = self
            .players
            .get(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?
            .inventory
            .slot(inventory_slot)?
            .cloned()
            .ok_or(GameStateError::MissingEquippedItem { uuid, slot })?;
        let dropped_count = if whole_stack { stack.count() } else { 1 };
        let dropped = stack.copy_with_count(dropped_count)?;
        let remaining = (dropped_count < stack.count())
            .then(|| stack.copy_with_count(stack.count() - dropped_count))
            .transpose()?;

        let yaw = f64::from(source.yaw).to_radians();
        let pitch = f64::from(source.pitch).to_radians();
        let horizontal = pitch.cos() * 0.3;
        let velocity = Velocity::new([
            -yaw.sin() * horizontal,
            -pitch.sin() * 0.3 + 0.1,
            yaw.cos() * horizontal,
        ])?;
        let transform = Transform::new(
            [
                source.position[0],
                source.position[1] + 1.3,
                source.position[2],
            ],
            source.yaw,
            source.pitch,
            false,
        )?;

        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        player.inventory.set_slot(inventory_slot, remaining)?;
        let mut events = vec![
            GameEvent::InventorySlotChanged {
                uuid,
                slot: inventory_slot,
                stack: player.inventory.slots()[inventory_slot].clone(),
            },
            GameEvent::ContainerContentChanged {
                uuid,
                snapshot: player.inventory_session.snapshot(&player.inventory),
            },
            GameEvent::ItemsDropped {
                uuid,
                stacks: vec![dropped.clone()],
            },
        ];
        events.extend(self.spawn_item_entity(transform, dropped, velocity, Some(uuid))?);
        Ok(events)
    }

    pub fn swap_equipped_items(
        &mut self,
        uuid: PlayerUuid,
        first: EquipmentSlot,
        second: EquipmentSlot,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        self.connected_entity_id(uuid)?;
        let selected_hotbar = self
            .players
            .get(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?
            .inventory
            .selected_hotbar();
        self.swap_inventory_slots(
            uuid,
            first.inventory_index(selected_hotbar),
            second.inventory_index(selected_hotbar),
        )
    }

    pub fn damage_player(
        &mut self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        self.damage_player_with_source(uuid, amount, DamageSource::generic(DamageKind::Generic))
    }

    pub fn damage_player_with_source(
        &mut self,
        uuid: PlayerUuid,
        amount: f32,
        source: DamageSource,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let (previous, current, final_damage) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if (player.abilities.invulnerable && !source.bypasses_invulnerability)
                || player.vitals.is_dead()
            {
                return Ok(Vec::new());
            }
            let result = calculate_damage(DamageContext {
                raw_damage: amount,
                armor: player.attribute_value("minecraft:armor").unwrap_or(0.0),
                armor_toughness: player
                    .attribute_value("minecraft:armor_toughness")
                    .unwrap_or(0.0),
                resistance_level: player.status_effects.damage_resistance_level(),
                difficulty: self.difficulty,
                source,
            })?;
            if result.final_damage <= 0.0 {
                return Ok(Vec::new());
            }
            let previous = player.vitals;
            player.vitals.damage(result.final_damage)?;
            (previous, player.vitals, result.final_damage)
        };
        let mut events = vec![
            GameEvent::PlayerDamaged {
                uuid,
                entity_id,
                amount: final_damage,
                source,
                previous,
                current,
            },
            GameEvent::PlayerVitalsChanged {
                uuid,
                vitals: current,
            },
        ];
        if !previous.is_dead() && current.is_dead() {
            events.extend(self.finish_player_death(uuid, entity_id)?);
        }
        Ok(events)
    }

    pub fn heal_player(
        &mut self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        self.connected_entity_id(uuid)?;
        let (previous, current) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if player.vitals.is_dead() {
                return Err(GameStateError::PlayerDead { uuid });
            }
            let previous = player.vitals;
            let max_health = player.max_health();
            player.vitals.heal_to_max(amount, max_health)?;
            (previous, player.vitals)
        };
        if previous == current {
            return Ok(Vec::new());
        }
        Ok(vec![GameEvent::PlayerVitalsChanged {
            uuid,
            vitals: current,
        }])
    }

    pub fn kill_player(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let current = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if player.vitals.is_dead() {
                return Ok(Vec::new());
            }
            player.vitals.health = 0.0;
            player.vitals
        };
        let mut events = vec![GameEvent::PlayerVitalsChanged {
            uuid,
            vitals: current,
        }];
        events.extend(self.finish_player_death(uuid, entity_id)?);
        Ok(events)
    }

    pub fn respawn_player(
        &mut self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let (game_mode, previous_game_mode, vitals) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if !player.vitals.is_dead() {
                return Err(GameStateError::PlayerAlive { uuid });
            }
            let previous_game_mode = player.previous_game_mode;
            player.vitals = Vitals::default();
            player.vitals.health = player.max_health();
            player.fall_distance = 0.0;
            (player.game_mode, previous_game_mode, player.vitals)
        };
        let entity = self
            .entities
            .get_mut(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?;
        entity.transform = transform;
        entity.velocity = crate::Velocity::default();
        Ok(vec![
            GameEvent::PlayerRespawned {
                uuid,
                entity_id,
                transform,
                game_mode,
                previous_game_mode,
            },
            GameEvent::PlayerVitalsChanged { uuid, vitals },
        ])
    }

    fn finish_player_death(
        &mut self,
        uuid: PlayerUuid,
        entity_id: EntityId,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let keep_inventory = matches!(
            self.game_rules.get("keepInventory"),
            Some(GameRuleValue::Boolean(true))
        );
        let (name, transform, stacks, inventory_events) = {
            let transform = self
                .entities
                .get(entity_id)
                .ok_or(GameStateError::PlayerMissingEntity { uuid })?
                .transform;
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            player.fall_distance = 0.0;
            if keep_inventory {
                (player.name.clone(), transform, Vec::new(), Vec::new())
            } else {
                let before = player.inventory.slots().to_vec();
                let stacks = player.inventory.drain();
                let mut inventory_events =
                    slot_diff_events(uuid, &before, player.inventory.slots());
                inventory_events.push(GameEvent::ContainerContentChanged {
                    uuid,
                    snapshot: player.inventory_session.snapshot(&player.inventory),
                });
                (player.name.clone(), transform, stacks, inventory_events)
            }
        };
        let mut events = vec![GameEvent::PlayerKilled {
            uuid,
            entity_id,
            name,
        }];
        events.extend(inventory_events);
        if !stacks.is_empty() {
            events.push(GameEvent::ItemsDropped {
                uuid,
                stacks: stacks.clone(),
            });
            events.extend(self.spawn_dropped_item_entities_at(uuid, transform, stacks)?);
        }
        Ok(events)
    }

    fn spawn_dropped_item_entities(
        &mut self,
        uuid: PlayerUuid,
        stacks: Vec<ItemStack>,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        if stacks.is_empty() {
            return Ok(Vec::new());
        }
        let entity_id = self.connected_entity_id(uuid)?;
        let transform = self
            .entities
            .get(entity_id)
            .ok_or(GameStateError::PlayerMissingEntity { uuid })?
            .transform;
        self.spawn_dropped_item_entities_at(uuid, transform, stacks)
    }

    fn spawn_dropped_item_entities_at(
        &mut self,
        uuid: PlayerUuid,
        transform: Transform,
        stacks: Vec<ItemStack>,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let mut events = Vec::with_capacity(stacks.len());
        for (index, stack) in stacks.into_iter().enumerate() {
            let phase = (index % 8) as f64 * std::f64::consts::TAU / 8.0;
            let velocity = Velocity::new([phase.cos() * 0.1, 0.2, phase.sin() * 0.1])?;
            events.extend(self.spawn_item_entity(transform, stack, velocity, Some(uuid))?);
        }
        Ok(events)
    }

    pub fn set_day_time(&mut self, day_time: i64) -> Vec<GameEvent> {
        self.time.day_time = day_time;
        vec![GameEvent::TimeChanged { day_time }]
    }

    pub fn set_difficulty(&mut self, difficulty: Difficulty) -> Difficulty {
        std::mem::replace(&mut self.difficulty, difficulty)
    }

    pub fn set_game_rule(&mut self, name: impl Into<String>, value: GameRuleValue) {
        let name = name.into();
        if name == "doDaylightCycle" {
            if let GameRuleValue::Boolean(enabled) = &value {
                self.time.daylight_cycle = *enabled;
            }
        }
        self.game_rules.insert(name, value);
    }

    pub fn detach_all_connections(&mut self) -> usize {
        let entity_ids = self
            .players
            .values_mut()
            .filter_map(|player| {
                if !player.connected {
                    return None;
                }
                let entity_id = player.entity_id;
                player.disconnect();
                entity_id
            })
            .collect::<Vec<_>>();
        for entity_id in &entity_ids {
            self.entities.despawn(*entity_id);
        }
        entity_ids.len()
    }

    fn tick_mob_ai(&mut self) -> Vec<GameEvent> {
        let targets = self
            .players
            .values()
            .filter(|player| {
                player.connected
                    && !player.vitals.is_dead()
                    && matches!(player.game_mode, GameMode::Survival | GameMode::Adventure)
            })
            .filter_map(|player| {
                let entity = self.entities.get(player.entity_id?)?;
                Some((player.uuid, entity.transform.position))
            })
            .collect::<Vec<_>>();
        let mobs = self
            .entities
            .iter()
            .filter_map(|(&entity_id, entity)| {
                entity
                    .living()
                    .and_then(|living| living.ai.as_ref())
                    .map(|ai| (entity_id, entity.transform, entity.velocity, ai.clone()))
            })
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        for (entity_id, previous_transform, previous_velocity, ai) in mobs {
            let target = targets
                .iter()
                .filter_map(|(uuid, position)| {
                    let distance_squared = squared_distance(previous_transform.position, *position);
                    (distance_squared <= ai.follow_range.powi(2)).then_some((
                        *uuid,
                        *position,
                        distance_squared,
                    ))
                })
                .min_by(|left, right| {
                    left.2
                        .total_cmp(&right.2)
                        .then_with(|| left.0.cmp(&right.0))
                });
            let mut transform = previous_transform;
            let mut velocity = previous_velocity;
            let mut cooldown = ai.attack_cooldown_ticks.saturating_sub(1);
            let mut attack = None;
            if let Some((target_uuid, target_position, distance_squared)) = target {
                let delta_x = target_position[0] - transform.position[0];
                let delta_z = target_position[2] - transform.position[2];
                let horizontal_distance = delta_x.hypot(delta_z);
                if distance_squared <= ai.attack_range.powi(2) {
                    velocity.0[0] = 0.0;
                    velocity.0[2] = 0.0;
                    if cooldown == 0 {
                        cooldown = ai.attack_interval_ticks;
                        attack = Some((target_uuid, [delta_x, delta_z], ai.attack_damage));
                    }
                } else if horizontal_distance > f64::EPSILON {
                    let step = ai.movement_speed.min(horizontal_distance);
                    velocity.0[0] = delta_x / horizontal_distance * step;
                    velocity.0[2] = delta_z / horizontal_distance * step;
                    transform.position[0] += velocity.0[0];
                    transform.position[2] += velocity.0[2];
                    transform.yaw = (-delta_x).atan2(delta_z).to_degrees() as f32;
                }
            } else {
                velocity.0[0] = 0.0;
                velocity.0[2] = 0.0;
            }

            if let Some(entity) = self.entities.get_mut(entity_id) {
                if let Some(current_ai) = entity.living_mut().and_then(|living| living.ai.as_mut())
                {
                    current_ai.target = target.map(|(uuid, _, _)| uuid);
                    current_ai.attack_cooldown_ticks = cooldown;
                }
                entity.transform = transform;
                entity.velocity = velocity;
            }
            if transform != previous_transform || velocity != previous_velocity {
                events.push(GameEvent::EntityMoved {
                    entity_id,
                    transform,
                    velocity,
                });
            }
            if let Some((target_uuid, direction_xz, damage)) = attack {
                let source = DamageSource {
                    kind: DamageKind::MobAttack,
                    attacker: Some(entity_id),
                    direct_entity: Some(entity_id),
                    bypasses_armor: false,
                    bypasses_invulnerability: false,
                };
                if let Ok(mut attack_events) =
                    self.damage_player_with_source(target_uuid, damage, source)
                {
                    let damaged = attack_events
                        .iter()
                        .any(|event| matches!(event, GameEvent::PlayerDamaged { .. }));
                    events.append(&mut attack_events);
                    if damaged
                        && let Ok(mut knockback_events) =
                            self.apply_knockback(target_uuid, direction_xz, 0.4)
                    {
                        events.append(&mut knockback_events);
                    }
                }
            }
        }
        events
    }

    fn tick_experience_orb_attraction(&mut self) {
        let targets = self
            .players
            .values()
            .filter(|player| {
                player.connected
                    && !player.vitals.is_dead()
                    && player.game_mode != GameMode::Spectator
            })
            .filter_map(|player| {
                let entity = self.entities.get(player.entity_id?)?;
                Some((player.uuid, entity.transform.position))
            })
            .collect::<Vec<_>>();
        let orbs = self
            .entities
            .iter()
            .filter_map(|(&entity_id, entity)| {
                matches!(entity.payload, EntityPayload::ExperienceOrb { .. }).then_some((
                    entity_id,
                    entity.transform.position,
                    entity.velocity,
                ))
            })
            .collect::<Vec<_>>();
        let follow_distance_squared = EXPERIENCE_ORB_FOLLOW_DISTANCE.powi(2);

        for (entity_id, orb_position, previous_velocity) in orbs {
            let Some((_, target_position)) = targets
                .iter()
                .filter_map(|(uuid, position)| {
                    let distance_squared = squared_distance(orb_position, *position);
                    (distance_squared <= follow_distance_squared).then_some((
                        *uuid,
                        *position,
                        distance_squared,
                    ))
                })
                .min_by(|left, right| {
                    left.2
                        .total_cmp(&right.2)
                        .then_with(|| left.0.cmp(&right.0))
                })
                .map(|(uuid, position, _)| (uuid, position))
            else {
                continue;
            };

            let direction = [
                target_position[0] - orb_position[0],
                target_position[1] + PLAYER_EYE_HEIGHT / 2.0 - orb_position[1],
                target_position[2] - orb_position[2],
            ];
            let length_squared = direction
                .iter()
                .map(|component| component * component)
                .sum();
            if length_squared <= f64::EPSILON {
                continue;
            }
            let length = f64::sqrt(length_squared);
            let distance_factor = 1.0 - length / EXPERIENCE_ORB_FOLLOW_DISTANCE;
            let acceleration = distance_factor.powi(2) * EXPERIENCE_ORB_FOLLOW_ACCELERATION;
            let velocity = Velocity([
                previous_velocity.0[0] + direction[0] / length * acceleration,
                previous_velocity.0[1] + direction[1] / length * acceleration,
                previous_velocity.0[2] + direction[2] / length * acceleration,
            ]);
            if let Some(entity) = self.entities.get_mut(entity_id) {
                entity.velocity = velocity;
                if velocity.0[1] > 0.0 {
                    entity.transform.on_ground = false;
                }
            }
        }
    }

    pub fn tick(&mut self) -> Vec<GameEvent> {
        self.time.game_time = self.time.game_time.saturating_add(1);
        if self.time.daylight_cycle {
            self.time.day_time = self.time.day_time.saturating_add(1);
        }
        let mut events = Vec::new();
        for (&uuid, player) in &mut self.players {
            player.tick_experience_pickup_delay();
            for effect in player.tick_status_effects() {
                events.push(GameEvent::PlayerStatusEffectChanged {
                    uuid,
                    effect: effect.effect.as_str().to_owned(),
                    active: false,
                });
            }
        }
        let previous_motion = self
            .entities
            .iter()
            .filter(|(_, entity)| !entity.is_player())
            .map(|(&entity_id, entity)| (entity_id, (entity.transform, entity.velocity)))
            .collect::<BTreeMap<_, _>>();
        self.tick_experience_orb_attraction();
        let removed = self.entities.tick();
        for (entity_id, (previous_transform, previous_velocity)) in previous_motion {
            let Some(entity) = self.entities.get(entity_id) else {
                continue;
            };
            if entity.transform != previous_transform || entity.velocity != previous_velocity {
                events.push(GameEvent::EntityMoved {
                    entity_id,
                    transform: entity.transform,
                    velocity: entity.velocity,
                });
            }
        }
        events.extend(self.tick_mob_ai());
        events.extend(
            removed
                .into_iter()
                .map(|entity_id| GameEvent::EntityRemoved { entity_id }),
        );
        let pickup_players = self
            .players
            .values()
            .filter(|player| player.connected)
            .map(|player| player.uuid)
            .collect::<Vec<_>>();
        for uuid in pickup_players {
            if let Ok(mut pickup_events) =
                self.pickup_nearby_experience_orbs(uuid, EXPERIENCE_ORB_PICKUP_RADIUS)
            {
                events.append(&mut pickup_events);
            }
        }
        events
    }

    #[must_use]
    pub fn player(&self, uuid: PlayerUuid) -> Option<&PlayerState> {
        self.players.get(&uuid)
    }

    pub fn player_mut(&mut self, uuid: PlayerUuid) -> Option<&mut PlayerState> {
        self.players.get_mut(&uuid)
    }

    #[must_use]
    pub fn player_uuid_by_name(&self, name: &str) -> Option<PlayerUuid> {
        self.player_names.get(&normalize_player_name(name)).copied()
    }

    #[must_use]
    pub fn online_player_count(&self) -> usize {
        self.players
            .values()
            .filter(|player| player.connected)
            .count()
    }

    fn connected_entity_id(&self, uuid: PlayerUuid) -> Result<EntityId, GameStateError> {
        let player = self
            .players
            .get(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        if !player.connected {
            return Err(GameStateError::PlayerNotConnected { uuid });
        }
        player
            .entity_id
            .ok_or(GameStateError::PlayerMissingEntity { uuid })
    }
}

fn container_events(
    uuid: PlayerUuid,
    inventory: &crate::Inventory,
    mutation: ContainerMutation,
) -> Vec<GameEvent> {
    let mut events = mutation
        .changed_player_slots
        .iter()
        .map(|&slot| GameEvent::InventorySlotChanged {
            uuid,
            slot,
            stack: inventory.slots()[slot].clone(),
        })
        .collect::<Vec<_>>();
    if mutation.accepted {
        events.push(GameEvent::ContainerContentChanged {
            uuid,
            snapshot: mutation.snapshot,
        });
    } else {
        events.push(GameEvent::InventoryInteractionRejected {
            uuid,
            reason: mutation
                .reason
                .unwrap_or_else(|| "inventory interaction rejected".to_owned()),
            snapshot: mutation.snapshot,
        });
    }
    if !mutation.dropped.is_empty() {
        events.push(GameEvent::ItemsDropped {
            uuid,
            stacks: mutation.dropped,
        });
    }
    events
}

#[allow(clippy::filter_map_bool_then)]
fn slot_diff_events(
    uuid: PlayerUuid,
    before: &[Option<ItemStack>],
    after: &[Option<ItemStack>],
) -> Vec<GameEvent> {
    before
        .iter()
        .zip(after)
        .enumerate()
        .filter_map(|(slot, (before, after))| {
            (before != after).then(|| GameEvent::InventorySlotChanged {
                uuid,
                slot,
                stack: after.clone(),
            })
        })
        .collect()
}

fn normalize_player_name(name: &str) -> String {
    name.to_ascii_lowercase()
}

fn squared_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| (left - right).powi(2))
        .sum()
}

#[derive(Debug, Error)]
pub enum GameStateError {
    #[error(transparent)]
    Entity(#[from] EntityError),
    #[error(transparent)]
    Player(#[from] PlayerError),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Container(#[from] ContainerError),
    #[error(transparent)]
    Attribute(#[from] AttributeError),
    #[error(transparent)]
    StatusEffect(#[from] StatusEffectError),
    #[error(transparent)]
    Combat(#[from] CombatError),
    #[error("invalid game dimension {dimension}")]
    InvalidDimension { dimension: String },
    #[error("player name {name} is already used by another UUID")]
    DuplicatePlayerName { name: String },
    #[error("player {uuid:?} is already connected")]
    PlayerAlreadyConnected { uuid: PlayerUuid },
    #[error("unknown player {uuid:?}")]
    UnknownPlayer { uuid: PlayerUuid },
    #[error("player {uuid:?} is not connected")]
    PlayerNotConnected { uuid: PlayerUuid },
    #[error("connected player {uuid:?} has no entity")]
    PlayerMissingEntity { uuid: PlayerUuid },
    #[error("player entities must be managed through the player connection lifecycle")]
    PlayerEntityRequiresConnection,
    #[error("entity {entity_id:?} is not a living entity")]
    NotLivingEntity { entity_id: EntityId },
    #[error("entity {entity_id:?} is not a valid player attack target")]
    InvalidAttackTarget { entity_id: EntityId },
    #[error("player {uuid:?} is dead and must respawn before healing")]
    PlayerDead { uuid: PlayerUuid },
    #[error("player {uuid:?} is alive and cannot respawn")]
    PlayerAlive { uuid: PlayerUuid },
    #[error("pickup radius {radius} must be finite and between 0 and 16")]
    InvalidPickupRadius { radius: f64 },
    #[error("item entity {entity_id:?} disappeared during pickup")]
    MissingItemEntity { entity_id: EntityId },
    #[error("entity {entity_id:?} is not an item entity")]
    NotItemEntity { entity_id: EntityId },
    #[error("experience orb {entity_id:?} disappeared during pickup")]
    MissingExperienceOrb { entity_id: EntityId },
    #[error("player {uuid:?} has no item equipped in {slot:?}")]
    MissingEquippedItem {
        uuid: PlayerUuid,
        slot: EquipmentSlot,
    },
    #[error("player {uuid:?} has {actual} equipped in {slot:?}, but {expected} was required")]
    UnexpectedEquippedItem {
        uuid: PlayerUuid,
        slot: EquipmentSlot,
        expected: String,
        actual: String,
    },
    #[error("unknown player attribute {attribute}")]
    UnknownAttribute { attribute: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn connects_moves_disconnects_and_reconnects_players() {
        let uuid = PlayerUuid::new(10);
        let mut state = GameState::default();
        let events = state.connect_player(uuid, "Steve", spawn()).unwrap();
        assert!(matches!(events[0], GameEvent::PlayerConnected { .. }));
        assert_eq!(state.online_player_count(), 1);

        let moved = Transform::new([10.0, 70.0, -4.0], 90.0, 0.0, true).unwrap();
        state.move_player(uuid, moved).unwrap();
        let entity_id = state.player(uuid).unwrap().entity_id.unwrap();
        assert_eq!(state.entities().get(entity_id).unwrap().transform, moved);

        state.disconnect_player(uuid).unwrap();
        assert_eq!(state.online_player_count(), 0);
        assert!(state.entities().is_empty());
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        assert_eq!(state.online_player_count(), 1);
    }

    #[test]
    fn names_are_unique_without_case_sensitivity() {
        let mut state = GameState::default();
        state
            .connect_player(PlayerUuid::new(1), "Steve", spawn())
            .unwrap();
        assert!(matches!(
            state.connect_player(PlayerUuid::new(2), "steve", spawn()),
            Err(GameStateError::DuplicatePlayerName { .. })
        ));
    }

    #[test]
    fn gives_items_changes_mode_and_advances_time() {
        let uuid = PlayerUuid::new(7);
        let mut state = GameState::default();
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        let (remainder, events) = state
            .give_item(uuid, ItemStack::new("minecraft:stone", 64).unwrap())
            .unwrap();
        assert_eq!(remainder, None);
        assert!(matches!(
            events[0],
            GameEvent::InventoryChanged { inserted: 64, .. }
        ));
        assert!(matches!(
            &events[1],
            GameEvent::InventorySlotChanged {
                slot: 9,
                stack: Some(stack),
                ..
            } if stack.item() == "minecraft:stone" && stack.count() == 64
        ));
        state.set_game_mode(uuid, GameMode::Creative).unwrap();
        assert_eq!(state.player(uuid).unwrap().game_mode, GameMode::Creative);
        state.tick();
        assert_eq!(state.time().game_time, 1);
        assert_eq!(state.time().day_time, 1);
    }

    #[test]
    fn selected_hotbar_is_authoritative_and_validated() {
        let uuid = PlayerUuid::new(8);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let events = state.select_hotbar(uuid, 5).unwrap();
        assert_eq!(state.player(uuid).unwrap().inventory.selected_hotbar(), 5);
        assert!(matches!(
            events.as_slice(),
            [GameEvent::SelectedHotbarChanged {
                uuid: event_uuid,
                previous: 0,
                current: 5,
            }] if *event_uuid == uuid
        ));
        assert!(state.select_hotbar(uuid, 9).is_err());
    }

    #[test]
    fn consumes_the_expected_equipped_stack_and_publishes_slot_state() {
        let uuid = PlayerUuid::new(9);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state
            .player_mut(uuid)
            .unwrap()
            .inventory
            .set_slot(
                crate::HOTBAR_START,
                Some(ItemStack::new("minecraft:stone", 2).unwrap()),
            )
            .unwrap();

        let events = state
            .consume_equipped_item(uuid, EquipmentSlot::MainHand, "minecraft:stone", 1)
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::InventorySlotChanged {
                    slot,
                    stack: Some(stack),
                    ..
                },
                GameEvent::ContainerContentChanged { .. }
            ] if *slot == crate::HOTBAR_START && stack.count() == 1
        ));

        let error = state
            .consume_equipped_item(uuid, EquipmentSlot::MainHand, "minecraft:dirt", 1)
            .unwrap_err();
        assert!(matches!(
            error,
            GameStateError::UnexpectedEquippedItem { .. }
        ));
        assert_eq!(
            state
                .player(uuid)
                .unwrap()
                .inventory
                .selected_stack()
                .unwrap()
                .count(),
            1
        );

        state
            .consume_equipped_item(uuid, EquipmentSlot::MainHand, "minecraft:stone", 1)
            .unwrap();
        assert!(
            state
                .player(uuid)
                .unwrap()
                .inventory
                .selected_stack()
                .is_none()
        );
    }

    #[test]
    fn damages_and_breaks_the_expected_equipped_tool() {
        let uuid = PlayerUuid::new(10);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state
            .player_mut(uuid)
            .unwrap()
            .inventory
            .set_slot(
                crate::HOTBAR_START,
                Some(ItemStack::with_max_count("minecraft:wooden_pickaxe", 1, 1).unwrap()),
            )
            .unwrap();

        state
            .damage_equipped_item(
                uuid,
                EquipmentSlot::MainHand,
                "minecraft:wooden_pickaxe",
                58,
                59,
            )
            .unwrap();
        assert_eq!(
            state
                .player(uuid)
                .unwrap()
                .inventory
                .selected_stack()
                .unwrap()
                .damage()
                .unwrap(),
            58
        );
        state
            .damage_equipped_item(
                uuid,
                EquipmentSlot::MainHand,
                "minecraft:wooden_pickaxe",
                1,
                59,
            )
            .unwrap();
        assert!(
            state
                .player(uuid)
                .unwrap()
                .inventory
                .selected_stack()
                .is_none()
        );
    }

    #[test]
    fn drops_one_or_all_equipped_items_as_authoritative_entities() {
        let uuid = PlayerUuid::new(11);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state
            .player_mut(uuid)
            .unwrap()
            .inventory
            .set_slot(
                crate::HOTBAR_START,
                Some(ItemStack::new("minecraft:cobblestone", 3).unwrap()),
            )
            .unwrap();

        let events = state
            .drop_equipped_item(uuid, EquipmentSlot::MainHand, false)
            .unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::EntitySpawned { entity }
                if entity.item().is_some_and(|item| item.stack.count() == 1)
                    && entity.transform.position[0] == 0.5
                    && (entity.transform.position[1] - 66.3).abs() < 1.0e-10
                    && entity.transform.position[2] == 0.5
                    && entity.velocity.0[2] > 0.0
        )));
        assert_eq!(
            state
                .player(uuid)
                .unwrap()
                .inventory
                .selected_stack()
                .unwrap()
                .count(),
            2
        );

        state
            .drop_equipped_item(uuid, EquipmentSlot::MainHand, true)
            .unwrap();
        assert!(
            state
                .player(uuid)
                .unwrap()
                .inventory
                .selected_stack()
                .is_none()
        );
        assert_eq!(
            state
                .entities()
                .iter()
                .filter(|(_, entity)| entity.item().is_some())
                .count(),
            2
        );
    }

    #[test]
    fn detaches_live_connections_for_restart() {
        let mut state = GameState::default();
        state
            .connect_player(PlayerUuid::new(20), "Steve", spawn())
            .unwrap();
        state
            .connect_player(PlayerUuid::new(21), "Alex", spawn())
            .unwrap();
        assert_eq!(state.detach_all_connections(), 2);
        assert_eq!(state.online_player_count(), 0);
        assert!(state.entities().is_empty());
        assert!(
            state
                .players()
                .values()
                .all(|player| player.entity_id.is_none())
        );
    }

    #[test]
    fn daylight_cycle_rule_controls_day_time() {
        let mut state = GameState::default();
        state.set_game_rule("doDaylightCycle", GameRuleValue::Boolean(false));
        state.tick();
        assert_eq!(state.time().game_time, 1);
        assert_eq!(state.time().day_time, 0);
    }

    #[test]
    fn damage_heal_and_death_publish_authoritative_vitals() {
        let uuid = PlayerUuid::new(30);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state.player_mut(uuid).unwrap().vitals.absorption = 2.0;

        let damaged = state.damage_player(uuid, 5.0).unwrap();
        assert!(matches!(
            damaged.as_slice(),
            [
                GameEvent::PlayerDamaged {
                    amount,
                    previous,
                    current,
                    ..
                },
                GameEvent::PlayerVitalsChanged { vitals, .. }
            ] if *amount == 5.0
                && previous.health == 20.0
                && previous.absorption == 2.0
                && current.health == 17.0
                && current.absorption == 0.0
                && *vitals == *current
        ));

        let healed = state.heal_player(uuid, 2.0).unwrap();
        assert!(matches!(
            healed.as_slice(),
            [GameEvent::PlayerVitalsChanged { vitals, .. }] if vitals.health == 19.0
        ));

        let fatal = state.damage_player(uuid, 100.0).unwrap();
        assert!(matches!(fatal[0], GameEvent::PlayerDamaged { .. }));
        assert!(matches!(
            fatal[1],
            GameEvent::PlayerVitalsChanged { vitals, .. } if vitals.health == 0.0
        ));
        assert!(matches!(fatal[2], GameEvent::PlayerKilled { .. }));
        assert!(matches!(
            state.heal_player(uuid, 1.0),
            Err(GameStateError::PlayerDead { .. })
        ));
    }

    #[test]
    fn invulnerable_players_ignore_damage() {
        let uuid = PlayerUuid::new(31);
        let mut state = GameState::default();
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        state.set_game_mode(uuid, GameMode::Creative).unwrap();
        assert!(state.damage_player(uuid, 20.0).unwrap().is_empty());
        assert_eq!(state.player(uuid).unwrap().vitals.health, 20.0);
    }

    #[test]
    fn respawn_resets_vitals_transform_and_velocity_without_reallocating_entity() {
        let uuid = PlayerUuid::new(32);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state.set_game_mode(uuid, GameMode::Creative).unwrap();
        let entity_id = state.player(uuid).unwrap().entity_id.unwrap();
        state.entities_mut().get_mut(entity_id).unwrap().velocity =
            crate::Velocity([1.0, 2.0, 3.0]);
        state.kill_player(uuid).unwrap();
        let respawn = Transform::new([8.5, 70.0, -3.5], 45.0, 0.0, false).unwrap();
        let events = state.respawn_player(uuid, respawn).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::PlayerRespawned {
                    entity_id: event_entity_id,
                    transform,
                    ..
                },
                GameEvent::PlayerVitalsChanged { vitals, .. }
            ] if *event_entity_id == entity_id
                && *transform == respawn
                && *vitals == Vitals::default()
        ));
        let entity = state.entities().get(entity_id).unwrap();
        assert_eq!(entity.transform, respawn);
        assert_eq!(entity.velocity, crate::Velocity::default());
        assert_eq!(state.player(uuid).unwrap().vitals, Vitals::default());
        assert!(matches!(
            events[0],
            GameEvent::PlayerRespawned {
                previous_game_mode: Some(GameMode::Survival),
                ..
            }
        ));
        assert!(matches!(
            state.respawn_player(uuid, respawn),
            Err(GameStateError::PlayerAlive { .. })
        ));
    }

    #[test]
    fn item_entities_age_and_are_picked_up_authoritatively() {
        let uuid = PlayerUuid::new(40);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let events = state
            .spawn_item_entity(
                spawn(),
                ItemStack::new("minecraft:cobblestone", 12).unwrap(),
                Velocity::default(),
                None,
            )
            .unwrap();
        let item_id = match &events[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        state
            .entities_mut()
            .get_mut(item_id)
            .unwrap()
            .item_mut()
            .unwrap()
            .pickup_delay_ticks = 0;
        let picked_up = state.pickup_nearby_items(uuid, 2.0).unwrap();
        assert!(picked_up.iter().any(|event| matches!(
            event,
            GameEvent::ItemPickedUp {
                entity_id,
                inserted: 12,
                ..
            } if *entity_id == item_id
        )));
        assert!(state.entities().get(item_id).is_none());
        assert_eq!(
            state
                .player(uuid)
                .unwrap()
                .inventory
                .slot(9)
                .unwrap()
                .unwrap()
                .count(),
            12
        );
    }

    #[test]
    fn experience_orbs_are_picked_up_with_vanilla_delay_and_level_progress() {
        let uuid = PlayerUuid::new(41);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let spawn_orb = |state: &mut GameState, value| {
            let events = state
                .spawn_entity(
                    EntityType::new("minecraft:experience_orb").unwrap(),
                    spawn(),
                    Velocity::default(),
                    EntityPayload::ExperienceOrb { value },
                )
                .unwrap();
            match &events[0] {
                GameEvent::EntitySpawned { entity } => entity.id,
                event => panic!("unexpected event: {event:?}"),
            }
        };

        let first_orb = spawn_orb(&mut state, 10);
        let second_orb = spawn_orb(&mut state, 5);
        let picked_up = state.pickup_nearby_experience_orbs(uuid, 2.0).unwrap();
        assert!(matches!(
            picked_up.as_slice(),
            [
                GameEvent::ExperienceOrbPickedUp {
                    entity_id,
                    value: 10,
                    ..
                },
                GameEvent::PlayerExperienceChanged { experience, .. },
                GameEvent::EntityRemoved {
                    entity_id: removed_id
                }
            ] if *entity_id == first_orb
                && *removed_id == first_orb
                && experience.level == 1
                && experience.total == 10
                && (experience.progress - (3.0 / 9.0)).abs() < f32::EPSILON
        ));
        assert!(state.entities().get(first_orb).is_none());
        assert!(state.entities().get(second_orb).is_some());
        assert!(
            state
                .pickup_nearby_experience_orbs(uuid, 2.0)
                .unwrap()
                .is_empty()
        );

        state.tick();
        assert!(
            state
                .pickup_nearby_experience_orbs(uuid, 2.0)
                .unwrap()
                .is_empty()
        );
        state.tick();
        state.pickup_nearby_experience_orbs(uuid, 2.0).unwrap();
        assert_eq!(state.player(uuid).unwrap().experience.total, 15);
        assert!(state.entities().get(second_orb).is_none());
    }

    #[test]
    fn experience_orbs_follow_and_reach_stationary_players_during_ticks() {
        let uuid = PlayerUuid::new(43);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let orb_transform = Transform::new([4.5, 65.0, 0.5], 0.0, 0.0, false).unwrap();
        let spawned = state
            .spawn_entity(
                EntityType::new("minecraft:experience_orb").unwrap(),
                orb_transform,
                Velocity::default(),
                EntityPayload::ExperienceOrb { value: 7 },
            )
            .unwrap();
        let orb_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };

        let events = state.tick();
        let orb = state.entities().get(orb_id).unwrap();
        assert!(orb.transform.position[0] < orb_transform.position[0]);
        assert!(orb.velocity.0[0] < 0.0);
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::EntityMoved { entity_id, .. } if *entity_id == orb_id
        )));

        state
            .entities_mut()
            .set_transform(
                orb_id,
                Transform::new([0.6, 65.0, 0.5], 0.0, 0.0, false).unwrap(),
            )
            .unwrap();
        let events = state.tick();
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::ExperienceOrbPickedUp {
                uuid: picked_up_by,
                entity_id,
                value: 7,
            } if *picked_up_by == uuid && *entity_id == orb_id
        )));
        assert_eq!(state.player(uuid).unwrap().experience.total, 7);
        assert!(state.entities().get(orb_id).is_none());
    }

    #[test]
    fn partial_item_pickup_updates_the_authoritative_entity_after_the_pickup_event() {
        let uuid = PlayerUuid::new(42);
        let mut state = GameState::default();
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        let player = state.player_mut(uuid).unwrap();
        for slot in crate::MAIN_INVENTORY_START..=crate::HOTBAR_END {
            player
                .inventory
                .set_slot(slot, Some(ItemStack::new("minecraft:dirt", 64).unwrap()))
                .unwrap();
        }
        player
            .inventory
            .set_slot(
                crate::MAIN_INVENTORY_START,
                Some(ItemStack::new("minecraft:stone", 63).unwrap()),
            )
            .unwrap();

        let spawned = state
            .spawn_item_entity(
                spawn(),
                ItemStack::new("minecraft:stone", 3).unwrap(),
                Velocity::default(),
                None,
            )
            .unwrap();
        let item_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        state
            .entities_mut()
            .get_mut(item_id)
            .unwrap()
            .item_mut()
            .unwrap()
            .pickup_delay_ticks = 0;

        let events = state.pickup_nearby_items(uuid, 2.0).unwrap();
        let pickup_index = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    GameEvent::ItemPickedUp {
                        entity_id,
                        inserted: 1,
                        ..
                    } if *entity_id == item_id
                )
            })
            .expect("partial pickup event");
        let changed_index = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    GameEvent::ItemEntityChanged {
                        entity_id,
                        stack,
                    } if *entity_id == item_id && stack.count() == 2
                )
            })
            .expect("item stack change event");
        assert!(pickup_index < changed_index);
        assert_eq!(
            state
                .entities()
                .get(item_id)
                .unwrap()
                .item()
                .unwrap()
                .stack
                .count(),
            2
        );
    }

    #[test]
    fn movement_tracks_fall_distance_and_applies_landing_damage() {
        let uuid = PlayerUuid::new(41);
        let mut state = GameState::default();
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        state
            .move_player(
                uuid,
                Transform::new([0.5, 75.0, 0.5], 0.0, 0.0, false).unwrap(),
            )
            .unwrap();
        state
            .move_player(
                uuid,
                Transform::new([0.5, 70.0, 0.5], 0.0, 0.0, false).unwrap(),
            )
            .unwrap();
        let landing = state
            .move_player(
                uuid,
                Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
            )
            .unwrap();
        assert!(landing.iter().any(|event| matches!(
            event,
            GameEvent::PlayerDamaged {
                source: DamageSource {
                    kind: DamageKind::Fall,
                    ..
                },
                amount,
                ..
            } if *amount == 7.0
        )));
        assert_eq!(state.player(uuid).unwrap().vitals.health, 13.0);
        assert_eq!(state.player(uuid).unwrap().fall_distance, 0.0);
    }

    #[test]
    fn attributes_effects_and_knockback_are_authoritative() {
        let uuid = PlayerUuid::new(42);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state
            .set_player_attribute_base(uuid, "minecraft:armor", 20.0)
            .unwrap();
        let damaged = state
            .damage_player_with_source(uuid, 10.0, DamageSource::generic(DamageKind::PlayerAttack))
            .unwrap();
        assert!(matches!(
            damaged[0],
            GameEvent::PlayerDamaged { amount, .. } if amount < 10.0
        ));
        let effect = StatusEffectInstance::new(
            crate::StatusEffectId::new("minecraft:resistance").unwrap(),
            0,
            1,
        )
        .unwrap();
        state.add_status_effect(uuid, effect).unwrap();
        assert!(state.tick().iter().any(|event| matches!(
            event,
            GameEvent::PlayerStatusEffectChanged {
                active: false,
                effect,
                ..
            } if effect == "minecraft:resistance"
        )));
        let knockback = state.apply_knockback(uuid, [1.0, 0.0], 0.4).unwrap();
        assert!(matches!(
            knockback[0],
            GameEvent::PlayerVelocityChanged { velocity, .. } if velocity.0[0] > 0.0
        ));
    }

    #[test]
    fn ticking_moving_entities_emits_authoritative_motion() {
        let mut state = GameState::default();
        let spawned = state
            .spawn_item_entity(
                Transform::new([0.5, 70.0, 0.5], 0.0, 0.0, false).unwrap(),
                ItemStack::new("minecraft:stone", 1).unwrap(),
                Velocity::new([0.1, 0.0, 0.0]).unwrap(),
                None,
            )
            .unwrap();
        let entity_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        assert!(state.tick().iter().any(|event| matches!(
            event,
            GameEvent::EntityMoved {
                entity_id: moved,
                transform,
                velocity,
            } if *moved == entity_id
                && transform.position[0] > 0.5
                && transform.position[1] < 70.0
                && velocity.0[1] < 0.0
        )));
    }

    #[test]
    fn manages_non_player_entity_lifecycle_without_bypassing_player_state() {
        let mut state = GameState::default();
        let spawned = state
            .spawn_entity(
                EntityType::new("minecraft:zombie").unwrap(),
                spawn(),
                Velocity::default(),
                EntityPayload::Living(crate::LivingEntityData::new(20.0).unwrap()),
            )
            .unwrap();
        let entity_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };

        let moved = Transform::new([4.5, 65.0, -2.5], 90.0, 0.0, true).unwrap();
        assert!(matches!(
            state.move_entity(entity_id, moved).unwrap().as_slice(),
            [GameEvent::EntityMoved {
                entity_id: moved_id,
                transform,
                ..
            }] if *moved_id == entity_id && *transform == moved
        ));
        let velocity = Velocity::new([0.2, 0.1, -0.2]).unwrap();
        assert!(matches!(
            state
                .set_entity_velocity(entity_id, velocity)
                .unwrap()
                .as_slice(),
            [GameEvent::EntityMoved {
                entity_id: moved_id,
                velocity: moved_velocity,
                ..
            }] if *moved_id == entity_id && *moved_velocity == velocity
        ));
        assert_eq!(
            state.despawn_entity(entity_id).unwrap(),
            [GameEvent::EntityRemoved { entity_id }]
        );
        assert!(state.entities().get(entity_id).is_none());

        assert!(matches!(
            state.spawn_entity(
                EntityType::new("minecraft:player").unwrap(),
                spawn(),
                Velocity::default(),
                EntityPayload::Generic,
            ),
            Err(GameStateError::PlayerEntityRequiresConnection)
        ));
    }

    #[test]
    fn damages_knocks_back_kills_and_drops_living_entities() {
        let mut state = GameState::default();
        let living = crate::LivingEntityData::new(20.0)
            .unwrap()
            .with_drops(vec![ItemStack::new("minecraft:rotten_flesh", 2).unwrap()])
            .unwrap();
        let spawned = state
            .spawn_entity(
                EntityType::new("minecraft:zombie").unwrap(),
                spawn(),
                Velocity::default(),
                EntityPayload::Living(living),
            )
            .unwrap();
        let entity_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };

        let damaged = state
            .damage_entity_with_source(
                entity_id,
                4.0,
                DamageSource::generic(DamageKind::PlayerAttack),
            )
            .unwrap();
        assert!(matches!(
            damaged.as_slice(),
            [GameEvent::LivingEntityDamaged {
                entity_id: damaged_id,
                amount,
                previous_health,
                current_health,
                ..
            }] if *damaged_id == entity_id
                && *amount == 4.0
                && *previous_health == 20.0
                && *current_health == 16.0
        ));
        assert!(matches!(
            state
                .apply_entity_knockback(entity_id, [1.0, 0.0], 0.4)
                .unwrap()
                .as_slice(),
            [GameEvent::EntityMoved { velocity, .. }] if velocity.0[0] > 0.0
        ));

        let killed = state.damage_entity(entity_id, 100.0).unwrap();
        assert!(matches!(
            killed.as_slice(),
            [
                GameEvent::LivingEntityDamaged { .. },
                GameEvent::LivingEntityKilled {
                    entity_id: killed_id,
                    dropped_stacks: 1,
                    ..
                },
                GameEvent::EntityRemoved {
                    entity_id: removed_id
                },
                GameEvent::EntitySpawned { entity: dropped },
            ] if *killed_id == entity_id
                && *removed_id == entity_id
                && dropped.item().is_some_and(|item| {
                    item.stack.item() == "minecraft:rotten_flesh" && item.stack.count() == 2
                })
        ));
        assert!(state.entities().get(entity_id).is_none());
    }

    #[test]
    fn player_attacks_enforce_reach_and_drive_living_entity_death() {
        let uuid = PlayerUuid::new(49);
        let mut state = GameState::default();
        let connected = state.connect_player(uuid, "Steve", spawn()).unwrap();
        let attacker_entity_id = match connected.as_slice() {
            [GameEvent::PlayerConnected { entity_id, .. }, ..] => *entity_id,
            events => panic!("unexpected connect events: {events:?}"),
        };
        let spawn_target = |state: &mut GameState, x| {
            let living = LivingEntityData::new(2.0)
                .unwrap()
                .with_drops(vec![ItemStack::new("minecraft:rotten_flesh", 1).unwrap()])
                .unwrap();
            let events = state
                .spawn_entity(
                    EntityType::new("minecraft:zombie").unwrap(),
                    Transform::new([x, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
                    Velocity::default(),
                    EntityPayload::Living(living),
                )
                .unwrap();
            match &events[0] {
                GameEvent::EntitySpawned { entity } => entity.id,
                event => panic!("unexpected spawn event: {event:?}"),
            }
        };

        let distant = spawn_target(&mut state, 10.5);
        assert!(state.attack_entity(uuid, distant).unwrap().is_empty());
        assert_eq!(
            state
                .entities()
                .get(distant)
                .unwrap()
                .living()
                .unwrap()
                .health,
            2.0
        );

        let target = spawn_target(&mut state, 2.5);
        let first = state.attack_entity(uuid, target).unwrap();
        assert!(first.iter().any(|event| matches!(
            event,
            GameEvent::LivingEntityDamaged {
                entity_id,
                source: DamageSource {
                    kind: DamageKind::PlayerAttack,
                    attacker: Some(attacker),
                    direct_entity: Some(direct),
                    ..
                },
                current_health: 1.0,
                ..
            } if *entity_id == target
                && *attacker == attacker_entity_id
                && *direct == attacker_entity_id
        )));
        assert!(first.iter().any(|event| matches!(
            event,
            GameEvent::EntityMoved { entity_id, velocity, .. }
                if *entity_id == target && velocity.0[0] > 0.0
        )));

        let fatal = state.attack_entity(uuid, target).unwrap();
        assert!(fatal.iter().any(|event| matches!(
            event,
            GameEvent::LivingEntityKilled { entity_id, .. } if *entity_id == target
        )));
        assert!(fatal.iter().any(|event| matches!(
            event,
            GameEvent::EntitySpawned { entity }
                if entity.item().is_some_and(|item| {
                    item.stack.item() == "minecraft:rotten_flesh" && item.stack.count() == 1
                })
        )));
        assert!(state.entities().get(target).is_none());
    }

    #[test]
    fn hostile_spawn_conditions_respect_players_gamerule_and_difficulty() {
        let uuid = PlayerUuid::new(50);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let too_close = Transform::new([10.5, 65.0, 0.5], 0.0, 0.0, true).unwrap();
        assert!(
            state
                .spawn_hostile_mob(
                    EntityType::new("minecraft:zombie").unwrap(),
                    too_close,
                    20.0,
                    Vec::new(),
                )
                .unwrap()
                .is_empty()
        );
        let valid = Transform::new([30.5, 65.0, 0.5], 0.0, 0.0, true).unwrap();
        assert!(matches!(
            state
                .spawn_hostile_mob(
                    EntityType::new("minecraft:zombie").unwrap(),
                    valid,
                    20.0,
                    Vec::new(),
                )
                .unwrap()
                .as_slice(),
            [GameEvent::EntitySpawned { entity }] if entity.living().unwrap().ai.is_some()
        ));
        state.set_game_rule("doMobSpawning", GameRuleValue::Boolean(false));
        assert!(!state.can_spawn_hostile_mob(valid));
        state.set_game_rule("doMobSpawning", GameRuleValue::Boolean(true));
        state.set_difficulty(Difficulty::Peaceful);
        assert!(!state.can_spawn_hostile_mob(valid));
    }

    #[test]
    fn hostile_ai_selects_pursues_and_attacks_survival_players() {
        let uuid = PlayerUuid::new(51);
        let mut state = GameState::default();
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        let living = LivingEntityData::new(20.0)
            .unwrap()
            .with_ai(crate::MobAi::new(32.0, 0.1, 1.5, 4.0, 20).unwrap())
            .unwrap();
        let spawned = state
            .spawn_entity(
                EntityType::new("minecraft:zombie").unwrap(),
                Transform::new([4.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
                Velocity::default(),
                EntityPayload::Living(living),
            )
            .unwrap();
        let entity_id = match &spawned[0] {
            GameEvent::EntitySpawned { entity } => entity.id,
            event => panic!("unexpected event: {event:?}"),
        };
        let movement = state.tick();
        assert!(movement.iter().any(|event| matches!(
            event,
            GameEvent::EntityMoved {
                entity_id: moved,
                transform,
                ..
            } if *moved == entity_id && transform.position[0] < 4.5
        )));
        assert_eq!(
            state
                .entities()
                .get(entity_id)
                .unwrap()
                .living()
                .unwrap()
                .ai
                .as_ref()
                .unwrap()
                .target,
            Some(uuid)
        );

        state
            .move_entity(
                entity_id,
                Transform::new([2.0, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
            )
            .unwrap();
        let attack = state.tick();
        assert!(attack.iter().any(|event| matches!(
            event,
            GameEvent::PlayerDamaged {
                uuid: damaged,
                source: DamageSource {
                    kind: DamageKind::MobAttack,
                    attacker: Some(attacker),
                    ..
                },
                ..
            } if *damaged == uuid && *attacker == entity_id
        )));
        assert_eq!(state.player(uuid).unwrap().vitals.health, 16.0);
        state.tick();
        assert_eq!(state.player(uuid).unwrap().vitals.health, 16.0);
    }
}
