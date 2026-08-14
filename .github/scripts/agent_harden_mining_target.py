from pathlib import Path

# PlayerGameplay: thread target token into session lifecycle.
path = Path("crates/ferrum-game/src/player_gameplay.rs")
text = path.read_text()
text = text.replace(
'''    pub fn begin_mining(
        &mut self,
        position: BlockPos,
        started_at_tick: u64,
        required_ticks: u32,
    ) -> Result<MiningSession, MiningSessionError> {
        let session = MiningSession::new(position, started_at_tick, required_ticks)?;''',
'''    pub fn begin_mining(
        &mut self,
        position: BlockPos,
        target_token: u64,
        started_at_tick: u64,
        required_ticks: u32,
    ) -> Result<MiningSession, MiningSessionError> {
        let session = MiningSession::new(position, target_token, started_at_tick, required_ticks)?;''',
1,
)
text = text.replace(
'''    pub fn finish_mining(
        &mut self,
        position: BlockPos,
        current_tick: u64,
    ) -> Result<MiningCompletion, MiningSessionError> {''',
'''    pub fn finish_mining(
        &mut self,
        position: BlockPos,
        target_token: u64,
        current_tick: u64,
    ) -> Result<MiningCompletion, MiningSessionError> {''',
1,
)
text = text.replace(
"        let completion = session.complete(position, current_tick)?;",
"        let completion = session.complete(position, target_token, current_tick)?;",
1,
)
text = text.replace(
"        gameplay.begin_mining(position, 100, 5).unwrap();\n        assert!(gameplay.finish_mining(position, 104).is_err());",
"        gameplay.begin_mining(position, 7, 100, 5).unwrap();\n        assert!(gameplay.finish_mining(position, 7, 104).is_err());",
1,
)
text = text.replace(
"        let completion = gameplay.finish_mining(position, 105).unwrap();",
"        let completion = gameplay.finish_mining(position, 7, 105).unwrap();",
1,
)
text = text.replace(
"        gameplay.begin_mining(position, 0, 1).unwrap();",
"        gameplay.begin_mining(position, 7, 0, 1).unwrap();",
1,
)
path.write_text(text)

# Server bridge: carry block-state token through begin and finish.
path = Path("crates/ferrum-server/src/mining_runtime.rs")
text = path.read_text()
text = text.replace(
'''        position: BlockPos,
        block: BlockMining,
    ) -> Result<Option<MiningStart>, MiningRuntimeError> {''',
'''        position: BlockPos,
        target_token: u64,
        block: BlockMining,
    ) -> Result<Option<MiningStart>, MiningRuntimeError> {''',
1,
)
text = text.replace(
"                .begin_mining(position, started_at_tick, required_ticks)",
"                .begin_mining(position, target_token, started_at_tick, required_ticks)",
1,
)
text = text.replace(
'''        uuid: PlayerUuid,
        position: BlockPos,
    ) -> Result<bool, MiningRuntimeError> {
        let current_tick''',
'''        uuid: PlayerUuid,
        position: BlockPos,
        target_token: u64,
    ) -> Result<bool, MiningRuntimeError> {
        let current_tick''',
1,
)
text = text.replace(
"            Ok(player.gameplay.finish_mining(position, current_tick))",
"            Ok(player.gameplay.finish_mining(position, target_token, current_tick))",
1,
)
text = text.replace(
'''                MiningSessionError::NoActiveSession
                | MiningSessionError::WrongTarget { .. }
                | MiningSessionError::TooEarly { .. },''',
'''                MiningSessionError::NoActiveSession
                | MiningSessionError::WrongTarget { .. }
                | MiningSessionError::TargetChanged { .. }
                | MiningSessionError::TooEarly { .. },''',
1,
)
# Tests use token 1.
text = text.replace(
".begin_mining(uuid, BlockPos { x: 0, y: 64, z: 0 }, block)",
".begin_mining(uuid, BlockPos { x: 0, y: 64, z: 0 }, 1, block)",
1,
)
text = text.replace(
".finish_mining(uuid, BlockPos { x: 0, y: 64, z: 0 })",
".finish_mining(uuid, BlockPos { x: 0, y: 64, z: 0 }, 1)",
1,
)
path.write_text(text)

# Play runtime: block-state ID is the token at both start and stop.
path = Path("crates/ferrum-server/src/play_runtime.rs")
text = path.read_text()
text = text.replace(
'''                                let _ = gameplay.runtime.begin_mining(
                                    gameplay.player_uuid,
                                    game_position,
                                    block,
                                )?;''',
'''                                let target_token = broken_state
                                    .map(|state| u64::from(state.get()))
                                    .unwrap_or_default();
                                let _ = gameplay.runtime.begin_mining(
                                    gameplay.player_uuid,
                                    game_position,
                                    target_token,
                                    block,
                                )?;''',
1,
)
text = text.replace(
'''                                gameplay
                                    .runtime
                                    .finish_mining(gameplay.player_uuid, game_position)?''',
'''                                let target_token = broken_state
                                    .map(|state| u64::from(state.get()))
                                    .unwrap_or_default();
                                gameplay.runtime.finish_mining(
                                    gameplay.player_uuid,
                                    game_position,
                                    target_token,
                                )?''',
1,
)
path.write_text(text)
