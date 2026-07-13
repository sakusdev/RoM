use std::collections::{BTreeMap, BTreeSet};

use ferrum_game::{ItemStack, PLAYER_INVENTORY_SLOTS};
use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemProtocolRegistry {
    ids: BTreeMap<String, i32>,
}

impl ItemProtocolRegistry {
    pub fn new<I, S>(entries: I) -> Result<Self, InventoryEncodeError>
    where
        I: IntoIterator<Item = (S, i32)>,
        S: Into<String>,
    {
        let mut ids = BTreeMap::new();
        let mut protocol_ids = BTreeSet::new();
        for (item, protocol_id) in entries {
            let item = item.into();
            if item.trim().is_empty() {
                return Err(InventoryEncodeError::EmptyItemId);
            }
            if protocol_id < 0 {
                return Err(InventoryEncodeError::NegativeItemProtocolId { item, protocol_id });
            }
            if ids.insert(item.clone(), protocol_id).is_some() {
                return Err(InventoryEncodeError::DuplicateItemId { item });
            }
            if !protocol_ids.insert(protocol_id) {
                return Err(InventoryEncodeError::DuplicateItemProtocolId { protocol_id });
            }
        }
        Ok(Self { ids })
    }

    #[must_use]
    pub fn protocol_id(&self, item: &str) -> Option<i32> {
        self.ids.get(item).copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }
}

/// Encodes the Minecraft 26.1.2 `set_player_inventory` payload.
///
/// A `None` result means the optional feature must be skipped safely because
/// item IDs are unavailable, the item is absent from the generated palette, or
/// the stack contains components for which no generated stream codec exists.
pub fn encode_set_player_inventory(
    slot: usize,
    stack: Option<&ItemStack>,
    items: &ItemProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    if slot >= PLAYER_INVENTORY_SLOTS {
        return Err(InventoryEncodeError::SlotOutOfRange { slot });
    }
    if items.is_empty() {
        return Ok(None);
    }

    let mut output = Vec::new();
    write_varint(&mut output, slot as i32);
    match stack {
        None => write_varint(&mut output, 0),
        Some(stack) => {
            // Item components use registry-specific stream codecs. Until those
            // codecs are generated, never guess or silently discard them.
            if !stack.components().is_empty() {
                return Ok(None);
            }
            let Some(protocol_id) = items.protocol_id(stack.item()) else {
                return Ok(None);
            };
            let count = i32::try_from(stack.count()).map_err(|_| {
                InventoryEncodeError::StackCountOutOfRange {
                    count: stack.count(),
                }
            })?;
            write_varint(&mut output, count);
            write_varint(&mut output, protocol_id);
            // Added/changed component count, followed by removed component count.
            write_varint(&mut output, 0);
            write_varint(&mut output, 0);
        }
    }
    Ok(Some(output))
}

fn write_varint(output: &mut Vec<u8>, value: i32) {
    let mut remaining = value as u32;
    loop {
        if remaining & !0x7f == 0 {
            output.push(remaining as u8);
            return;
        }
        output.push(((remaining & 0x7f) | 0x80) as u8);
        remaining >>= 7;
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryEncodeError {
    #[error("item ID cannot be empty")]
    EmptyItemId,
    #[error("item {item} has negative protocol ID {protocol_id}")]
    NegativeItemProtocolId { item: String, protocol_id: i32 },
    #[error("duplicate item ID {item}")]
    DuplicateItemId { item: String },
    #[error("duplicate item protocol ID {protocol_id}")]
    DuplicateItemProtocolId { protocol_id: i32 },
    #[error("player inventory slot {slot} is outside 0..{PLAYER_INVENTORY_SLOTS}")]
    SlotOutOfRange { slot: usize },
    #[error("item stack count {count} exceeds the protocol VarInt range")]
    StackCountOutOfRange { count: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> ItemProtocolRegistry {
        ItemProtocolRegistry::new([
            ("minecraft:air", 0),
            ("minecraft:stone", 1),
            ("minecraft:diamond", 42),
        ])
        .unwrap()
    }

    #[test]
    fn encodes_empty_and_plain_item_stacks() {
        assert_eq!(
            encode_set_player_inventory(45, None, &items()).unwrap(),
            Some(vec![45, 0])
        );
        let stack = ItemStack::new("minecraft:stone", 64).unwrap();
        assert_eq!(
            encode_set_player_inventory(9, Some(&stack), &items()).unwrap(),
            Some(vec![9, 64, 1, 0, 0])
        );
    }

    #[test]
    fn skips_when_palette_or_item_id_is_unavailable() {
        let stack = ItemStack::new("minecraft:stone", 1).unwrap();
        assert_eq!(
            encode_set_player_inventory(9, Some(&stack), &ItemProtocolRegistry::default()).unwrap(),
            None
        );
        let unknown = ItemStack::new("minecraft:dirt", 1).unwrap();
        assert_eq!(
            encode_set_player_inventory(9, Some(&unknown), &items()).unwrap(),
            None
        );
    }

    #[test]
    fn validates_palette_and_slot_bounds() {
        assert!(matches!(
            ItemProtocolRegistry::new([("minecraft:stone", 1), ("minecraft:dirt", 1)]),
            Err(InventoryEncodeError::DuplicateItemProtocolId { protocol_id: 1 })
        ));
        assert!(matches!(
            encode_set_player_inventory(PLAYER_INVENTORY_SLOTS, None, &items()),
            Err(InventoryEncodeError::SlotOutOfRange { .. })
        ));
    }
}
