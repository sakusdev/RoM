use super::{
    KEEP_ALIVE_INTERVAL, MAX_IGNORED_PLAY_PACKETS, STATIC_CHUNK_RADIUS, STATIC_FLOOR_Y,
    STATIC_CHUNK_X, STATIC_CHUNK_Z, ServerConfig, is_connection_eof, version_26_1_2,
    write_play_payload,
};
use crate::codec::{PacketReader, read_packet};
use anyhow::{Context, Result, bail};
use ferrum_play::{
    PlayerMovement, PlayerState, decode_move_player_position,
    decode_move_player_position_rotation, decode_move_player_rotation,
    decode_move_player_status, encode_chunk_batch_finished, encode_chunk_batch_start,
    encode_forget_level_chunk, encode_keep_alive, encode_level_chunk_with_light,
    encode_set_chunk_cache_center,
};
use ferrum_protocol::{
    PacketDirection, PacketKind, ProtocolPhase, ProtocolProfile, ProtocolSession,
};
use ferrum_world::{
    BiomeId, BlockStateId, ChunkPos, ChunkView, ChunkViewDelta, FlatWorldSpec, StaticChunk,
};
use std::io::{Read, Write};

const CLIENT_TICKS_PER_SECOND: usize = 20;

pub(super) fn is_movement_packet_id(profile: &ProtocolProfile, packet_id: i32) -> bool {
    matches!(
        profile.packets().resolve(
            ProtocolPhase::Play,
            PacketDirection::Serverbound,
            packet_id,
        ),
        Some(
            PacketKind::MovePlayerPosition
                | PacketKind::MovePlayerPositionRotation
                | PacketKind::MovePlayerRotation
                | PacketKind::MovePlayerStatusOnly
        )
    )
}

pub(super) fn run_play_loop<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    play_round_limit: Option<usize>,
) -> Result<()> {
    if play_round_limit == Some(0) {
        return Ok(());
    }

    let mut player = PlayerState::new([0.5, 65.0, 0.5], 0.0, 0.0, false, false)?;
    let mut view = ChunkView::new(
        ChunkPos {
            x: STATIC_CHUNK_X,
            z: STATIC_CHUNK_Z,
        },
        STATIC_CHUNK_RADIUS,
    )?;
    view.mark_loaded(player.chunk_pos());
    let initial_delta = view.synchronize()?;
    send_chunk_view_delta(writer, profile, view.center(), &initial_delta)?;

    let tick_interval = usize::try_from(KEEP_ALIVE_INTERVAL.as_secs())
        .context("keep alive interval exceeds usize")?
        .checked_mul(CLIENT_TICKS_PER_SECOND)
        .context("keep alive tick interval overflow")?;
    let mut keep_alive_id = 1_i64;
    let mut completed_rounds = 0_usize;
    let mut ignored_packets = 0_usize;

    loop {
        write_play_payload(
            writer,
            profile,
            PacketKind::KeepAliveRequest,
            &encode_keep_alive(keep_alive_id),
        )?;
        session.keep_alive_sent(keep_alive_id)?;
        writer.flush()?;

        let mut keep_alive_acknowledged = false;
        let mut ticks_since_request = 0_usize;
        loop {
            let packet = match read_packet(reader).context("cannot read Play packet") {
                Ok(packet) => packet,
                Err(error) if is_connection_eof(&error) => {
                    session.disconnect();
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            let mut packet_reader = PacketReader::new(&packet);
            let packet_id = packet_reader.read_varint()?;
            let kind = profile.packets().resolve(
                ProtocolPhase::Play,
                PacketDirection::Serverbound,
                packet_id,
            );

            match kind {
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
                    let previous_chunk = player.chunk_pos();
                    player.apply(movement);
                    let current_chunk = player.chunk_pos();
                    if current_chunk != previous_chunk {
                        let delta = view.recenter(current_chunk)?;
                        send_chunk_view_delta(writer, profile, current_chunk, &delta)?;
                    }
                }
                _ => {
                    ignored_packets = ignored_packets
                        .checked_add(1)
                        .context("ignored Play packet count overflow")?;
                    if ignored_packets > MAX_IGNORED_PLAY_PACKETS {
                        bail!("ignored Play packet limit exceeded");
                    }
                }
            }

            if keep_alive_acknowledged && ticks_since_request >= tick_interval {
                break;
            }
        }

        keep_alive_id = keep_alive_id
            .checked_add(1)
            .context("keep alive id overflow")?;
    }
}

fn decode_movement(kind: PacketKind, payload: &[u8]) -> Result<PlayerMovement> {
    Ok(match kind {
        PacketKind::MovePlayerPosition => decode_move_player_position(payload)?,
        PacketKind::MovePlayerPositionRotation => {
            decode_move_player_position_rotation(payload)?
        }
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

fn send_chunk_view_delta<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    center: ChunkPos,
    delta: &ChunkViewDelta,
) -> Result<()> {
    if delta.center_changed {
        write_play_payload(
            writer,
            profile,
            PacketKind::SetChunkCacheCenter,
            &encode_set_chunk_cache_center(center.x, center.z),
        )?;
    }

    for pos in &delta.no_longer_visible {
        write_play_payload(
            writer,
            profile,
            PacketKind::ForgetLevelChunk,
            &encode_forget_level_chunk(*pos),
        )?;
    }

    if !delta.newly_visible.is_empty() {
        write_play_payload(
            writer,
            profile,
            PacketKind::ChunkBatchStart,
            &encode_chunk_batch_start(),
        )?;
        for pos in &delta.newly_visible {
            write_play_payload(
                writer,
                profile,
                PacketKind::LevelChunkWithLight,
                &encode_level_chunk_with_light(&flat_chunk(*pos)?)?,
            )?;
        }
        let batch_size = i32::try_from(delta.newly_visible.len())
            .context("visible chunk batch exceeds i32")?;
        write_play_payload(
            writer,
            profile,
            PacketKind::ChunkBatchFinished,
            &encode_chunk_batch_finished(batch_size)?,
        )?;
    }
    writer.flush()?;
    Ok(())
}

fn flat_chunk(pos: ChunkPos) -> Result<StaticChunk> {
    Ok(StaticChunk::flat_overworld(
        pos,
        version_26_1_2::OVERWORLD_MIN_SECTION_Y,
        version_26_1_2::OVERWORLD_SECTION_COUNT,
        FlatWorldSpec {
            floor_y: STATIC_FLOOR_Y,
            air: BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),
            bedrock: BlockStateId::new(version_26_1_2::BEDROCK_BLOCK_STATE_ID),
            stone: BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),
            dirt: BlockStateId::new(version_26_1_2::DIRT_BLOCK_STATE_ID),
            grass: BlockStateId::new(version_26_1_2::GRASS_BLOCK_STATE_ID),
            biome: BiomeId::new(version_26_1_2::PLAINS_BIOME_ID),
        },
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_packet_classifier_is_phase_and_direction_aware() {
        let profile = version_26_1_2::protocol_profile().unwrap();
        assert!(is_movement_packet_id(&profile, 0x1e));
        assert!(is_movement_packet_id(&profile, 0x1f));
        assert!(is_movement_packet_id(&profile, 0x20));
        assert!(is_movement_packet_id(&profile, 0x21));
        assert!(!is_movement_packet_id(&profile, 0x48));
    }
}
