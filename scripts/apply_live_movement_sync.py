from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


play_runtime = Path("crates/ferrum-server/src/play_runtime.rs")
text = play_runtime.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use anyhow::{Context, Result, bail};\nuse ferrum_play::{",
    "use anyhow::{Context, Result, bail};\nuse ferrum_game::{PlayerUuid as GamePlayerUuid, Transform};\nuse ferrum_play::{",
    "gameplay imports",
)
text = replace_once(
    text,
    "    authoritative_runtime::{PlayInput, PlayOutput},\n    play_connection::PlayReaderEndpoint,",
    "    authoritative_runtime::{PlayInput, PlayOutput},\n    game_runtime::SharedGameRuntime,\n    play_connection::PlayReaderEndpoint,",
    "shared gameplay runtime import",
)
text = replace_once(
    text,
    "type LocalWorldRuntime = DeterministicRuntime<ChunkStore, WorldEvent>;\n",
    '''type LocalWorldRuntime = DeterministicRuntime<ChunkStore, WorldEvent>;

#[derive(Debug, Clone, Copy)]
pub(super) struct GameplaySync<'a> {
    runtime: &'a SharedGameRuntime,
    player_uuid: GamePlayerUuid,
}

impl<'a> GameplaySync<'a> {
    #[must_use]
    pub(super) const fn new(
        runtime: &'a SharedGameRuntime,
        player_uuid: GamePlayerUuid,
    ) -> Self {
        Self {
            runtime,
            player_uuid,
        }
    }

    fn synchronize(self, player: &PlayerState) -> Result<()> {
        let transform = Transform::new(
            player.position,
            player.yaw,
            player.pitch,
            player.on_ground,
        )?;
        self.runtime.move_player(self.player_uuid, transform)?;
        Ok(())
    }
}
''',
    "gameplay sync type",
)
text = replace_once(
    text,
    "        connection,\n        None,\n        play_round_limit,",
    "        connection,\n        None,\n        None,\n        play_round_limit,",
    "test play loop default gameplay",
)
text = replace_once(
    text,
    "    connection: ConnectionId,\n    play_reader: Option<&PlayReaderEndpoint>,\n    play_round_limit: Option<usize>,",
    "    connection: ConnectionId,\n    play_reader: Option<&PlayReaderEndpoint>,\n    gameplay: Option<GameplaySync<'_>>,\n    play_round_limit: Option<usize>,",
    "play loop gameplay argument",
)
text = replace_once(
    text,
    "                            player.apply(movement);\n                            let current_chunk = player.chunk_pos();",
    "                            player.apply(movement);\n                            if let Some(gameplay) = gameplay {\n                                gameplay.synchronize(&player)?;\n                            }\n                            let current_chunk = player.chunk_pos();",
    "movement synchronization",
)
play_runtime.write_text(text, encoding="utf-8")

main = Path("crates/ferrum-server/src/main.rs")
text = main.read_text(encoding="utf-8")
text = replace_once(
    text,
    "    fn play_reader(&self) -> &PlayReaderEndpoint {",
    "    fn player_uuid(&self) -> GamePlayerUuid {\n        self.player_uuid\n    }\n\n    fn play_reader(&self) -> &PlayReaderEndpoint {",
    "guard player UUID accessor",
)
text = replace_once(
    text,
    "    let play_reader = writer_worker.as_ref().map(|_| online_player.play_reader());\n    let result = run_static_play_session_with_bridge(",
    "    let play_reader = writer_worker.as_ref().map(|_| online_player.play_reader());\n    let gameplay = play_runtime::GameplaySync::new(\n        &context.state.game_runtime,\n        online_player.player_uuid(),\n    );\n    let result = run_static_play_session_with_bridge(",
    "live gameplay context",
)
text = replace_once(
    text,
    "        play_reader,\n        play_round_limit,\n    );",
    "        play_reader,\n        Some(gameplay),\n        play_round_limit,\n    );",
    "live session gameplay argument",
)
text = replace_once(
    text,
    "        world,\n        None,\n        play_round_limit,\n    )",
    "        world,\n        None,\n        None,\n        play_round_limit,\n    )",
    "static session default gameplay",
)
text = replace_once(
    text,
    "    world: PlayWorldContext<'_>,\n    play_reader: Option<&PlayReaderEndpoint>,\n    play_round_limit: Option<usize>,\n) -> Result<()> {",
    "    world: PlayWorldContext<'_>,\n    play_reader: Option<&PlayReaderEndpoint>,\n    gameplay: Option<play_runtime::GameplaySync<'_>>,\n    play_round_limit: Option<usize>,\n) -> Result<()> {",
    "static session gameplay argument",
)
text = replace_once(
    text,
    "        world,\n        play_reader,\n        play_round_limit,\n    )\n}",
    "        world,\n        play_reader,\n        gameplay,\n        play_round_limit,\n    )\n}",
    "static session forwards gameplay",
)
text = replace_once(
    text,
    "        world,\n        None,\n        play_round_limit,\n    )\n}",
    "        world,\n        None,\n        None,\n        play_round_limit,\n    )\n}",
    "keep alive default gameplay",
)
text = replace_once(
    text,
    "    world: PlayWorldContext<'_>,\n    play_reader: Option<&PlayReaderEndpoint>,\n    play_round_limit: Option<usize>,\n) -> Result<()> {\n    play_runtime::run_play_loop_with_bridge(",
    "    world: PlayWorldContext<'_>,\n    play_reader: Option<&PlayReaderEndpoint>,\n    gameplay: Option<play_runtime::GameplaySync<'_>>,\n    play_round_limit: Option<usize>,\n) -> Result<()> {\n    play_runtime::run_play_loop_with_bridge(",
    "keep alive gameplay argument",
)
text = replace_once(
    text,
    "        world.connection,\n        play_reader,\n        play_round_limit,",
    "        world.connection,\n        play_reader,\n        gameplay,\n        play_round_limit,",
    "play loop receives gameplay",
)
main.write_text(text, encoding="utf-8")
