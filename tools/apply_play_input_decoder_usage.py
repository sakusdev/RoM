from pathlib import Path

path = Path("crates/ferrum-server/src/play_runtime.rs")
source = path.read_text()

old_import = '''use ferrum_play::{
    BlockPosition, PlayerMovement, PlayerState, decode_move_player_position,
    decode_move_player_position_rotation, decode_move_player_rotation, decode_move_player_status,
    decode_player_action, decode_use_item_on_block, encode_block_changed_ack, encode_block_update,
'''
new_import = '''use ferrum_play::{
    BlockPosition, PlayerMovement, PlayerState, decode_player_action, decode_use_item_on_block,
    encode_block_changed_ack, encode_block_update,
'''
if old_import not in source:
    raise SystemExit("movement decoder import marker not found")
source = source.replace(old_import, new_import, 1)

protocol_import = '''use ferrum_protocol::{
    PacketDirection, PacketKind, ProtocolPhase, ProtocolProfile, ProtocolSession,
};
'''
protocol_replacement = protocol_import + '''use ferrum_server::{authoritative_runtime::PlayInput, play_input::decode_play_input};
'''
if protocol_import not in source:
    raise SystemExit("protocol import marker not found")
source = source.replace(protocol_import, protocol_replacement, 1)

old_match = '''            match kind {
                Some(PacketKind::KeepAliveResponse) => {
                    let received_id = packet_reader.read_i64()?;
                    if received_id != keep_alive_id {
                        bail!("expected keep alive id {keep_alive_id}, got {received_id}");
                    }
                    require_empty(&mut packet_reader, "keep alive response")?;
                    session.keep_alive_response(keep_alive_id)?;
                    keep_alive_acknowledged = true;
                    completed_rounds += 1;
                    if play_round_limit.is_some_and(|limit| completed_rounds >= limit) {
                        return Ok(());
                    }
                }
                Some(PacketKind::ClientTickEnd) => {
                    require_empty(&mut packet_reader, "client tick end")?;
                    ticks_since_request = ticks_since_request.saturating_add(1);
                }
                Some(PacketKind::ChunkBatchReceived) => {
                    let desired_chunks_per_tick = packet_reader.read_f32()?;
                    if !desired_chunks_per_tick.is_finite() || desired_chunks_per_tick <= 0.0 {
                        bail!(
                            "chunk batch acknowledgement contains invalid desired chunks per tick {desired_chunks_per_tick}"
                        );
                    }
                    require_empty(&mut packet_reader, "chunk batch acknowledgement")?;
                }
                Some(
                    kind @ (PacketKind::MovePlayerPosition
                    | PacketKind::MovePlayerPositionRotation
                    | PacketKind::MovePlayerRotation
                    | PacketKind::MovePlayerStatusOnly),
                ) => {
                    let movement = decode_movement(kind, packet_reader.take_remaining())?;
                    validate_movement_delta(&player, movement)?;
                    validate_movement_floor(movement, world_profile.floor_y)?;
                    let previous_chunk = player.chunk_pos();
                    player.apply(movement);
                    let current_chunk = player.chunk_pos();
                    if current_chunk != previous_chunk {
                        let delta = view.recenter(current_chunk)?;
                        shared_world.ensure_chunks_loaded(&delta.newly_visible)?;
                        send_chunk_view_delta(
                            writer,
                            profile,
                            shared_world,
                            current_chunk,
                            &delta,
                        )?;
                    }
                }
'''
new_match = '''            match kind {
                Some(
                    kind @ (PacketKind::KeepAliveResponse
                    | PacketKind::ClientTickEnd
                    | PacketKind::ChunkBatchReceived
                    | PacketKind::MovePlayerPosition
                    | PacketKind::MovePlayerPositionRotation
                    | PacketKind::MovePlayerRotation
                    | PacketKind::MovePlayerStatusOnly),
                ) => {
                    let input = decode_play_input(kind, packet_reader.take_remaining())?
                        .context("resolved migrated Play packet did not decode")?;
                    match input {
                        PlayInput::KeepAliveResponse(received_id) => {
                            if received_id != keep_alive_id {
                                bail!(
                                    "expected keep alive id {keep_alive_id}, got {received_id}"
                                );
                            }
                            session.keep_alive_response(keep_alive_id)?;
                            keep_alive_acknowledged = true;
                            completed_rounds += 1;
                            if play_round_limit.is_some_and(|limit| completed_rounds >= limit) {
                                return Ok(());
                            }
                        }
                        PlayInput::ClientTickEnd => {
                            ticks_since_request = ticks_since_request.saturating_add(1);
                        }
                        PlayInput::ChunkBatchReceived(_) => {}
                        PlayInput::Movement(movement) => {
                            validate_movement_delta(&player, movement)?;
                            validate_movement_floor(movement, world_profile.floor_y)?;
                            let previous_chunk = player.chunk_pos();
                            player.apply(movement);
                            let current_chunk = player.chunk_pos();
                            if current_chunk != previous_chunk {
                                let delta = view.recenter(current_chunk)?;
                                shared_world.ensure_chunks_loaded(&delta.newly_visible)?;
                                send_chunk_view_delta(
                                    writer,
                                    profile,
                                    shared_world,
                                    current_chunk,
                                    &delta,
                                )?;
                            }
                        }
                        PlayInput::Disconnected => {
                            unreachable!("socket disconnect is not decoded from a Play packet")
                        }
                    }
                }
'''
if old_match not in source:
    raise SystemExit("Play input match marker not found")
source = source.replace(old_match, new_match, 1)

helpers = '''fn decode_movement(kind: PacketKind, payload: &[u8]) -> Result<PlayerMovement> {
    Ok(match kind {
        PacketKind::MovePlayerPosition => decode_move_player_position(payload)?,
        PacketKind::MovePlayerPositionRotation => decode_move_player_position_rotation(payload)?,
        PacketKind::MovePlayerRotation => decode_move_player_rotation(payload)?,
        PacketKind::MovePlayerStatusOnly => decode_move_player_status(payload)?,
        _ => bail!("packet {kind:?} is not a movement packet"),
    })
}

fn require_empty(reader: &mut PacketReader<'_>, label: &str) -> Result<()> {
    if !reader.take_remaining().is_empty() {
        bail!("{label} contains trailing bytes");
    }
    Ok(())
}

'''
if helpers not in source:
    raise SystemExit("obsolete decoder helper marker not found")
source = source.replace(helpers, "", 1)

path.write_text(source)
