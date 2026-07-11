from pathlib import Path
import re


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


entity = Path("crates/ferrum-game/src/entity.rs")
text = entity.read_text(encoding="utf-8")
text = replace_once(
    text,
    "    #[must_use]\n    pub fn iter(&self) -> impl Iterator<Item = (&EntityId, &Entity)> {",
    "    pub fn iter(&self) -> impl Iterator<Item = (&EntityId, &Entity)> {",
    "entity iter double must_use",
)
entity.write_text(text, encoding="utf-8")

main = Path("crates/ferrum-server/src/main.rs")
text = main.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use ferrum_nbt::{Tag, encode_anonymous};",
    "use ferrum_game::{GameState, PlayerUuid as GamePlayerUuid, Transform};\nuse ferrum_nbt::{Tag, encode_anonymous};",
    "game imports",
)
text = replace_once(
    text,
    "    authoritative_runtime::{PlayInput, PlayOutput},\n",
    "    authoritative_runtime::{PlayInput, PlayOutput},\n    game_runtime::SharedGameRuntime,\n",
    "shared game runtime import",
)
text = replace_once(
    text,
    "use identity::offline_player_identity;",
    "use identity::{PlayerIdentity, offline_player_identity};",
    "identity import",
)
text = replace_once(
    text,
    "    shared_play_runtime: SharedPlayRuntime,\n}",
    "    shared_play_runtime: SharedPlayRuntime,\n    game_runtime: SharedGameRuntime,\n}",
    "server state field",
)
text = replace_once(
    text,
    "        let center = play_runtime::spawn_chunk(&world);\n        let shared_runtime_config = shared_play_runtime_config(&play_policy)?;",
    "        let center = play_runtime::spawn_chunk(&world);\n        let game_runtime = SharedGameRuntime::new(GameState::new(world.dimension.clone())?);\n        let shared_runtime_config = shared_play_runtime_config(&play_policy)?;",
    "game runtime construction",
)
text = replace_once(
    text,
    "            shared_play_runtime,\n        })",
    "            shared_play_runtime,\n            game_runtime,\n        })",
    "game runtime initialization",
)

pattern = re.compile(
    r"    fn try_enter_play\(&self\) -> Result<OnlinePlayerGuard<'_>> \{.*?\n    \}\n\n    #\[cfg\(test\)\]",
    re.S,
)
replacement = '''    fn try_enter_play(
        &self,
        identity: &PlayerIdentity,
        transform: Transform,
    ) -> Result<OnlinePlayerGuard<'_>> {
        let player_uuid = GamePlayerUuid::from_bytes(*identity.uuid.as_bytes());
        self.game_runtime
            .connect_player(player_uuid, identity.username.clone(), transform)?;
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);
        let connection_id = ConnectionId::new(id);
        let endpoints = register_play_connection(
            &self.shared_play_runtime.connector(),
            connection_id,
            NonZeroUsize::new(PLAY_OUTPUT_QUEUE_CAPACITY)
                .expect("Play output queue capacity is non-zero"),
        );
        let (play_reader, play_writer) = match endpoints {
            Ok(endpoints) => endpoints,
            Err(error) => {
                let _ = self.game_runtime.disconnect_player(player_uuid);
                return Err(error);
            }
        };
        self.online_players.fetch_add(1, Ordering::Relaxed);
        Ok(OnlinePlayerGuard {
            state: self,
            connection_id,
            player_uuid,
            play_reader,
            play_writer: Some(play_writer),
        })
    }

    #[cfg(test)]'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1:
    raise SystemExit("replacement target not found: try_enter_play function")

helper_pattern = re.compile(
    r"    fn enter_play\(&self\) -> OnlinePlayerGuard<'_> \{.*?\n    \}",
    re.S,
)
helper_replacement = '''    fn enter_play(&self) -> OnlinePlayerGuard<'_> {
        let identity = offline_player_identity("TestPlayer");
        let transform = game_spawn_transform(self.world.world_profile())
            .expect("test spawn transform must be valid");
        self.try_enter_play(&identity, transform)
            .expect("test Play connection must register")
    }'''
text, count = helper_pattern.subn(helper_replacement, text, count=1)
if count != 1:
    raise SystemExit("replacement target not found: test enter_play helper")

text = replace_once(
    text,
    "    connection_id: ConnectionId,\n    play_reader: PlayReaderEndpoint,",
    "    connection_id: ConnectionId,\n    player_uuid: GamePlayerUuid,\n    play_reader: PlayReaderEndpoint,",
    "online player UUID field",
)
text = replace_once(
    text,
    "        let _ = self.play_reader.try_disconnect();\n        self.state.online_players.fetch_sub(1, Ordering::Relaxed);",
    "        let _ = self.play_reader.try_disconnect();\n        let _ = self.state.game_runtime.disconnect_player(self.player_uuid);\n        self.state.online_players.fetch_sub(1, Ordering::Relaxed);",
    "disconnect cleanup",
)
text = replace_once(
    text,
    "                context,\n                profile,\n                session,",
    "                context,\n                &identity,\n                profile,\n                session,",
    "configuration identity argument",
)

config_signature = '''    context: ServerContext<'_>,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let config = context.config;'''
config_signature_with_identity = '''    context: ServerContext<'_>,
    identity: &PlayerIdentity,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let config = context.config;'''
text = replace_once(
    text,
    config_signature,
    config_signature_with_identity,
    "configuration signature",
)
text = replace_once(
    text,
    "    handle_play_protocol(reader, writer, context, profile, session, play_round_limit)",
    "    handle_play_protocol(\n        reader,\n        writer,\n        context,\n        identity,\n        profile,\n        session,\n        play_round_limit,\n    )",
    "play call identity",
)

play_signature = '''    context: ServerContext<'_>,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let config = context.config;
    if config.profile_name.as_deref() != Some(version_26_1_2::PROFILE_NAME) {'''
play_signature_with_identity = '''    context: ServerContext<'_>,
    identity: &PlayerIdentity,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    let config = context.config;
    if config.profile_name.as_deref() != Some(version_26_1_2::PROFILE_NAME) {'''
text = replace_once(
    text,
    play_signature,
    play_signature_with_identity,
    "play signature",
)
text = replace_once(
    text,
    "    let mut online_player = context.state.try_enter_play()?;",
    "    let transform = game_spawn_transform(context.state.world().world_profile())?;\n    let mut online_player = context.state.try_enter_play(identity, transform)?;",
    "live gameplay registration",
)
text = replace_once(
    text,
    "fn static_player_position(world: &RomPackWorld) -> PlayerPosition {",
    '''fn game_spawn_transform(world: &RomPackWorld) -> Result<Transform> {
    Transform::new(
        play_runtime::player_spawn_position(world),
        0.0,
        0.0,
        false,
    )
    .context("generated player spawn transform is invalid")
}

fn static_player_position(world: &RomPackWorld) -> PlayerPosition {''',
    "spawn transform helper",
)
main.write_text(text, encoding="utf-8")
