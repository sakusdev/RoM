//! Deterministic Minecraft Java Edition Play-state payload codecs.
//!
//! Packet IDs are version metadata and intentionally live outside this crate.

use thiserror::Error;

const MAX_RESOURCE_LOCATION_BYTES: usize = 32_767;
const BLOCK_POS_XZ_MIN: i32 = -33_554_432;
const BLOCK_POS_XZ_MAX: i32 = 33_554_431;
const BLOCK_POS_Y_MIN: i32 = -2_048;
const BLOCK_POS_Y_MAX: i32 = 2_047;

#[derive(Debug, Clone, PartialEq)]
pub struct CommonPlayerSpawnInfo {
    pub dimension_type_id: i32,
    pub dimension: String,
    pub seed: i64,
    pub game_mode: i8,
    pub previous_game_mode: i8,
    pub is_debug: bool,
    pub is_flat: bool,
    pub last_death_location: Option<GlobalPosition>,
    pub portal_cooldown: i32,
    pub sea_level: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JoinGame {
    pub player_id: i32,
    pub hardcore: bool,
    pub levels: Vec<String>,
    pub max_players: i32,
    pub chunk_radius: i32,
    pub simulation_distance: i32,
    pub reduced_debug_info: bool,
    pub show_death_screen: bool,
    pub limited_crafting: bool,
    pub spawn_info: CommonPlayerSpawnInfo,
    pub enforces_secure_chat: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalPosition {
    pub dimension: String,
    pub position: BlockPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DefaultSpawnPosition {
    pub position: GlobalPosition,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionMoveRotation {
    pub position: [f64; 3],
    pub delta_movement: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerPosition {
    pub teleport_id: i32,
    pub change: PositionMoveRotation,
    pub relative_flags: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlayEncodeError {
    #[error("{field} cannot be negative: {value}")]
    NegativeValue { field: &'static str, value: i32 },
    #[error("collection length {length} exceeds the protocol VarInt range")]
    CollectionTooLong { length: usize },
    #[error("resource location is empty")]
    EmptyResourceLocation,
    #[error("resource location is too long: {length} bytes")]
    ResourceLocationTooLong { length: usize },
    #[error("block position is outside the wire range: ({x}, {y}, {z})")]
    BlockPositionOutOfRange { x: i32, y: i32, z: i32 },
    #[error("{field} must be finite")]
    NonFinite { field: &'static str },
}

#[must_use]
pub fn encode_keep_alive(id: i64) -> Vec<u8> {
    id.to_be_bytes().to_vec()
}

pub fn encode_join_game(packet: &JoinGame) -> Result<Vec<u8>, PlayEncodeError> {
    require_non_negative("max_players", packet.max_players)?;
    require_non_negative("chunk_radius", packet.chunk_radius)?;
    require_non_negative("simulation_distance", packet.simulation_distance)?;
    require_non_negative("dimension_type_id", packet.spawn_info.dimension_type_id)?;
    require_non_negative("portal_cooldown", packet.spawn_info.portal_cooldown)?;

    let mut output = Vec::new();
    output.extend_from_slice(&packet.player_id.to_be_bytes());
    write_bool(&mut output, packet.hardcore);
    write_len(&mut output, packet.levels.len())?;
    for level in &packet.levels {
        write_resource_location(&mut output, level)?;
    }
    write_varint(&mut output, packet.max_players);
    write_varint(&mut output, packet.chunk_radius);
    write_varint(&mut output, packet.simulation_distance);
    write_bool(&mut output, packet.reduced_debug_info);
    write_bool(&mut output, packet.show_death_screen);
    write_bool(&mut output, packet.limited_crafting);
    encode_common_spawn_info(&mut output, &packet.spawn_info)?;
    write_bool(&mut output, packet.enforces_secure_chat);
    Ok(output)
}

pub fn encode_default_spawn_position(
    packet: &DefaultSpawnPosition,
) -> Result<Vec<u8>, PlayEncodeError> {
    require_finite_f32("spawn yaw", packet.yaw)?;
    require_finite_f32("spawn pitch", packet.pitch)?;
    let mut output = Vec::new();
    encode_global_position(&mut output, &packet.position)?;
    output.extend_from_slice(&packet.yaw.to_be_bytes());
    output.extend_from_slice(&packet.pitch.to_be_bytes());
    Ok(output)
}

pub fn encode_player_position(packet: &PlayerPosition) -> Result<Vec<u8>, PlayEncodeError> {
    require_non_negative("teleport_id", packet.teleport_id)?;
    for (field, value) in [
        ("position x", packet.change.position[0]),
        ("position y", packet.change.position[1]),
        ("position z", packet.change.position[2]),
        ("delta x", packet.change.delta_movement[0]),
        ("delta y", packet.change.delta_movement[1]),
        ("delta z", packet.change.delta_movement[2]),
    ] {
        require_finite_f64(field, value)?;
    }
    require_finite_f32("yaw", packet.change.yaw)?;
    require_finite_f32("pitch", packet.change.pitch)?;

    let mut output = Vec::new();
    write_varint(&mut output, packet.teleport_id);
    for value in packet.change.position {
        output.extend_from_slice(&value.to_be_bytes());
    }
    for value in packet.change.delta_movement {
        output.extend_from_slice(&value.to_be_bytes());
    }
    output.extend_from_slice(&packet.change.yaw.to_be_bytes());
    output.extend_from_slice(&packet.change.pitch.to_be_bytes());
    output.extend_from_slice(&packet.relative_flags.to_be_bytes());
    Ok(output)
}

fn encode_common_spawn_info(
    output: &mut Vec<u8>,
    info: &CommonPlayerSpawnInfo,
) -> Result<(), PlayEncodeError> {
    write_varint(output, info.dimension_type_id);
    write_resource_location(output, &info.dimension)?;
    output.extend_from_slice(&info.seed.to_be_bytes());
    output.push(info.game_mode as u8);
    output.push(info.previous_game_mode as u8);
    write_bool(output, info.is_debug);
    write_bool(output, info.is_flat);
    match &info.last_death_location {
        Some(position) => {
            write_bool(output, true);
            encode_global_position(output, position)?;
        }
        None => write_bool(output, false),
    }
    write_varint(output, info.portal_cooldown);
    write_varint(output, info.sea_level);
    Ok(())
}

fn encode_global_position(
    output: &mut Vec<u8>,
    position: &GlobalPosition,
) -> Result<(), PlayEncodeError> {
    write_resource_location(output, &position.dimension)?;
    output.extend_from_slice(&pack_block_position(position.position)?.to_be_bytes());
    Ok(())
}

fn pack_block_position(position: BlockPosition) -> Result<i64, PlayEncodeError> {
    if !(BLOCK_POS_XZ_MIN..=BLOCK_POS_XZ_MAX).contains(&position.x)
        || !(BLOCK_POS_XZ_MIN..=BLOCK_POS_XZ_MAX).contains(&position.z)
        || !(BLOCK_POS_Y_MIN..=BLOCK_POS_Y_MAX).contains(&position.y)
    {
        return Err(PlayEncodeError::BlockPositionOutOfRange {
            x: position.x,
            y: position.y,
            z: position.z,
        });
    }
    let x = i64::from(position.x) & 0x3ff_ffff;
    let y = i64::from(position.y) & 0xfff;
    let z = i64::from(position.z) & 0x3ff_ffff;
    Ok((x << 38) | (z << 12) | y)
}

fn write_bool(output: &mut Vec<u8>, value: bool) {
    output.push(u8::from(value));
}

fn write_resource_location(output: &mut Vec<u8>, value: &str) -> Result<(), PlayEncodeError> {
    if value.is_empty() {
        return Err(PlayEncodeError::EmptyResourceLocation);
    }
    if value.len() > MAX_RESOURCE_LOCATION_BYTES {
        return Err(PlayEncodeError::ResourceLocationTooLong {
            length: value.len(),
        });
    }
    write_varint(
        output,
        i32::try_from(value.len()).map_err(|_| PlayEncodeError::ResourceLocationTooLong {
            length: value.len(),
        })?,
    );
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_len(output: &mut Vec<u8>, length: usize) -> Result<(), PlayEncodeError> {
    let length =
        i32::try_from(length).map_err(|_| PlayEncodeError::CollectionTooLong { length })?;
    write_varint(output, length);
    Ok(())
}

fn write_varint(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn require_non_negative(field: &'static str, value: i32) -> Result<(), PlayEncodeError> {
    if value < 0 {
        Err(PlayEncodeError::NegativeValue { field, value })
    } else {
        Ok(())
    }
}

fn require_finite_f32(field: &'static str, value: f32) -> Result<(), PlayEncodeError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(PlayEncodeError::NonFinite { field })
    }
}

fn require_finite_f64(field: &'static str, value: f64) -> Result<(), PlayEncodeError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(PlayEncodeError::NonFinite { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn static_spawn_info() -> CommonPlayerSpawnInfo {
        CommonPlayerSpawnInfo {
            dimension_type_id: 0,
            dimension: "minecraft:overworld".to_owned(),
            seed: 0,
            game_mode: 0,
            previous_game_mode: -1,
            is_debug: false,
            is_flat: true,
            last_death_location: None,
            portal_cooldown: 0,
            sea_level: 63,
        }
    }

    #[test]
    fn encodes_static_join_game_payload_exactly() {
        let payload = encode_join_game(&JoinGame {
            player_id: 1,
            hardcore: false,
            levels: vec!["minecraft:overworld".to_owned()],
            max_players: 20,
            chunk_radius: 2,
            simulation_distance: 2,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            spawn_info: static_spawn_info(),
            enforces_secure_chat: false,
        })
        .unwrap();

        let mut expected = vec![0, 0, 0, 1, 0, 1, 19];
        expected.extend_from_slice(b"minecraft:overworld");
        expected.extend_from_slice(&[20, 2, 2, 0, 1, 0, 0, 19]);
        expected.extend_from_slice(b"minecraft:overworld");
        expected.extend_from_slice(&0_i64.to_be_bytes());
        expected.extend_from_slice(&[0, 0xff, 0, 1, 0, 0, 63, 0]);
        assert_eq!(payload, expected);
    }

    #[test]
    fn encodes_default_spawn_position_exactly() {
        let payload = encode_default_spawn_position(&DefaultSpawnPosition {
            position: GlobalPosition {
                dimension: "minecraft:overworld".to_owned(),
                position: BlockPosition { x: 0, y: 64, z: 0 },
            },
            yaw: 0.0,
            pitch: 0.0,
        })
        .unwrap();
        let mut expected = vec![19];
        expected.extend_from_slice(b"minecraft:overworld");
        expected.extend_from_slice(&64_i64.to_be_bytes());
        expected.extend_from_slice(&0_f32.to_be_bytes());
        expected.extend_from_slice(&0_f32.to_be_bytes());
        assert_eq!(payload, expected);
    }

    #[test]
    fn encodes_player_position_with_absolute_flags() {
        let payload = encode_player_position(&PlayerPosition {
            teleport_id: 1,
            change: PositionMoveRotation {
                position: [0.5, 65.0, 0.5],
                delta_movement: [0.0, 0.0, 0.0],
                yaw: 0.0,
                pitch: 0.0,
            },
            relative_flags: 0,
        })
        .unwrap();
        let mut expected = vec![1];
        for value in [0.5_f64, 65.0, 0.5, 0.0, 0.0, 0.0] {
            expected.extend_from_slice(&value.to_be_bytes());
        }
        expected.extend_from_slice(&0_f32.to_be_bytes());
        expected.extend_from_slice(&0_f32.to_be_bytes());
        expected.extend_from_slice(&0_u32.to_be_bytes());
        assert_eq!(payload, expected);
    }

    #[test]
    fn packs_negative_block_coordinates() {
        let packed = pack_block_position(BlockPosition {
            x: -1,
            y: -1,
            z: -1,
        })
        .unwrap();
        assert_eq!(packed as u64, u64::MAX);
    }

    #[test]
    fn rejects_non_finite_position() {
        let error = encode_player_position(&PlayerPosition {
            teleport_id: 1,
            change: PositionMoveRotation {
                position: [f64::NAN, 0.0, 0.0],
                delta_movement: [0.0; 3],
                yaw: 0.0,
                pitch: 0.0,
            },
            relative_flags: 0,
        })
        .unwrap_err();
        assert_eq!(
            error,
            PlayEncodeError::NonFinite {
                field: "position x"
            }
        );
    }
}
