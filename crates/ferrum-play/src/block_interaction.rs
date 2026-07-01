use crate::BlockPosition;
use ferrum_world::{BlockMutation, BlockPos, BlockStateId, WorldEvent};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionHand {
    Main,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFace {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerActionStatus {
    StartDestroyBlock,
    AbortDestroyBlock,
    StopDestroyBlock,
    DropAllItems,
    DropItem,
    ReleaseUseItem,
    SwapItemWithOffhand,
    Stab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerAction {
    pub status: PlayerActionStatus,
    pub position: BlockPosition,
    pub face: BlockFace,
    pub sequence: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UseItemOnBlock {
    pub hand: InteractionHand,
    pub position: BlockPosition,
    pub face: BlockFace,
    pub cursor: [f32; 3],
    pub inside_block: bool,
    pub world_border_hit: bool,
    pub sequence: i32,
}

pub fn decode_player_action(payload: &[u8]) -> Result<PlayerAction, BlockInteractionDecodeError> {
    let mut reader = PayloadReader::new(payload);
    let status = decode_player_action_status(reader.read_varint()?)?;
    let position = reader.read_block_position()?;
    let face = decode_block_face(i32::from(reader.read_i8()?))?;
    let sequence = decode_sequence(reader.read_varint()?)?;
    reader.require_empty()?;
    Ok(PlayerAction {
        status,
        position,
        face,
        sequence,
    })
}

pub fn decode_use_item_on_block(
    payload: &[u8],
) -> Result<UseItemOnBlock, BlockInteractionDecodeError> {
    let mut reader = PayloadReader::new(payload);
    let hand = decode_hand(reader.read_varint()?)?;
    let position = reader.read_block_position()?;
    let face = decode_block_face(reader.read_varint()?)?;
    let cursor = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
    for (axis, value) in ["x", "y", "z"].into_iter().zip(cursor) {
        if !value.is_finite() {
            return Err(BlockInteractionDecodeError::NonFiniteCursor { axis, value });
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(BlockInteractionDecodeError::CursorOutOfRange { axis, value });
        }
    }
    let inside_block = reader.read_bool()?;
    let world_border_hit = reader.read_bool()?;
    let sequence = decode_sequence(reader.read_varint()?)?;
    reader.require_empty()?;
    Ok(UseItemOnBlock {
        hand,
        position,
        face,
        cursor,
        inside_block,
        world_border_hit,
        sequence,
    })
}

#[must_use]
pub fn block_position_to_world(position: BlockPosition) -> BlockPos {
    BlockPos {
        x: position.x,
        y: position.y,
        z: position.z,
    }
}

#[must_use]
pub fn player_action_to_world_event(action: PlayerAction, air: BlockStateId) -> Option<WorldEvent> {
    match action.status {
        PlayerActionStatus::StopDestroyBlock => Some(WorldEvent::BlockMutation(BlockMutation {
            position: block_position_to_world(action.position),
            state: air,
        })),
        _ => None,
    }
}

#[must_use]
pub fn use_item_on_block_to_world_event(
    interaction: UseItemOnBlock,
    placed_state: BlockStateId,
) -> Option<WorldEvent> {
    if interaction.world_border_hit {
        return None;
    }
    let [dx, dy, dz] = interaction.face.offset();
    Some(WorldEvent::BlockMutation(BlockMutation {
        position: BlockPos {
            x: interaction.position.x + dx,
            y: interaction.position.y + dy,
            z: interaction.position.z + dz,
        },
        state: placed_state,
    }))
}

impl BlockFace {
    #[must_use]
    const fn offset(self) -> [i32; 3] {
        match self {
            Self::Down => [0, -1, 0],
            Self::Up => [0, 1, 0],
            Self::North => [0, 0, -1],
            Self::South => [0, 0, 1],
            Self::West => [-1, 0, 0],
            Self::East => [1, 0, 0],
        }
    }
}

fn decode_sequence(value: i32) -> Result<i32, BlockInteractionDecodeError> {
    if value < 0 {
        Err(BlockInteractionDecodeError::NegativeSequence(value))
    } else {
        Ok(value)
    }
}

fn decode_hand(value: i32) -> Result<InteractionHand, BlockInteractionDecodeError> {
    match value {
        0 => Ok(InteractionHand::Main),
        1 => Ok(InteractionHand::Off),
        other => Err(BlockInteractionDecodeError::InvalidHand(other)),
    }
}

fn decode_block_face(value: i32) -> Result<BlockFace, BlockInteractionDecodeError> {
    match value {
        0 => Ok(BlockFace::Down),
        1 => Ok(BlockFace::Up),
        2 => Ok(BlockFace::North),
        3 => Ok(BlockFace::South),
        4 => Ok(BlockFace::West),
        5 => Ok(BlockFace::East),
        other => Err(BlockInteractionDecodeError::InvalidBlockFace(other)),
    }
}

fn decode_player_action_status(
    value: i32,
) -> Result<PlayerActionStatus, BlockInteractionDecodeError> {
    match value {
        0 => Ok(PlayerActionStatus::StartDestroyBlock),
        1 => Ok(PlayerActionStatus::AbortDestroyBlock),
        2 => Ok(PlayerActionStatus::StopDestroyBlock),
        3 => Ok(PlayerActionStatus::DropAllItems),
        4 => Ok(PlayerActionStatus::DropItem),
        5 => Ok(PlayerActionStatus::ReleaseUseItem),
        6 => Ok(PlayerActionStatus::SwapItemWithOffhand),
        7 => Ok(PlayerActionStatus::Stab),
        other => Err(BlockInteractionDecodeError::InvalidPlayerActionStatus(
            other,
        )),
    }
}

fn unpack_block_position(value: i64) -> BlockPosition {
    BlockPosition {
        x: (value >> 38) as i32,
        y: (value << 52 >> 52) as i32,
        z: (value << 26 >> 38) as i32,
    }
}

struct PayloadReader<'a> {
    payload: &'a [u8],
    cursor: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, cursor: 0 }
    }

    fn read_varint(&mut self) -> Result<i32, BlockInteractionDecodeError> {
        let mut value = 0i32;
        for position in 0..5 {
            let byte = self.read_u8()?;
            value |= i32::from(byte & 0x7f) << (7 * position);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(BlockInteractionDecodeError::VarIntTooLong)
    }

    fn read_block_position(&mut self) -> Result<BlockPosition, BlockInteractionDecodeError> {
        Ok(unpack_block_position(self.read_i64()?))
    }

    fn read_i64(&mut self) -> Result<i64, BlockInteractionDecodeError> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_f32(&mut self) -> Result<f32, BlockInteractionDecodeError> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i8(&mut self) -> Result<i8, BlockInteractionDecodeError> {
        Ok(i8::from_be_bytes([self.read_u8()?]))
    }

    fn read_bool(&mut self) -> Result<bool, BlockInteractionDecodeError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(BlockInteractionDecodeError::InvalidBool(other)),
        }
    }

    fn read_u8(&mut self) -> Result<u8, BlockInteractionDecodeError> {
        Ok(*self.read_bytes(1)?.first().expect("one byte was just read"))
    }

    fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], BlockInteractionDecodeError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(BlockInteractionDecodeError::CursorOverflow)?;
        let bytes = self.payload.get(self.cursor..end).ok_or(
            BlockInteractionDecodeError::UnexpectedEof {
                needed: length,
                remaining: self.payload.len().saturating_sub(self.cursor),
            },
        )?;
        self.cursor = end;
        Ok(bytes)
    }

    fn require_empty(&self) -> Result<(), BlockInteractionDecodeError> {
        if self.cursor == self.payload.len() {
            Ok(())
        } else {
            Err(BlockInteractionDecodeError::TrailingBytes {
                count: self.payload.len() - self.cursor,
            })
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum BlockInteractionDecodeError {
    #[error("block interaction VarInt exceeds 5 bytes")]
    VarIntTooLong,
    #[error("block interaction payload ended while reading {needed} bytes; {remaining} remain")]
    UnexpectedEof { needed: usize, remaining: usize },
    #[error("block interaction cursor arithmetic overflowed")]
    CursorOverflow,
    #[error("invalid interaction hand {0}")]
    InvalidHand(i32),
    #[error("invalid block face {0}")]
    InvalidBlockFace(i32),
    #[error("invalid player action status {0}")]
    InvalidPlayerActionStatus(i32),
    #[error("block interaction sequence cannot be negative: {0}")]
    NegativeSequence(i32),
    #[error("boolean field must be 0 or 1, got {0}")]
    InvalidBool(u8),
    #[error("cursor coordinate {axis} is not finite: {value}")]
    NonFiniteCursor { axis: &'static str, value: f32 },
    #[error("cursor coordinate {axis} is outside the block: {value}")]
    CursorOutOfRange { axis: &'static str, value: f32 },
    #[error("block interaction payload has {count} trailing bytes")]
    TrailingBytes { count: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn pack_block_position(position: BlockPosition) -> i64 {
        let x = i64::from(position.x) & 0x3ff_ffff;
        let y = i64::from(position.y) & 0xfff;
        let z = i64::from(position.z) & 0x3ff_ffff;
        (x << 38) | (z << 12) | y
    }

    #[test]
    fn decodes_player_action_exactly() {
        let mut payload = Vec::new();
        write_varint(&mut payload, 0);
        payload.extend_from_slice(
            &pack_block_position(BlockPosition {
                x: -1,
                y: 64,
                z: -16,
            })
            .to_be_bytes(),
        );
        payload.push(1);
        write_varint(&mut payload, 42);

        assert_eq!(
            decode_player_action(&payload).unwrap(),
            PlayerAction {
                status: PlayerActionStatus::StartDestroyBlock,
                position: BlockPosition {
                    x: -1,
                    y: 64,
                    z: -16
                },
                face: BlockFace::Up,
                sequence: 42,
            }
        );
    }

    #[test]
    fn decodes_26_1_2_stab_action() {
        let mut payload = Vec::new();
        write_varint(&mut payload, 7);
        payload.extend_from_slice(&0_i64.to_be_bytes());
        payload.push(0);
        write_varint(&mut payload, 1);
        assert_eq!(
            decode_player_action(&payload).unwrap().status,
            PlayerActionStatus::Stab
        );
    }

    #[test]
    fn decodes_use_item_on_block_exactly() {
        let mut payload = Vec::new();
        write_varint(&mut payload, 1);
        payload.extend_from_slice(
            &pack_block_position(BlockPosition { x: 1, y: 2, z: 3 }).to_be_bytes(),
        );
        write_varint(&mut payload, 5);
        for value in [0.25_f32, 0.5, 0.75] {
            payload.extend_from_slice(&value.to_be_bytes());
        }
        payload.push(1);
        payload.push(0);
        write_varint(&mut payload, 7);

        assert_eq!(
            decode_use_item_on_block(&payload).unwrap(),
            UseItemOnBlock {
                hand: InteractionHand::Off,
                position: BlockPosition { x: 1, y: 2, z: 3 },
                face: BlockFace::East,
                cursor: [0.25, 0.5, 0.75],
                inside_block: true,
                world_border_hit: false,
                sequence: 7,
            }
        );
    }

    #[test]
    fn rejects_invalid_block_interaction_payloads() {
        assert_eq!(
            decode_player_action(&[8]).unwrap_err(),
            BlockInteractionDecodeError::InvalidPlayerActionStatus(8)
        );

        let mut payload = Vec::new();
        write_varint(&mut payload, 0);
        payload.extend_from_slice(&0_i64.to_be_bytes());
        write_varint(&mut payload, 0);
        payload.extend_from_slice(&f32::NAN.to_be_bytes());
        payload.extend_from_slice(&0.0_f32.to_be_bytes());
        payload.extend_from_slice(&0.0_f32.to_be_bytes());
        payload.push(0);
        payload.push(0);
        write_varint(&mut payload, 0);
        assert!(matches!(
            decode_use_item_on_block(&payload).unwrap_err(),
            BlockInteractionDecodeError::NonFiniteCursor { axis: "x", .. }
        ));

        let mut payload = Vec::new();
        write_varint(&mut payload, 0);
        payload.extend_from_slice(&0_i64.to_be_bytes());
        write_varint(&mut payload, 0);
        payload.extend_from_slice(&0.0_f32.to_be_bytes());
        payload.extend_from_slice(&1.25_f32.to_be_bytes());
        payload.extend_from_slice(&0.0_f32.to_be_bytes());
        payload.push(0);
        payload.push(0);
        write_varint(&mut payload, 0);
        assert_eq!(
            decode_use_item_on_block(&payload).unwrap_err(),
            BlockInteractionDecodeError::CursorOutOfRange {
                axis: "y",
                value: 1.25
            }
        );
    }

    #[test]
    fn maps_block_interactions_to_world_events() {
        let air = BlockStateId::new(0);
        let stone = BlockStateId::new(1);
        let position = BlockPosition { x: 4, y: 65, z: -2 };

        assert_eq!(
            player_action_to_world_event(
                PlayerAction {
                    status: PlayerActionStatus::StopDestroyBlock,
                    position,
                    face: BlockFace::Up,
                    sequence: 3,
                },
                air,
            ),
            Some(WorldEvent::BlockMutation(BlockMutation {
                position: BlockPos { x: 4, y: 65, z: -2 },
                state: air,
            }))
        );
        assert_eq!(
            player_action_to_world_event(
                PlayerAction {
                    status: PlayerActionStatus::StartDestroyBlock,
                    position,
                    face: BlockFace::Up,
                    sequence: 4,
                },
                air,
            ),
            None
        );
        assert_eq!(
            use_item_on_block_to_world_event(
                UseItemOnBlock {
                    hand: InteractionHand::Main,
                    position,
                    face: BlockFace::East,
                    cursor: [0.5, 0.5, 0.5],
                    inside_block: false,
                    world_border_hit: false,
                    sequence: 5,
                },
                stone,
            ),
            Some(WorldEvent::BlockMutation(BlockMutation {
                position: BlockPos { x: 5, y: 65, z: -2 },
                state: stone,
            }))
        );
        assert_eq!(
            use_item_on_block_to_world_event(
                UseItemOnBlock {
                    hand: InteractionHand::Main,
                    position,
                    face: BlockFace::Up,
                    cursor: [0.5, 0.5, 0.5],
                    inside_block: false,
                    world_border_hit: true,
                    sequence: 6,
                },
                stone,
            ),
            None
        );
    }
}
