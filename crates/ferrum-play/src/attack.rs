use ferrum_game::EntityId;
use thiserror::Error;

const MAX_VARINT_BYTES: usize = 5;

pub fn decode_attack(payload: &[u8]) -> Result<EntityId, AttackDecodeError> {
    let (entity_id, consumed) = read_varint(payload)?;
    if consumed != payload.len() {
        return Err(AttackDecodeError::TrailingBytes {
            remaining: payload.len() - consumed,
        });
    }
    if entity_id <= 0 {
        return Err(AttackDecodeError::InvalidEntityId { entity_id });
    }
    EntityId::new(entity_id as u32).map_err(|_| AttackDecodeError::InvalidEntityId { entity_id })
}

fn read_varint(payload: &[u8]) -> Result<(i32, usize), AttackDecodeError> {
    let mut value = 0_u32;
    for index in 0..MAX_VARINT_BYTES {
        let Some(&byte) = payload.get(index) else {
            return Err(AttackDecodeError::UnexpectedEof);
        };
        value |= u32::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            if index == MAX_VARINT_BYTES - 1 && byte & 0xf0 != 0 {
                return Err(AttackDecodeError::VarIntTooLarge);
            }
            return Ok((value as i32, index + 1));
        }
    }
    Err(AttackDecodeError::VarIntTooLarge)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AttackDecodeError {
    #[error("attack packet ended before the entity ID VarInt completed")]
    UnexpectedEof,
    #[error("attack entity ID VarInt exceeds five bytes or the i32 range")]
    VarIntTooLarge,
    #[error("attack entity ID {entity_id} must be positive")]
    InvalidEntityId { entity_id: i32 },
    #[error("attack packet contains {remaining} trailing bytes")]
    TrailingBytes { remaining: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_26_1_2_attack_entity_id_varint() {
        assert_eq!(decode_attack(&[0xac, 0x02]).unwrap().get(), 300);
    }

    #[test]
    fn rejects_invalid_or_non_canonical_attack_payloads() {
        assert_eq!(decode_attack(&[]), Err(AttackDecodeError::UnexpectedEof));
        assert_eq!(
            decode_attack(&[0]),
            Err(AttackDecodeError::InvalidEntityId { entity_id: 0 })
        );
        assert_eq!(
            decode_attack(&[1, 0]),
            Err(AttackDecodeError::TrailingBytes { remaining: 1 })
        );
        assert_eq!(
            decode_attack(&[0x80; 5]),
            Err(AttackDecodeError::VarIntTooLarge)
        );
        assert_eq!(
            decode_attack(&[0xff, 0xff, 0xff, 0xff, 0x10]),
            Err(AttackDecodeError::VarIntTooLarge)
        );
    }
}
