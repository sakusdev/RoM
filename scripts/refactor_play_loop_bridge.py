from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")

text = replace_once(
    text,
    '''    run_keep_alive_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world,
        play_reader,
        gameplay,
        play_round_limit,
    )''',
    '''    run_keep_alive_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world,
        PlayLoopBridge {
            play_reader,
            gameplay,
        },
        play_round_limit,
    )''',
    "live keep alive bridge call",
)

text = replace_once(
    text,
    '''    run_keep_alive_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world,
        None,
        None,
        play_round_limit,
    )''',
    '''    run_keep_alive_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world,
        PlayLoopBridge::default(),
        play_round_limit,
    )''',
    "test keep alive bridge call",
)

text = replace_once(
    text,
    '''fn run_keep_alive_loop_with_bridge<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    play_reader: Option<&PlayReaderEndpoint>,
    gameplay: Option<play_runtime::GameplaySync<'_>>,
    play_round_limit: Option<usize>,
) -> Result<()> {
    play_runtime::run_play_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world.shared_world,
        world.connection,
        play_reader,
        gameplay,
        play_round_limit,
    )
}''',
    '''#[derive(Debug, Clone, Copy, Default)]
struct PlayLoopBridge<'a> {
    play_reader: Option<&'a PlayReaderEndpoint>,
    gameplay: Option<play_runtime::GameplaySync<'a>>,
}

fn run_keep_alive_loop_with_bridge<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    world: PlayWorldContext<'_>,
    bridge: PlayLoopBridge<'_>,
    play_round_limit: Option<usize>,
) -> Result<()> {
    play_runtime::run_play_loop_with_bridge(
        reader,
        writer,
        profile,
        session,
        world.shared_world,
        world.connection,
        bridge.play_reader,
        bridge.gameplay,
        play_round_limit,
    )
}''',
    "play loop bridge function",
)

path.write_text(text, encoding="utf-8")
