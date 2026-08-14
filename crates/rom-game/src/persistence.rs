use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Difficulty, Entity, EntityError, EntityStore, GAME_SNAPSHOT_SCHEMA_VERSION, GameRuleValue,
    GameState, GameStateError, GameTime, PlayerError, PlayerState, PlayerUuid,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSnapshot {
    pub schema_version: u32,
    pub dimension: String,
    pub time: GameTime,
    pub difficulty: Difficulty,
    pub game_rules: BTreeMap<String, GameRuleValue>,
    pub players: Vec<PlayerState>,
    pub entities: Vec<Entity>,
}

impl GameSnapshot {
    #[must_use]
    pub fn capture(state: &GameState) -> Self {
        Self {
            schema_version: GAME_SNAPSHOT_SCHEMA_VERSION,
            dimension: state.dimension.clone(),
            time: state.time,
            difficulty: state.difficulty,
            game_rules: state.game_rules.clone(),
            players: state.players.values().cloned().collect(),
            entities: state
                .entities
                .iter()
                .map(|(_, entity)| entity.clone())
                .collect(),
        }
    }

    pub fn restore(self) -> Result<GameState, PersistenceError> {
        if self.schema_version != GAME_SNAPSHOT_SCHEMA_VERSION {
            return Err(PersistenceError::UnsupportedSchema {
                actual: self.schema_version,
                expected: GAME_SNAPSHOT_SCHEMA_VERSION,
            });
        }
        let mut state = GameState::new(self.dimension)?;
        state.time = self.time;
        state.difficulty = self.difficulty;
        state.game_rules = self.game_rules;
        state.entities = EntityStore::new();
        for entity in self.entities {
            state.entities.insert_restored(entity)?;
        }

        let mut entity_bindings = BTreeSet::new();
        for player in self.players {
            crate::player::validate_username(&player.name)?;
            if !crate::validate_resource_location(&player.dimension) {
                return Err(PersistenceError::InvalidPlayerDimension {
                    player: player.name,
                    dimension: player.dimension,
                });
            }
            if state.players.contains_key(&player.uuid) {
                return Err(PersistenceError::DuplicatePlayerUuid { uuid: player.uuid });
            }
            let normalized = player.name.to_ascii_lowercase();
            if state.player_names.contains_key(&normalized) {
                return Err(PersistenceError::DuplicatePlayerName { name: player.name });
            }
            match (player.connected, player.entity_id) {
                (true, Some(entity_id)) => {
                    let entity = state.entities.get(entity_id).ok_or(
                        PersistenceError::MissingPlayerEntity {
                            uuid: player.uuid,
                            entity_id,
                        },
                    )?;
                    if entity.uuid.get() != player.uuid.get() || !entity.is_player() {
                        return Err(PersistenceError::MismatchedPlayerEntity {
                            uuid: player.uuid,
                            entity_id,
                        });
                    }
                    if !entity_bindings.insert(entity_id) {
                        return Err(PersistenceError::DuplicatePlayerEntityBinding { entity_id });
                    }
                }
                (true, None) => {
                    return Err(PersistenceError::ConnectedPlayerWithoutEntity {
                        uuid: player.uuid,
                    });
                }
                (false, Some(entity_id)) => {
                    return Err(PersistenceError::DisconnectedPlayerWithEntity {
                        uuid: player.uuid,
                        entity_id,
                    });
                }
                (false, None) => {}
            }
            state.player_names.insert(normalized, player.uuid);
            state.players.insert(player.uuid, player);
        }
        Ok(state)
    }

    pub fn to_json_pretty(&self) -> Result<String, PersistenceError> {
        serde_json::to_string_pretty(self).map_err(PersistenceError::Serialize)
    }

    pub fn from_json(input: &str) -> Result<Self, PersistenceError> {
        serde_json::from_str(input).map_err(PersistenceError::Deserialize)
    }
}

impl GameState {
    #[must_use]
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot::capture(self)
    }

    pub fn restore(snapshot: GameSnapshot) -> Result<Self, PersistenceError> {
        snapshot.restore()
    }
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("snapshot schema {actual} is unsupported; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error(transparent)]
    State(#[from] GameStateError),
    #[error(transparent)]
    Player(#[from] PlayerError),
    #[error(transparent)]
    Entity(#[from] EntityError),
    #[error("cannot serialize game snapshot")]
    Serialize(#[source] serde_json::Error),
    #[error("cannot deserialize game snapshot")]
    Deserialize(#[source] serde_json::Error),
    #[error("duplicate player UUID {uuid:?} in snapshot")]
    DuplicatePlayerUuid { uuid: PlayerUuid },
    #[error("duplicate player name {name} in snapshot")]
    DuplicatePlayerName { name: String },
    #[error("player {player} has invalid dimension {dimension}")]
    InvalidPlayerDimension { player: String, dimension: String },
    #[error("connected player {uuid:?} has no entity binding")]
    ConnectedPlayerWithoutEntity { uuid: PlayerUuid },
    #[error("disconnected player {uuid:?} still references entity {entity_id:?}")]
    DisconnectedPlayerWithEntity {
        uuid: PlayerUuid,
        entity_id: crate::EntityId,
    },
    #[error("player {uuid:?} references missing entity {entity_id:?}")]
    MissingPlayerEntity {
        uuid: PlayerUuid,
        entity_id: crate::EntityId,
    },
    #[error("player {uuid:?} references non-player or mismatched entity {entity_id:?}")]
    MismatchedPlayerEntity {
        uuid: PlayerUuid,
        entity_id: crate::EntityId,
    },
    #[error("multiple players reference entity {entity_id:?}")]
    DuplicatePlayerEntityBinding { entity_id: crate::EntityId },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntityId, PlayerUuid, Transform};

    fn spawn() -> Transform {
        Transform::new([10.5, 64.0, -8.5], 45.0, 10.0, true).unwrap()
    }

    #[test]
    fn round_trips_complete_game_state() {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(0x1234);
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state.tick();
        state.set_day_time(6_000);
        let snapshot = state.snapshot();
        let json = snapshot.to_json_pretty().unwrap();
        let restored = GameSnapshot::from_json(&json).unwrap().restore().unwrap();
        assert_eq!(restored, state);
    }

    #[test]
    fn rejects_unknown_schema_versions() {
        let mut snapshot = GameState::default().snapshot();
        snapshot.schema_version += 1;
        assert!(matches!(
            snapshot.restore(),
            Err(PersistenceError::UnsupportedSchema { .. })
        ));
    }

    #[test]
    fn rejects_broken_player_entity_references() {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(1);
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        let mut snapshot = state.snapshot();
        snapshot.players[0].entity_id = Some(EntityId::new(999).unwrap());
        assert!(matches!(
            snapshot.restore(),
            Err(PersistenceError::MissingPlayerEntity { .. })
        ));
    }

    #[test]
    fn disconnected_players_round_trip_without_entities() {
        let mut state = GameState::default();
        let uuid = PlayerUuid::new(2);
        state.connect_player(uuid, "Notch", spawn()).unwrap();
        state.disconnect_player(uuid).unwrap();
        let restored = state.snapshot().restore().unwrap();
        assert_eq!(restored.player(uuid).unwrap().entity_id, None);
        assert!(restored.entities().is_empty());
    }
}
