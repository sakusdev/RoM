from pathlib import Path

main_path = Path("crates/ferrum-server/src/main.rs")
play_path = Path("crates/ferrum-server/src/play_runtime.rs")
main = main_path.read_text()
play = play_path.read_text()

for signature in (
    "fn run_static_play_session<R: Read, W: Write>(",
    "fn wait_for_play_bootstrap_acknowledgements<R: Read>(",
    "fn run_keep_alive_loop<R: Read, W: Write>(",
):
    marker = f"#[cfg(test)]\n#[allow(dead_code)]\n{signature}"
    if marker in main:
        continue
    main = main.replace(f"#[cfg(test)]\n{signature}", marker, 1)
    if marker not in main:
        main = main.replace(signature, marker, 1)

static_bridge = "fn run_static_play_session_with_bridge<R: Read, W: Write>("
static_marker = (
    "#[expect(\n"
    "    clippy::too_many_arguments,\n"
    "    reason = \"transitional bridge preserves the finite legacy call boundary\"\n"
    ")]\n"
    f"{static_bridge}"
)
if static_marker not in main:
    main = main.replace(static_bridge, static_marker, 1)

keep_bridge = "fn run_keep_alive_loop_with_bridge<R: Read, W: Write>("
unneeded_keep_expect = (
    "#[expect(\n"
    "    clippy::too_many_arguments,\n"
    "    reason = \"transitional bridge preserves the finite legacy call boundary\"\n"
    ")]\n"
    f"{keep_bridge}"
)
main = main.replace(unneeded_keep_expect, keep_bridge, 1)

legacy_play = "pub(super) fn run_play_loop<R: Read, W: Write>("
legacy_play_marker = f"#[cfg(test)]\n#[allow(dead_code)]\n{legacy_play}"
if legacy_play_marker not in play:
    play = play.replace(f"#[cfg(test)]\n{legacy_play}", legacy_play_marker, 1)
    if legacy_play_marker not in play:
        play = play.replace(legacy_play, legacy_play_marker, 1)

bridge_play = "pub(super) fn run_play_loop_with_bridge<R: Read, W: Write>("
bridge_marker = (
    "#[expect(\n"
    "    clippy::too_many_arguments,\n"
    "    reason = \"transitional bridge preserves the finite legacy call boundary\"\n"
    ")]\n"
    f"{bridge_play}"
)
if bridge_marker not in play:
    play = play.replace(bridge_play, bridge_marker, 1)

main_path.write_text(main)
play_path.write_text(play)
