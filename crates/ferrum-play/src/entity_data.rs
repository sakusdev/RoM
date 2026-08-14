//! Version-neutral wire encoder for Play `SetEntityData` payloads.
//!
//! Metadata indices and serializer IDs are version metadata. Callers provide
//! those numeric values; this module only owns the bounded wire framing.

use ferrum_game::EntityId;
use thiserror::Error;

pub const ENTITY_DATA_TERMINATOR: u8 = 0xff;
pub const MAX_ENTITY_DATA_ENTRIES: usize = 254;
pub const MAX_ENTITY_DATA_VALUE_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityDataEntry<'a> {
    pub index: u8,
    pub serializer_id: i32,
    pub value: &'a [u8],
}

impl<'a> EntityDataEntry<'a> {
    #[must_use]
    pub const fn new(index: u8, serializer_id: i32, value: &'a [u8]) -> Self {
        Self {
            index,
            serializer_id,
            value,
        }
    }
}

pub fn encode_entity_data(
    entity_id: EntityId,
    entries: &[EntityDataEntry<'_>],
) -> Result<Vec<u8>, EntityDataEncodeError> {
    if entries.len() > MAX_ENTITY_DATA_ENTRIES {
        return Err(EntityDataEncodeError::TooManyEntries {
            count: entries.len(),
        });
    }
    let mut seen = [false; 255];
    let mut output = Vec::new();
    write_varint(
        &mut output,
        i32::try_from(entity_id.get()).map_err(|_| EntityDataEncodeError::EntityIdOutOfRange {
            entity_id: entity_id.get(),
        })?,
    );
    for entry in entries {
        if entry.index == ENTITY_DATA_TERMINATOR {
            return Err(EntityDataEncodeError::ReservedIndex);
        }
        if seen[usize::from(entry.index)] {
            return Err(EntityDataEncodeError::DuplicateIndex { index: entry.index });
        }
        if entry.serializer_id < 0 {
            return Err(EntityDataEncodeError::NegativeSerializerId {
                serializer_id: entry.serializer_id,
            });
        }
        if entry.value.len() > MAX_ENTITY_DATA_VALUE_BYTES {
            return Err(EntityDataEncodeError::ValueTooLarge {
                index: entry.index,
                length: entry.value.len(),
            });
        }
        seen[usize::from(entry.index)] = true;
        output.push(entry.index);
        write_varint(&mut output, entry.serializer_id);
        output.extend_from_slice(entry.value);
    }
    output.push(ENTITY_DATA_TERMINATOR);
    Ok(output)
}

pub fn encode_empty_entity_data(entity_id: EntityId) -> Result<Vec<u8>, EntityDataEncodeError> {
    encode_entity_data(entity_id, &[])
}

fn write_varint(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            output.push(value as u8);
            return;
        }
        output.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EntityDataEncodeError {
    #[error("entity id {entity_id} exceeds the protocol VarInt range")]
    EntityIdOutOfRange { entity_id: u32 },
    #[error("entity metadata index 255 is reserved as the terminator")]
    ReservedIndex,
    #[error("entity metadata index {index} appears more than once")]
    DuplicateIndex { index: u8 },
    #[error("entity metadata serializer id cannot be negative: {serializer_id}")]
    NegativeSerializerId { serializer_id: i32 },
    #[error("entity metadata has {count} entries, exceeding {MAX_ENTITY_DATA_ENTRIES}")]
    TooManyEntries { count: usize },
    #[error("entity metadata value at index {index} has {length} bytes, exceeding {MAX_ENTITY_DATA_VALUE_BYTES}")]
    ValueTooLarge { index: u8, length: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_single_raw_metadata_entry() {
        let id = EntityId::new(5).unwrap();
        assert_eq!(
            encode_entity_data(id, &[EntityDataEntry::new(8, 7, &[1, 2, 3])]).unwrap(),
            vec![5, 8, 7, 1, 2, 3, 0xff]
        );
    }

    #[test]
    fn empty_metadata_keeps_protocol_terminator() {
        let id = EntityId::new(128).unwrap();
        assert_eq!(encode_empty_entity_data(id).unwrap(), vec![0x80, 0x01, 0xff]);
    }

    #[test]
    fn rejects_duplicate_and_reserved_indices() {
        let id = EntityId::new(1).unwrap();
        assert_eq!(
            encode_entity_data(
                id,
                &[
                    EntityDataEntry::new(8, 1, &[]),
                    EntityDataEntry::new(8, 1, &[]),
                ],
            )
            .unwrap_err(),
            EntityDataEncodeError::DuplicateIndex { index: 8 }
        );
        assert_eq!(
            encode_entity_data(id, &[EntityDataEntry::new(0xff, 1, &[])]).unwrap_err(),
            EntityDataEncodeError::ReservedIndex
        );
    }

    #[test]
    fn rejects_negative_serializer_id() {
        let id = EntityId::new(1).unwrap();
        assert_eq!(
            encode_entity_data(id, &[EntityDataEntry::new(8, -1, &[])]).unwrap_err(),
            EntityDataEncodeError::NegativeSerializerId { serializer_id: -1 }
        );
    }
}
