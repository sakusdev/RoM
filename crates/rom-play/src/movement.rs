use rom_world::ChunkPos;
use thiserror::Error;

pub const MAX_PLAYER_COORDINATE: f64 = 30_000_000.0;
const ON_GROUND_FLAG: u8 = 0x01;
const HORIZONTAL_COLLISION_FLAG: u8 = 0x02;
const VALID_MOVEMENT_FLAGS: u8 = ON_GROUND_FLAG | HORIZONTAL_COLLISION_FLAG;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementFlags {
    pub on_ground: bool,
    pub horizontal_collision: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerMovement {
    Position {
        position: [f64; 3],
        flags: MovementFlags,
    },
    PositionRotation {
        position: [f64; 3],
        yaw: f32,
        pitch: f32,
        flags: MovementFlags,
    },
    Rotation {
        yaw: f32,
        pitch: f32,
        flags: MovementFlags,
    },
    StatusOnly {
        flags: MovementFlags,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerState {
    pub position: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub horizontal_collision: bool,
}

impl PlayerState {
    pub fn new(
        position: [f64; 3],
        yaw: f32,
        pitch: f32,
        on_ground: bool,
        horizontal_collision: bool,
    ) -> Result<Self, MovementDecodeError> {
        validate_position(position)?;
        validate_rotation(yaw, pitch)?;
        Ok(Self {
            position,
            yaw,
            pitch,
            on_ground,
            horizontal_collision,
        })
    }

    pub fn apply(&mut self, movement: PlayerMovement) {
        match movement {
            PlayerMovement::Position { position, flags } => {
                self.position = position;
                self.apply_flags(flags);
            }
            PlayerMovement::PositionRotation {
                position,
                yaw,
                pitch,
                flags,
            } => {
                self.position = position;
                self.yaw = yaw;
                self.pitch = pitch;
                self.apply_flags(flags);
            }
            PlayerMovement::Rotation { yaw, pitch, flags } => {
                self.yaw = yaw;
                self.pitch = pitch;
                self.apply_flags(flags);
            }
            PlayerMovement::StatusOnly { flags } => self.apply_flags(flags),
        }
    }

    #[must_use]
    pub fn chunk_pos(&self) -> ChunkPos {
        ChunkPos {
            x: (self.position[0] / 16.0).floor() as i32,
            z: (self.position[2] / 16.0).floor() as i32,
        }
    }

    fn apply_flags(&mut self, flags: MovementFlags) {
        self.on_ground = flags.on_ground;
        self.horizontal_collision = flags.horizontal_collision;
    }
}

pub fn decode_move_player_position(payload: &[u8]) -> Result<PlayerMovement, MovementDecodeError> {
    require_length(payload, 25)?;
    let position = [
        read_f64(payload, 0),
        read_f64(payload, 8),
        read_f64(payload, 16),
    ];
    validate_position(position)?;
    Ok(PlayerMovement::Position {
        position,
        flags: decode_flags(payload[24])?,
    })
}

pub fn decode_move_player_position_rotation(
    payload: &[u8],
) -> Result<PlayerMovement, MovementDecodeError> {
    require_length(payload, 33)?;
    let position = [
        read_f64(payload, 0),
        read_f64(payload, 8),
        read_f64(payload, 16),
    ];
    let yaw = read_f32(payload, 24);
    let pitch = read_f32(payload, 28);
    validate_position(position)?;
    validate_rotation(yaw, pitch)?;
    Ok(PlayerMovement::PositionRotation {
        position,
        yaw,
        pitch,
        flags: decode_flags(payload[32])?,
    })
}

pub fn decode_move_player_rotation(payload: &[u8]) -> Result<PlayerMovement, MovementDecodeError> {
    require_length(payload, 9)?;
    let yaw = read_f32(payload, 0);
    let pitch = read_f32(payload, 4);
    validate_rotation(yaw, pitch)?;
    Ok(PlayerMovement::Rotation {
        yaw,
        pitch,
        flags: decode_flags(payload[8])?,
    })
}

pub fn decode_move_player_status(payload: &[u8]) -> Result<PlayerMovement, MovementDecodeError> {
    require_length(payload, 1)?;
    Ok(PlayerMovement::StatusOnly {
        flags: decode_flags(payload[0])?,
    })
}

fn require_length(payload: &[u8], expected: usize) -> Result<(), MovementDecodeError> {
    if payload.len() != expected {
        return Err(MovementDecodeError::InvalidLength {
            expected,
            actual: payload.len(),
        });
    }
    Ok(())
}

fn decode_flags(value: u8) -> Result<MovementFlags, MovementDecodeError> {
    if value & !VALID_MOVEMENT_FLAGS != 0 {
        return Err(MovementDecodeError::InvalidFlags(value));
    }
    Ok(MovementFlags {
        on_ground: value & ON_GROUND_FLAG != 0,
        horizontal_collision: value & HORIZONTAL_COLLISION_FLAG != 0,
    })
}

fn validate_position(position: [f64; 3]) -> Result<(), MovementDecodeError> {
    for (axis, value) in ["x", "y", "z"].into_iter().zip(position) {
        if !value.is_finite() {
            return Err(MovementDecodeError::NonFiniteCoordinate { axis, value });
        }
        if value.abs() > MAX_PLAYER_COORDINATE {
            return Err(MovementDecodeError::CoordinateOutOfRange { axis, value });
        }
    }
    Ok(())
}

fn validate_rotation(yaw: f32, pitch: f32) -> Result<(), MovementDecodeError> {
    if !yaw.is_finite() {
        return Err(MovementDecodeError::NonFiniteRotation {
            axis: "yaw",
            value: yaw,
        });
    }
    if !pitch.is_finite() {
        return Err(MovementDecodeError::NonFiniteRotation {
            axis: "pitch",
            value: pitch,
        });
    }
    Ok(())
}

fn read_f64(payload: &[u8], offset: usize) -> f64 {
    f64::from_be_bytes(
        payload[offset..offset + 8]
            .try_into()
            .expect("length checked"),
    )
}

fn read_f32(payload: &[u8], offset: usize) -> f32 {
    f32::from_be_bytes(
        payload[offset..offset + 4]
            .try_into()
            .expect("length checked"),
    )
}

#[derive(Debug, Error, PartialEq)]
pub enum MovementDecodeError {
    #[error("movement payload length must be {expected} bytes, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("movement flags contain unsupported bits: 0x{0:02x}")]
    InvalidFlags(u8),
    #[error("movement coordinate {axis} is not finite: {value}")]
    NonFiniteCoordinate { axis: &'static str, value: f64 },
    #[error("movement coordinate {axis} is outside the supported range: {value}")]
    CoordinateOutOfRange { axis: &'static str, value: f64 },
    #[error("movement rotation {axis} is not finite: {value}")]
    NonFiniteRotation { axis: &'static str, value: f32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position_payload(position: [f64; 3], flags: u8) -> Vec<u8> {
        let mut payload = Vec::new();
        for coordinate in position {
            payload.extend_from_slice(&coordinate.to_be_bytes());
        }
        payload.push(flags);
        payload
    }

    #[test]
    fn decodes_position_and_flags_exactly() {
        let movement =
            decode_move_player_position(&position_payload([16.25, 65.0, -0.25], 0x03)).unwrap();
        assert_eq!(
            movement,
            PlayerMovement::Position {
                position: [16.25, 65.0, -0.25],
                flags: MovementFlags {
                    on_ground: true,
                    horizontal_collision: true,
                },
            }
        );
    }

    #[test]
    fn decodes_position_rotation_exactly() {
        let mut payload = position_payload([1.0, 2.0, 3.0], 0);
        let flags = payload.pop().unwrap();
        payload.extend_from_slice(&90.0_f32.to_be_bytes());
        payload.extend_from_slice(&(-30.0_f32).to_be_bytes());
        payload.push(flags | 0x01);
        assert_eq!(
            decode_move_player_position_rotation(&payload).unwrap(),
            PlayerMovement::PositionRotation {
                position: [1.0, 2.0, 3.0],
                yaw: 90.0,
                pitch: -30.0,
                flags: MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                },
            }
        );
    }

    #[test]
    fn decodes_rotation_and_status_packets() {
        let mut rotation = Vec::new();
        rotation.extend_from_slice(&180.0_f32.to_be_bytes());
        rotation.extend_from_slice(&45.0_f32.to_be_bytes());
        rotation.push(0x02);
        assert!(matches!(
            decode_move_player_rotation(&rotation).unwrap(),
            PlayerMovement::Rotation {
                yaw: 180.0,
                pitch: 45.0,
                ..
            }
        ));
        assert_eq!(
            decode_move_player_status(&[0x01]).unwrap(),
            PlayerMovement::StatusOnly {
                flags: MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                }
            }
        );
    }

    #[test]
    fn rejects_non_finite_and_out_of_range_coordinates() {
        let error =
            decode_move_player_position(&position_payload([f64::NAN, 0.0, 0.0], 0)).unwrap_err();
        assert!(matches!(
            error,
            MovementDecodeError::NonFiniteCoordinate { axis: "x", .. }
        ));

        let error = decode_move_player_position(&position_payload(
            [MAX_PLAYER_COORDINATE + 1.0, 0.0, 0.0],
            0,
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            MovementDecodeError::CoordinateOutOfRange { axis: "x", .. }
        ));
    }

    #[test]
    fn rejects_bad_lengths_and_unknown_flag_bits() {
        assert!(matches!(
            decode_move_player_status(&[]).unwrap_err(),
            MovementDecodeError::InvalidLength { .. }
        ));
        assert_eq!(
            decode_move_player_status(&[0x04]).unwrap_err(),
            MovementDecodeError::InvalidFlags(0x04)
        );
    }

    #[test]
    fn player_state_updates_selectively_and_tracks_negative_chunks() {
        let mut state = PlayerState::new([0.5, 65.0, 0.5], 0.0, 0.0, false, false).unwrap();
        state.apply(
            decode_move_player_rotation(&{
                let mut payload = Vec::new();
                payload.extend_from_slice(&30.0_f32.to_be_bytes());
                payload.extend_from_slice(&15.0_f32.to_be_bytes());
                payload.push(0x01);
                payload
            })
            .unwrap(),
        );
        assert_eq!(state.position, [0.5, 65.0, 0.5]);
        assert_eq!(
            (state.yaw, state.pitch, state.on_ground),
            (30.0, 15.0, true)
        );

        state
            .apply(decode_move_player_position(&position_payload([-0.1, 65.0, -16.1], 0)).unwrap());
        assert_eq!(state.chunk_pos(), ChunkPos { x: -1, z: -2 });
    }
}
