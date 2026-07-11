use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Difficulty, EntityError, EntityId, EntityStore, EntityType, EntityUuid, GameMode,
    InventoryError, ItemStack, PlayerError, PlayerState, PlayerUuid, Transform,
};

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
    PlayerKilled {
        uuid: PlayerUuid,
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
        let entity_id = player.entity_id;
        if let Some(id) = entity_id {
            self.entities.despawn(id);
        }
        player.disconnect();
        Ok(vec![GameEvent::PlayerDisconnected { uuid, entity_id }])
    }

    pub fn move_player(
        &mut self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        self.entities.set_transform(entity_id, transform)?;
        Ok(vec![GameEvent::PlayerMoved {
            uuid,
            entity_id,
            transform,
        }])
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
        let remainder = player.inventory.insert(stack);
        let inserted = requested - remainder.as_ref().map_or(0, ItemStack::count);
        let events = if inserted == 0 {
            Vec::new()
        } else {
            vec![GameEvent::InventoryChanged {
                uuid,
                inserted,
                item,
            }]
        };
        Ok((remainder, events))
    }

    pub fn kill_player(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        player.vitals.health = 0.0;
        Ok(vec![GameEvent::PlayerKilled { uuid }])
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

    pub fn tick(&mut self) {
        self.time.game_time = self.time.game_time.saturating_add(1);
        if self.time.daylight_cycle {
            self.time.day_time = self.time.day_time.saturating_add(1);
        }
        self.entities.tick();
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

fn normalize_player_name(name: &str) -> String {
    name.to_ascii_lowercase()
}

#[derive(Debug, Error)]
pub enum GameStateError {
    #[error(transparent)]
    Entity(#[from] EntityError),
    #[error(transparent)]
    Player(#[from] PlayerError),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
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
        state.set_game_mode(uuid, GameMode::Creative).unwrap();
        assert_eq!(state.player(uuid).unwrap().game_mode, GameMode::Creative);
        state.tick();
        assert_eq!(state.time().game_time, 1);
        assert_eq!(state.time().day_time, 1);
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
}
