use crate::authoritative_runtime::PlayInput;
use anyhow::{Context, Result, bail};
use rom_play::{
    decode_move_player_position, decode_move_player_position_rotation, decode_move_player_rotation,
    decode_move_player_status,
};
use rom_protocol::PacketKind;

/// Decode one already-resolved serverbound Play packet payload into the
/// version-neutral input consumed by the authoritative runtime.
///
/// Packet ID resolution remains owned by the protocol profile. Unsupported
/// packets return `None` so the current ignored-packet policy can remain at the
/// connection boundary while inputs migrated to the authoritative owner use a
/// shared decoder.
pub fn decode_play_input(kind: PacketKind, payload: &[u8]) -> Result<Option<PlayInput>> {
    let input = match kind {
        PacketKind::ClientTickEnd => {
            require_length("client tick end", payload, 0)?;
            PlayInput::ClientTickEnd
        }
        PacketKind::Attack => {
            PlayInput::AttackEntity(decode_positive_varint("attack entity id", payload)?)
        }
        PacketKind::KeepAliveResponse => {
            require_length("keep alive response", payload, 8)?;
            PlayInput::KeepAliveResponse(i64::from_be_bytes(
                payload.try_into().expect("keep alive length checked"),
            ))
        }
        PacketKind::ChunkBatchReceived => {
            require_length("chunk batch acknowledgement", payload, 4)?;
            let desired_chunks_per_tick = f32::from_be_bytes(
                payload
                    .try_into()
                    .expect("chunk batch acknowledgement length checked"),
            );
            if !desired_chunks_per_tick.is_finite() || desired_chunks_per_tick <= 0.0 {
                bail!(
                    "chunk batch acknowledgement contains invalid desired chunks per tick {desired_chunks_per_tick}"
                );
            }
            PlayInput::ChunkBatchReceived(desired_chunks_per_tick)
        }
        PacketKind::MovePlayerPosition => PlayInput::Movement(
            decode_move_player_position(payload).context("cannot decode player position")?,
        ),
        PacketKind::MovePlayerPositionRotation => PlayInput::Movement(
            decode_move_player_position_rotation(payload)
                .context("cannot decode player position and rotation")?,
        ),
        PacketKind::MovePlayerRotation => PlayInput::Movement(
            decode_move_player_rotation(payload).context("cannot decode player rotation")?,
        ),
        PacketKind::MovePlayerStatusOnly => PlayInput::Movement(
            decode_move_player_status(payload).context("cannot decode player status")?,
        ),
        _ => return Ok(None),
    };
    Ok(Some(input))
}

fn decode_positive_varint(name: &str, payload: &[u8]) -> Result<u32> {
    let mut value = 0_u32;
    let mut cursor = 0_usize;
    for shift in (0..35).step_by(7) {
        let Some(&byte) = payload.get(cursor) else {
            bail!("{name} contains a truncated VarInt");
        };
        cursor += 1;
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            if cursor != payload.len() {
                bail!("{name} payload contains trailing bytes");
            }
            let signed = value as i32;
            if signed <= 0 {
                bail!("{name} must be a positive entity id, got {signed}");
            }
            return Ok(signed as u32);
        }
    }
    bail!("{name} VarInt is too long")
}

fn require_length(name: &str, payload: &[u8], expected: usize) -> Result<()> {
    if payload.len() != expected {
        bail!(
            "{name} payload length must be {expected} bytes, got {}",
            payload.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rom_play::{MovementFlags, PlayerMovement};

    #[test]
    fn decodes_tick_and_keep_alive_payloads_exactly() {
        assert_eq!(
            decode_play_input(PacketKind::ClientTickEnd, &[]).unwrap(),
            Some(PlayInput::ClientTickEnd)
        );
        assert_eq!(
            decode_play_input(PacketKind::KeepAliveResponse, &41_i64.to_be_bytes()).unwrap(),
            Some(PlayInput::KeepAliveResponse(41))
        );
        assert!(
            decode_play_input(PacketKind::ClientTickEnd, &[0])
                .unwrap_err()
                .to_string()
                .contains("must be 0 bytes")
        );
    }

    #[test]
    fn decodes_dedicated_attack_entity_id() {
        assert_eq!(
            decode_play_input(PacketKind::Attack, &[0xac, 0x02]).unwrap(),
            Some(PlayInput::AttackEntity(300))
        );
        assert!(decode_play_input(PacketKind::Attack, &[0]).is_err());
        assert!(decode_play_input(PacketKind::Attack, &[1, 0]).is_err());
    }

    #[test]
    fn validates_chunk_batch_acknowledgements() {
        assert_eq!(
            decode_play_input(PacketKind::ChunkBatchReceived, &8.5_f32.to_be_bytes()).unwrap(),
            Some(PlayInput::ChunkBatchReceived(8.5))
        );
        for invalid in [0.0_f32, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                decode_play_input(PacketKind::ChunkBatchReceived, &invalid.to_be_bytes())
                    .unwrap_err()
                    .to_string()
                    .contains("invalid desired chunks per tick")
            );
        }
    }

    #[test]
    fn decodes_all_movement_shapes() {
        let mut position = Vec::new();
        position.extend_from_slice(&1.0_f64.to_be_bytes());
        position.extend_from_slice(&65.0_f64.to_be_bytes());
        position.extend_from_slice(&(-2.0_f64).to_be_bytes());
        position.push(0x03);
        assert_eq!(
            decode_play_input(PacketKind::MovePlayerPosition, &position).unwrap(),
            Some(PlayInput::Movement(PlayerMovement::Position {
                position: [1.0, 65.0, -2.0],
                flags: MovementFlags {
                    on_ground: true,
                    horizontal_collision: true,
                },
            }))
        );

        let mut position_rotation = position[..24].to_vec();
        position_rotation.extend_from_slice(&90.0_f32.to_be_bytes());
        position_rotation.extend_from_slice(&15.0_f32.to_be_bytes());
        position_rotation.push(0x01);
        assert!(matches!(
            decode_play_input(PacketKind::MovePlayerPositionRotation, &position_rotation).unwrap(),
            Some(PlayInput::Movement(PlayerMovement::PositionRotation { .. }))
        ));

        let mut rotation = Vec::new();
        rotation.extend_from_slice(&45.0_f32.to_be_bytes());
        rotation.extend_from_slice(&(-10.0_f32).to_be_bytes());
        rotation.push(0x00);
        assert!(matches!(
            decode_play_input(PacketKind::MovePlayerRotation, &rotation).unwrap(),
            Some(PlayInput::Movement(PlayerMovement::Rotation { .. }))
        ));
        assert_eq!(
            decode_play_input(PacketKind::MovePlayerStatusOnly, &[0x02]).unwrap(),
            Some(PlayInput::Movement(PlayerMovement::StatusOnly {
                flags: MovementFlags {
                    on_ground: false,
                    horizontal_collision: true,
                },
            }))
        );
    }

    #[test]
    fn leaves_unmigrated_play_packets_at_the_connection_boundary() {
        assert_eq!(
            decode_play_input(PacketKind::PlayerAction, &[1, 2, 3]).unwrap(),
            None
        );
    }
}
