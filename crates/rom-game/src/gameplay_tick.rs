//! Authoritative gameplay ticking and world-side inventory/experience effects.
//!
//! This module deliberately sits above the low-level entity helpers. The entity
//! modules know how a single item or experience orb behaves; this module decides
//! when those behaviours run, which players can pick them up, and which domain
//! events must be published by the server runtime.

use thiserror::Error;

use crate::{
    EntityError, EntityId, EntityUuid, ExperienceOrbError, GameEvent, GameState, ItemEntityError,
    ItemStack, PlayerUuid, Transform, Velocity, experience_orb_data, item_entity_data,
    merge_experience_orbs, merge_nearby_item_entities, spawn_experience_orb, spawn_item_entity,
    split_experience_value, tick_experience_orbs, tick_item_entities, try_pickup_experience_orb,
    try_pickup_item_entity,
};

const DROP_HORIZONTAL_SPEED: f64 = 0.08;
const DROP_VERTICAL_SPEED: f64 = 0.20;
const DROP_PICKUP_DELAY_TICKS: u16 = 10;
const EXPERIENCE_PICKUP_DELAY_TICKS: u16 = 2;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameplayTickStats {
    pub expired_items: usize,
    pub expired_experience_orbs: usize,
    pub merged_item_entities: usize,
    pub merged_experience_orbs: usize,
    pub item_pickups: usize,
    pub experience_pickups: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameplayTickOutcome {
    pub events: Vec<GameEvent>,
    pub stats: GameplayTickStats,
}

impl GameplayTickOutcome {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            events: Vec::new(),
            stats: GameplayTickStats::default(),
        }
    }
}

impl GameState {
    /// Advances the authoritative gameplay simulation by exactly one server tick.
    ///
    /// Entity age is advanced once by `EntityStore::tick`; item/orb helpers must
    /// therefore never increment age on their own.
    pub fn tick_gameplay(&mut self) -> Result<GameplayTickOutcome, GameplayTickError> {
        self.time.game_time = self.time.game_time.saturating_add(1);
        if self.time.daylight_cycle {
            self.time.day_time = self.time.day_time.saturating_add(1);
        }

        self.entities.tick();

        let mut outcome = GameplayTickOutcome::empty();
        outcome.stats.expired_items = tick_item_entities(&mut self.entities)?.len();

        let before_orb_count = experience_orb_ids(self).len();
        outcome.stats.expired_experience_orbs = tick_experience_orbs(&mut self.entities)?.len();
        outcome.stats.merged_item_entities = merge_nearby_item_entities(&mut self.entities)?.len();
        outcome.stats.merged_experience_orbs = merge_experience_orbs(&mut self.entities)?;

        debug_assert!(
            before_orb_count >= outcome.stats.expired_experience_orbs,
            "experience orb expiry count cannot exceed the pre-tick orb count"
        );

        self.collect_nearby_pickups(&mut outcome)?;
        Ok(outcome)
    }

    /// Materializes every `ItemsDropped` event as actual world item entities.
    ///
    /// The original event remains useful for audit/logging compatibility. The
    /// returned IDs identify the newly-created entities for diagnostics and tests.
    pub fn materialize_drop_events(
        &mut self,
        events: &[GameEvent],
    ) -> Result<Vec<EntityId>, GameplayTickError> {
        let mut spawned = Vec::new();
        for event in events {
            let GameEvent::ItemsDropped { uuid, stacks } = event else {
                continue;
            };
            let transform = self.player_transform(*uuid)?;
            for (index, stack) in stacks.iter().cloned().enumerate() {
                spawned.push(self.spawn_dropped_item(*uuid, index, transform, stack)?);
            }
        }
        Ok(spawned)
    }

    /// Spawns experience as vanilla-sized orb values at a world position.
    pub fn spawn_experience_value(
        &mut self,
        source: u128,
        position: [f64; 3],
        value: u32,
    ) -> Result<Vec<EntityId>, GameplayTickError> {
        let mut spawned = Vec::new();
        if value == 0 {
            return Ok(spawned);
        }
        let transform = Transform::new(position, 0.0, 0.0, false)?;
        for (index, orb_value) in split_experience_value(value).into_iter().enumerate() {
            let uuid = self.fresh_entity_uuid(mix_seed(
                source,
                self.time.game_time,
                index as u64,
                0x5850_4f52_425f_5350,
            ));
            let velocity = radial_velocity(index, 0.04, 0.10)?;
            let entity_id = spawn_experience_orb(
                &mut self.entities,
                uuid,
                transform,
                velocity,
                orb_value,
                EXPERIENCE_PICKUP_DELAY_TICKS,
            )?;
            spawned.push(entity_id);
        }
        Ok(spawned)
    }

    fn collect_nearby_pickups(
        &mut self,
        outcome: &mut GameplayTickOutcome,
    ) -> Result<(), GameplayTickError> {
        let players = self
            .players
            .iter()
            .filter_map(|(uuid, player)| {
                if !player.connected || player.vitals.is_dead() {
                    return None;
                }
                let entity_id = player.entity_id?;
                let position = self.entities.get(entity_id)?.transform.position;
                Some((*uuid, position))
            })
            .collect::<Vec<_>>();

        for (uuid, position) in players {
            self.collect_item_pickups(uuid, position, outcome)?;
            self.collect_experience_pickups(uuid, position, outcome)?;
        }
        Ok(())
    }

    fn collect_item_pickups(
        &mut self,
        uuid: PlayerUuid,
        position: [f64; 3],
        outcome: &mut GameplayTickOutcome,
    ) -> Result<(), GameplayTickError> {
        let ids = item_entity_ids(self);
        let mut changed_any = false;

        for entity_id in ids {
            if self.entities.get(entity_id).is_none() {
                continue;
            }
            let Some(before) = item_entity_data(&self.entities, entity_id)? else {
                continue;
            };
            let item_name = before.stack.item().to_owned();

            let result = {
                let (entities, players) = (&mut self.entities, &mut self.players);
                let Some(player) = players.get_mut(&uuid) else {
                    continue;
                };
                try_pickup_item_entity(entities, entity_id, position, &mut player.inventory)?
            };

            let Some(result) = result else {
                continue;
            };
            if result.inserted == 0 {
                continue;
            }

            changed_any = true;
            outcome.stats.item_pickups = outcome.stats.item_pickups.saturating_add(1);
            outcome.events.push(GameEvent::InventoryChanged {
                uuid,
                inserted: result.inserted,
                item: item_name,
            });

            if let Some(player) = self.players.get(&uuid) {
                outcome
                    .events
                    .extend(result.changed_slots.iter().copied().map(|slot| {
                        GameEvent::InventorySlotChanged {
                            uuid,
                            slot,
                            stack: player.inventory.slots()[slot].clone(),
                        }
                    }));
            }
        }

        if changed_any {
            if let Some(player) = self.players.get(&uuid) {
                outcome.events.push(GameEvent::ContainerContentChanged {
                    uuid,
                    snapshot: player.inventory_session.snapshot(&player.inventory),
                });
            }
        }
        Ok(())
    }

    fn collect_experience_pickups(
        &mut self,
        uuid: PlayerUuid,
        position: [f64; 3],
        outcome: &mut GameplayTickOutcome,
    ) -> Result<(), GameplayTickError> {
        let ids = experience_orb_ids(self);
        for entity_id in ids {
            if self.entities.get(entity_id).is_none() {
                continue;
            }
            let Some(data) = experience_orb_data(&self.entities, entity_id)? else {
                continue;
            };
            if data.value == 0 {
                continue;
            }
            let Some(value) = try_pickup_experience_orb(&mut self.entities, entity_id, position)?
            else {
                continue;
            };

            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameplayTickError::UnknownPlayer { uuid })?;
            let (total, level, progress) =
                crate::experience::add_points(player.experience.total, u64::from(value));
            player.experience.total = total;
            player.experience.level = level;
            player.experience.progress = progress;
            outcome.stats.experience_pickups = outcome.stats.experience_pickups.saturating_add(1);
        }
        Ok(())
    }

    fn spawn_dropped_item(
        &mut self,
        owner: PlayerUuid,
        index: usize,
        source_transform: Transform,
        stack: ItemStack,
    ) -> Result<EntityId, GameplayTickError> {
        let angle = deterministic_angle(owner.get(), self.time.game_time, index as u64);
        let position = [
            source_transform.position[0] + angle.cos() * 0.18,
            source_transform.position[1] + 0.35,
            source_transform.position[2] + angle.sin() * 0.18,
        ];
        let transform = Transform::new(position, 0.0, 0.0, false)?;
        let velocity = Velocity::new([
            angle.cos() * DROP_HORIZONTAL_SPEED,
            DROP_VERTICAL_SPEED,
            angle.sin() * DROP_HORIZONTAL_SPEED,
        ])?;
        let uuid = self.fresh_entity_uuid(mix_seed(
            owner.get(),
            self.time.game_time,
            index as u64,
            0x4954_454d_5f44_524f,
        ));
        Ok(spawn_item_entity(
            &mut self.entities,
            uuid,
            transform,
            velocity,
            stack,
            DROP_PICKUP_DELAY_TICKS,
        )?)
    }

    fn player_transform(&self, uuid: PlayerUuid) -> Result<Transform, GameplayTickError> {
        let player = self
            .players
            .get(&uuid)
            .ok_or(GameplayTickError::UnknownPlayer { uuid })?;
        let entity_id = player
            .entity_id
            .ok_or(GameplayTickError::PlayerMissingEntity { uuid })?;
        self.entities
            .get(entity_id)
            .map(|entity| entity.transform)
            .ok_or(GameplayTickError::PlayerMissingEntity { uuid })
    }

    fn fresh_entity_uuid(&self, mut value: u128) -> EntityUuid {
        loop {
            let uuid = EntityUuid::new(value);
            if self.entities.id_by_uuid(uuid).is_none() {
                return uuid;
            }
            value = value.wrapping_add(0x9e37_79b9_7f4a_7c15_6a09_e667_f3bc_c909);
        }
    }
}

fn item_entity_ids(state: &GameState) -> Vec<EntityId> {
    state
        .entities
        .iter()
        .filter_map(|(id, entity)| (entity.entity_type.as_str() == "minecraft:item").then_some(*id))
        .collect()
}

fn experience_orb_ids(state: &GameState) -> Vec<EntityId> {
    state
        .entities
        .iter()
        .filter_map(|(id, entity)| {
            (entity.entity_type.as_str() == "minecraft:experience_orb").then_some(*id)
        })
        .collect()
}

fn mix_seed(source: u128, game_time: u64, index: u64, domain: u64) -> u128 {
    let mut value =
        source ^ (u128::from(game_time) << 64) ^ u128::from(index) ^ (u128::from(domain) << 32);
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd_ff51_afd7_ed55_8ccd);
    value ^= value >> 29;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53_c4ce_b9fe_1a85_ec53);
    value ^ (value >> 32)
}

fn deterministic_angle(source: u128, game_time: u64, index: u64) -> f64 {
    let mixed = mix_seed(source, game_time, index, 0x4452_4f50);
    let unit = (mixed as u64) as f64 / u64::MAX as f64;
    unit * std::f64::consts::TAU
}

fn radial_velocity(index: usize, horizontal: f64, vertical: f64) -> Result<Velocity, EntityError> {
    let angle = (index as f64 * 2.399_963_229_728_653).rem_euclid(std::f64::consts::TAU);
    Velocity::new([angle.cos() * horizontal, vertical, angle.sin() * horizontal])
}

#[derive(Debug, Error)]
pub enum GameplayTickError {
    #[error(transparent)]
    Entity(#[from] EntityError),
    #[error(transparent)]
    ItemEntity(#[from] ItemEntityError),
    #[error(transparent)]
    ExperienceOrb(#[from] ExperienceOrbError),
    #[error("unknown player {uuid:?}")]
    UnknownPlayer { uuid: PlayerUuid },
    #[error("player {uuid:?} has no live entity")]
    PlayerMissingEntity { uuid: PlayerUuid },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntityType, ItemStack};

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn materialized_drop_is_a_real_item_entity() {
        let uuid = PlayerUuid::new(10);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let events = vec![GameEvent::ItemsDropped {
            uuid,
            stacks: vec![ItemStack::new("minecraft:stone", 3).unwrap()],
        }];
        let spawned = state.materialize_drop_events(&events).unwrap();
        assert_eq!(spawned.len(), 1);
        let entity_id = spawned[0];
        let entity = state.entities.get(entity_id).unwrap();
        assert_eq!(entity.entity_type.as_str(), "minecraft:item");
        assert_eq!(
            item_entity_data(&state.entities, entity_id)
                .unwrap()
                .unwrap()
                .stack
                .count(),
            3
        );
    }

    #[test]
    fn gameplay_tick_picks_up_ready_item() {
        let uuid = PlayerUuid::new(11);
        let mut state = GameState::default();
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        let item_id = spawn_item_entity(
            &mut state.entities,
            EntityUuid::new(999),
            Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
            Velocity::default(),
            ItemStack::new("minecraft:cobblestone", 4).unwrap(),
            0,
        )
        .unwrap();

        let outcome = state.tick_gameplay().unwrap();
        assert_eq!(outcome.stats.item_pickups, 1);
        assert!(state.entities.get(item_id).is_none());
        assert!(
            outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::InventoryChanged { inserted: 4, .. }))
        );
    }

    #[test]
    fn gameplay_tick_adds_experience() {
        let uuid = PlayerUuid::new(12);
        let mut state = GameState::default();
        state.connect_player(uuid, "Notch", spawn()).unwrap();
        let orb_id = spawn_experience_orb(
            &mut state.entities,
            EntityUuid::new(1000),
            Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap(),
            Velocity::default(),
            7,
            0,
        )
        .unwrap();

        let outcome = state.tick_gameplay().unwrap();
        assert_eq!(outcome.stats.experience_pickups, 1);
        assert!(state.entities.get(orb_id).is_none());
        let player = state.players.get(&uuid).unwrap();
        assert_eq!(player.experience.total, 7);
        assert_eq!(player.experience.level, 1);
    }

    #[test]
    fn entity_age_is_advanced_once_per_gameplay_tick() {
        let mut state = GameState::default();
        let id = state
            .entities
            .spawn(
                EntityUuid::new(2000),
                EntityType::new("minecraft:armor_stand").unwrap(),
                spawn(),
            )
            .unwrap();
        state.tick_gameplay().unwrap();
        assert_eq!(state.entities.get(id).unwrap().age_ticks, 1);
    }

    #[test]
    fn experience_spawns_as_multiple_orbs_and_preserves_value() {
        let mut state = GameState::default();
        let spawned = state
            .spawn_experience_value(123, [1.0, 64.0, 1.0], 3000)
            .unwrap();
        let sum = state
            .entities
            .iter()
            .filter(|(_, entity)| entity.entity_type.as_str() == "minecraft:experience_orb")
            .map(|(id, _)| {
                experience_orb_data(&state.entities, *id)
                    .unwrap()
                    .unwrap()
                    .value
            })
            .sum::<u32>();
        assert_eq!(sum, 3000);
        assert_eq!(spawned.len(), experience_orb_ids(&state).len());
    }
}
