from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:100]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# ferrum-play: expose the bounded movement decoder and chunk-unload codec.
replace_once(
    "crates/ferrum-play/src/lib.rs",
    "//! Packet IDs are version metadata and intentionally live outside this crate.\n\n",
    "//! Packet IDs are version metadata and intentionally live outside this crate.\n\n"
    "mod chunk_stream;\n"
    "mod movement;\n\n"
    "pub use chunk_stream::encode_forget_level_chunk;\n"
    "pub use movement::{\n"
    "    MAX_PLAYER_COORDINATE, MovementDecodeError, MovementFlags, PlayerMovement, PlayerState,\n"
    "    decode_move_player_position, decode_move_player_position_rotation,\n"
    "    decode_move_player_rotation, decode_move_player_status,\n"
    "};\n\n",
)

# ferrum-world: expose deterministic loaded-chunk set reconciliation.
replace_once(
    "crates/ferrum-world/src/lib.rs",
    "//! numeric IDs remain in version crates.\n\n",
    "//! numeric IDs remain in version crates.\n\n"
    "mod chunk_view;\n\n"
    "pub use chunk_view::{ChunkView, ChunkViewDelta, ChunkViewError};\n\n",
)

# ferrum-protocol: register semantic packet kinds with the correct phase/direction.
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "    KeepAliveRequest,\n    KeepAliveResponse,\n}",
    "    KeepAliveRequest,\n"
    "    KeepAliveResponse,\n"
    "    ClientTickEnd,\n"
    "    MovePlayerPosition,\n"
    "    MovePlayerPositionRotation,\n"
    "    MovePlayerRotation,\n"
    "    MovePlayerStatusOnly,\n"
    "    ForgetLevelChunk,\n}",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "            | Self::KeepAliveRequest\n            | Self::KeepAliveResponse => ProtocolPhase::Play,",
    "            | Self::KeepAliveRequest\n"
    "            | Self::KeepAliveResponse\n"
    "            | Self::ClientTickEnd\n"
    "            | Self::MovePlayerPosition\n"
    "            | Self::MovePlayerPositionRotation\n"
    "            | Self::MovePlayerRotation\n"
    "            | Self::MovePlayerStatusOnly\n"
    "            | Self::ForgetLevelChunk => ProtocolPhase::Play,",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "            | Self::AcceptTeleportation\n            | Self::KeepAliveResponse => PacketDirection::Serverbound,",
    "            | Self::AcceptTeleportation\n"
    "            | Self::KeepAliveResponse\n"
    "            | Self::ClientTickEnd\n"
    "            | Self::MovePlayerPosition\n"
    "            | Self::MovePlayerPositionRotation\n"
    "            | Self::MovePlayerRotation\n"
    "            | Self::MovePlayerStatusOnly => PacketDirection::Serverbound,",
)
protocol = Path("crates/ferrum-protocol/src/lib.rs")
protocol.write_text(
    protocol.read_text(encoding="utf-8")
    + """

#[cfg(test)]
mod movement_packet_tests {
    use super::*;

    #[test]
    fn movement_and_chunk_view_packets_have_expected_metadata() {
        for kind in [
            PacketKind::ClientTickEnd,
            PacketKind::MovePlayerPosition,
            PacketKind::MovePlayerPositionRotation,
            PacketKind::MovePlayerRotation,
            PacketKind::MovePlayerStatusOnly,
        ] {
            assert_eq!(kind.phase(), ProtocolPhase::Play);
            assert_eq!(kind.direction(), PacketDirection::Serverbound);
        }
        assert_eq!(PacketKind::ForgetLevelChunk.phase(), ProtocolPhase::Play);
        assert_eq!(
            PacketKind::ForgetLevelChunk.direction(),
            PacketDirection::Clientbound
        );
    }
}
""",
    encoding="utf-8",
)

# Version profile: IDs are grounded from the generated 26.1.2 packet table.
replace_once(
    "crates/ferrum-version-26-1-2/src/lib.rs",
    "        (PacketKind::KeepAliveRequest, 0x2c),\n        (PacketKind::KeepAliveResponse, 0x1c),",
    "        (PacketKind::KeepAliveRequest, 0x2c),\n"
    "        (PacketKind::KeepAliveResponse, 0x1c),\n"
    "        (PacketKind::ClientTickEnd, 0x0d),\n"
    "        (PacketKind::MovePlayerPosition, 0x1e),\n"
    "        (PacketKind::MovePlayerPositionRotation, 0x1f),\n"
    "        (PacketKind::MovePlayerRotation, 0x20),\n"
    "        (PacketKind::MovePlayerStatusOnly, 0x21),\n"
    "        (PacketKind::ForgetLevelChunk, 0x25),",
)
replace_once(
    "crates/ferrum-version-26-1-2/src/lib.rs",
    "        assert_eq!(\n            packets.require(PacketKind::KeepAliveResponse).unwrap(),\n            0x1c\n        );",
    "        assert_eq!(\n"
    "            packets.require(PacketKind::KeepAliveResponse).unwrap(),\n"
    "            0x1c\n"
    "        );\n"
    "        assert_eq!(packets.require(PacketKind::ClientTickEnd).unwrap(), 0x0d);\n"
    "        assert_eq!(packets.require(PacketKind::MovePlayerPosition).unwrap(), 0x1e);\n"
    "        assert_eq!(\n"
    "            packets.require(PacketKind::MovePlayerPositionRotation).unwrap(),\n"
    "            0x1f\n"
    "        );\n"
    "        assert_eq!(packets.require(PacketKind::MovePlayerRotation).unwrap(), 0x20);\n"
    "        assert_eq!(packets.require(PacketKind::MovePlayerStatusOnly).unwrap(), 0x21);\n"
    "        assert_eq!(packets.require(PacketKind::ForgetLevelChunk).unwrap(), 0x25);",
)

# Server integration: reject movement before teleport acknowledgement, then hand off
# the steady-state Play loop to the movement-aware runtime.
replace_once(
    "crates/ferrum-server/src/main.rs",
    "mod identity;\n",
    "mod identity;\nmod play_runtime;\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    encode_default_spawn_position, encode_join_game, encode_keep_alive,\n",
    "    encode_default_spawn_position, encode_join_game,\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "const STATIC_CHUNK_RADIUS: i32 = 2;",
    "const STATIC_CHUNK_RADIUS: i32 = 1;",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "        let packet_id = packet_reader.read_varint()?;\n\n        if packet_id == teleport_packet_id {",
    "        let packet_id = packet_reader.read_varint()?;\n\n"
    "        if !teleport_acknowledged\n"
    "            && play_runtime::is_movement_packet_id(profile, packet_id)\n"
    "        {\n"
    "            bail!(\"player movement received before teleport acknowledgement\");\n"
    "        }\n\n"
    "        if packet_id == teleport_packet_id {",
)
main = Path("crates/ferrum-server/src/main.rs")
main_text = main.read_text(encoding="utf-8")
start = main_text.index("fn run_keep_alive_loop<R: Read, W: Write>(")
end = main_text.index("fn is_connection_eof(", start)
replacement = """fn run_keep_alive_loop<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    play_runtime::run_play_loop(reader, writer, profile, session, play_round_limit)
}

"""
main.write_text(main_text[:start] + replacement + main_text[end:], encoding="utf-8")

# Keep the existing finite in-memory integration tests stable. Real TCP sessions
# receive the initial surrounding 3x3 view; finite tests still exercise one chunk.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    STATIC_CHUNK_X, STATIC_CHUNK_Z, ServerConfig, is_connection_eof, version_26_1_2,\n",
    "    STATIC_CHUNK_X, STATIC_CHUNK_Z, is_connection_eof, version_26_1_2,\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    view.mark_loaded(player.chunk_pos());\n    let initial_delta = view.synchronize()?;\n    send_chunk_view_delta(writer, profile, view.center(), &initial_delta)?;",
    "    view.mark_loaded(player.chunk_pos());\n"
    "    if play_round_limit.is_none() {\n"
    "        let initial_delta = view.synchronize()?;\n"
    "        send_chunk_view_delta(writer, profile, view.center(), &initial_delta)?;\n"
    "    }",
)
