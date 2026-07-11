use std::sync::{Arc, RwLock};

use ferrum_game::{
    CommandError, CommandOutcome, CommandSource, GameEvent, GameSnapshot, GameState,
    GameStateError, PersistenceError, PlayerUuid, Transform, execute_command,
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct SharedGameRuntime {
    state: Arc<RwLock<GameState>>,
}

impl SharedGameRuntime {
    #[must_use]
    pub fn new(state: GameState) -> Self {
        Self {
            state: Arc::new(RwLock::new(state)),
        }
    }

    #[must_use]
    pub fn vanilla_overworld() -> Self {
        Self::new(GameState::default())
    }

    pub fn connect_player(
        &self,
        uuid: PlayerUuid,
        name: impl Into<String>,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        self.write()?
            .connect_player(uuid, name, transform)
            .map_err(Into::into)
    }

    pub fn disconnect_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        self.write()?.disconnect_player(uuid).map_err(Into::into)
    }

    pub fn move_player(
        &self,
        uuid: PlayerUuid,
        transform: Transform,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        self.write()?
            .move_player(uuid, transform)
            .map_err(Into::into)
    }

    pub fn execute_command(
        &self,
        source: &CommandSource,
        input: &str,
    ) -> Result<CommandOutcome, GameRuntimeError> {
        execute_command(&mut self.write()?, source, input).map_err(Into::into)
    }

    pub fn tick(&self) -> Result<(), GameRuntimeError> {
        self.write()?.tick();
        Ok(())
    }

    pub fn snapshot(&self) -> Result<GameSnapshot, GameRuntimeError> {
        Ok(self.read()?.snapshot())
    }

    pub fn replace_from_snapshot(&self, snapshot: GameSnapshot) -> Result<(), GameRuntimeError> {
        let restored = GameState::restore(snapshot)?;
        *self.write()? = restored;
        Ok(())
    }

    pub fn with_state<T>(
        &self,
        operation: impl FnOnce(&GameState) -> T,
    ) -> Result<T, GameRuntimeError> {
        Ok(operation(&self.read()?))
    }

    pub fn with_state_mut<T>(
        &self,
        operation: impl FnOnce(&mut GameState) -> Result<T, GameRuntimeError>,
    ) -> Result<T, GameRuntimeError> {
        operation(&mut self.write()?)
    }

    fn read(&self) -> Result<std::sync::RwLockReadGuard<'_, GameState>, GameRuntimeError> {
        self.state.read().map_err(|_| GameRuntimeError::Poisoned)
    }

    fn write(&self) -> Result<std::sync::RwLockWriteGuard<'_, GameState>, GameRuntimeError> {
        self.state.write().map_err(|_| GameRuntimeError::Poisoned)
    }
}

#[derive(Debug, Error)]
pub enum GameRuntimeError {
    #[error("shared game-state lock is poisoned")]
    Poisoned,
    #[error(transparent)]
    State(#[from] GameStateError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_game::{GameMode, PlayerState};
    use std::thread;

    fn spawn() -> Transform {
        Transform::new([0.5, 65.0, 0.5], 0.0, 0.0, true).unwrap()
    }

    #[test]
    fn shares_players_and_commands_across_runtime_clones() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(1);
        runtime.connect_player(uuid, "Steve", spawn()).unwrap();
        let clone = runtime.clone();
        thread::spawn(move || {
            clone
                .execute_command(&CommandSource::console(), "/gamemode creative Steve")
                .unwrap();
        })
        .join()
        .unwrap();

        let game_mode = runtime
            .with_state(|state| {
                state
                    .player(uuid)
                    .map(|player: &PlayerState| player.game_mode)
            })
            .unwrap();
        assert_eq!(game_mode, Some(GameMode::Creative));
    }

    #[test]
    fn replaces_state_from_validated_snapshot() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let uuid = PlayerUuid::new(2);
        runtime.connect_player(uuid, "Alex", spawn()).unwrap();
        let snapshot = runtime.snapshot().unwrap();

        runtime.disconnect_player(uuid).unwrap();
        assert_eq!(
            runtime.with_state(GameState::online_player_count).unwrap(),
            0
        );
        runtime.replace_from_snapshot(snapshot).unwrap();
        assert_eq!(
            runtime.with_state(GameState::online_player_count).unwrap(),
            1
        );
    }
}
