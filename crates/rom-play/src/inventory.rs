use std::collections::{BTreeMap, BTreeSet};

use rom_game::{
    ContainerClick, ContainerClickKind, EntityId, EquipmentSlot, ItemStack, MAX_CONTAINER_SLOTS,
    PLAYER_INVENTORY_SLOTS,
};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemProtocolRegistry {
    ids: BTreeMap<String, i32>,
    names: BTreeMap<i32, String>,
}

impl ItemProtocolRegistry {
    pub fn new<I, S>(entries: I) -> Result<Self, InventoryEncodeError>
    where
        I: IntoIterator<Item = (S, i32)>,
        S: Into<String>,
    {
        let mut ids = BTreeMap::new();
        let mut names = BTreeMap::new();
        for (item, protocol_id) in entries {
            let item = item.into();
            validate_registry_entry(&item, protocol_id)?;
            if ids.insert(item.clone(), protocol_id).is_some() {
                return Err(InventoryEncodeError::DuplicateItemId { item });
            }
            if names.insert(protocol_id, item).is_some() {
                return Err(InventoryEncodeError::DuplicateItemProtocolId { protocol_id });
            }
        }
        Ok(Self { ids, names })
    }

    #[must_use]
    pub fn protocol_id(&self, item: &str) -> Option<i32> {
        self.ids.get(item).copied()
    }

    #[must_use]
    pub fn item_name(&self, protocol_id: i32) -> Option<&str> {
        self.names.get(&protocol_id).map(String::as_str)
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataComponentProtocolRegistry {
    ids: BTreeMap<String, i32>,
}

impl DataComponentProtocolRegistry {
    pub fn new<I, S>(entries: I) -> Result<Self, InventoryEncodeError>
    where
        I: IntoIterator<Item = (S, i32)>,
        S: Into<String>,
    {
        let mut ids = BTreeMap::new();
        let mut protocol_ids = BTreeSet::new();
        for (component, protocol_id) in entries {
            let component = component.into();
            validate_registry_entry(&component, protocol_id)?;
            if ids.insert(component.clone(), protocol_id).is_some() {
                return Err(InventoryEncodeError::DuplicateComponentId { component });
            }
            if !protocol_ids.insert(protocol_id) {
                return Err(InventoryEncodeError::DuplicateComponentProtocolId { protocol_id });
            }
        }
        Ok(Self { ids })
    }

    #[must_use]
    pub fn protocol_id(&self, component: &str) -> Option<i32> {
        self.ids.get(component).copied()
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

#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentEntry {
    pub slot: EquipmentSlot,
    pub stack: Option<ItemStack>,
}

impl EquipmentEntry {
    #[must_use]
    pub const fn new(slot: EquipmentSlot, stack: Option<ItemStack>) -> Self {
        Self { slot, stack }
    }
}

pub fn encode_set_equipment(
    entity_id: EntityId,
    entries: &[EquipmentEntry],
    items: &ItemProtocolRegistry,
    components: &DataComponentProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    if entries.is_empty() {
        return Err(InventoryEncodeError::EmptyEquipmentEntries);
    }
    let entity_id =
        i32::try_from(entity_id.get()).map_err(|_| InventoryEncodeError::EntityIdOutOfRange {
            entity_id: entity_id.get(),
        })?;
    let mut output = Vec::new();
    write_varint(&mut output, entity_id);
    let mut seen = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let protocol_slot = equipment_protocol_slot(entry.slot);
        if !seen.insert(protocol_slot) {
            return Err(InventoryEncodeError::DuplicateEquipmentSlot { slot: entry.slot });
        }
        let continuation = if index + 1 < entries.len() { 0x80 } else { 0 };
        output.push(protocol_slot | continuation);
        if !encode_item_stack_into(&mut output, entry.stack.as_ref(), items, components)? {
            return Ok(None);
        }
    }
    Ok(Some(output))
}

const fn equipment_protocol_slot(slot: EquipmentSlot) -> u8 {
    match slot {
        EquipmentSlot::MainHand => 0,
        EquipmentSlot::OffHand => 1,
        EquipmentSlot::Feet => 2,
        EquipmentSlot::Legs => 3,
        EquipmentSlot::Chest => 4,
        EquipmentSlot::Head => 5,
    }
}

fn validate_registry_entry(name: &str, protocol_id: i32) -> Result<(), InventoryEncodeError> {
    if name.trim().is_empty() {
        return Err(InventoryEncodeError::EmptyRegistryId);
    }
    if protocol_id < 0 {
        return Err(InventoryEncodeError::NegativeProtocolId {
            name: name.to_owned(),
            protocol_id,
        });
    }
    Ok(())
}

pub fn encode_item_stack(
    stack: Option<&ItemStack>,
    items: &ItemProtocolRegistry,
    components: &DataComponentProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    let mut output = Vec::new();
    if !encode_item_stack_into(&mut output, stack, items, components)? {
        return Ok(None);
    }
    Ok(Some(output))
}

pub fn encode_set_player_inventory(
    slot: usize,
    stack: Option<&ItemStack>,
    items: &ItemProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    encode_set_player_inventory_with_components(
        slot,
        stack,
        items,
        &DataComponentProtocolRegistry::default(),
    )
}

pub fn encode_set_player_inventory_with_components(
    slot: usize,
    stack: Option<&ItemStack>,
    items: &ItemProtocolRegistry,
    components: &DataComponentProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    if slot >= PLAYER_INVENTORY_SLOTS {
        return Err(InventoryEncodeError::SlotOutOfRange { slot });
    }
    let mut output = Vec::new();
    write_varint(&mut output, slot as i32);
    if !encode_item_stack_into(&mut output, stack, items, components)? {
        return Ok(None);
    }
    Ok(Some(output))
}

pub fn encode_set_container_content(
    container_id: i32,
    state_id: i32,
    slots: &[Option<ItemStack>],
    carried: Option<&ItemStack>,
    items: &ItemProtocolRegistry,
    components: &DataComponentProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    if container_id < 0 {
        return Err(InventoryEncodeError::NegativeContainerId { container_id });
    }
    if state_id < 0 {
        return Err(InventoryEncodeError::NegativeStateId { state_id });
    }
    if slots.len() > MAX_CONTAINER_SLOTS {
        return Err(InventoryEncodeError::TooManyContainerSlots {
            actual: slots.len(),
        });
    }
    let mut output = Vec::new();
    write_varint(&mut output, container_id);
    write_varint(&mut output, state_id);
    write_varint(
        &mut output,
        i32::try_from(slots.len()).map_err(|_| InventoryEncodeError::TooManyContainerSlots {
            actual: slots.len(),
        })?,
    );
    for slot in slots {
        if !encode_item_stack_into(&mut output, slot.as_ref(), items, components)? {
            return Ok(None);
        }
    }
    if !encode_item_stack_into(&mut output, carried, items, components)? {
        return Ok(None);
    }
    Ok(Some(output))
}

pub fn encode_set_container_slot(
    container_id: i32,
    state_id: i32,
    slot: i16,
    stack: Option<&ItemStack>,
    items: &ItemProtocolRegistry,
    components: &DataComponentProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    if container_id < -2 {
        return Err(InventoryEncodeError::NegativeContainerId { container_id });
    }
    if state_id < 0 {
        return Err(InventoryEncodeError::NegativeStateId { state_id });
    }
    let mut output = Vec::new();
    write_varint(&mut output, container_id);
    write_varint(&mut output, state_id);
    output.extend_from_slice(&slot.to_be_bytes());
    if !encode_item_stack_into(&mut output, stack, items, components)? {
        return Ok(None);
    }
    Ok(Some(output))
}

fn encode_item_stack_into(
    output: &mut Vec<u8>,
    stack: Option<&ItemStack>,
    items: &ItemProtocolRegistry,
    components: &DataComponentProtocolRegistry,
) -> Result<bool, InventoryEncodeError> {
    let Some(stack) = stack else {
        write_varint(output, 0);
        return Ok(true);
    };
    let Some(protocol_id) = items.protocol_id(stack.item()) else {
        return Ok(false);
    };
    let count =
        i32::try_from(stack.count()).map_err(|_| InventoryEncodeError::StackCountOutOfRange {
            count: stack.count(),
        })?;
    write_varint(output, count);
    write_varint(output, protocol_id);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    for (name, value) in stack.components() {
        let Some(component_id) = components.protocol_id(name) else {
            return Ok(false);
        };
        if value.is_null() {
            removed.push(component_id);
        } else {
            let Some(bytes) = explicit_component_bytes(value)? else {
                return Ok(false);
            };
            added.push((component_id, bytes));
        }
    }
    write_varint(output, usize_to_varint(added.len())?);
    write_varint(output, usize_to_varint(removed.len())?);
    for (component_id, bytes) in added {
        write_varint(output, component_id);
        output.extend_from_slice(&bytes);
    }
    for component_id in removed {
        write_varint(output, component_id);
    }
    Ok(true)
}

fn explicit_component_bytes(value: &Value) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if object.len() != 1 {
        return Ok(None);
    }
    if let Some(value) = object.get("wire_hex") {
        let Some(hex) = value.as_str() else {
            return Err(InventoryEncodeError::InvalidComponentWireValue);
        };
        if hex.len() % 2 != 0 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(InventoryEncodeError::InvalidComponentWireValue);
        }
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        for pair in hex.as_bytes().chunks_exact(2) {
            let pair = std::str::from_utf8(pair).expect("hex is UTF-8");
            bytes.push(u8::from_str_radix(pair, 16).expect("validated hex"));
        }
        return Ok(Some(bytes));
    }
    if let Some(value) = object.get("wire_bytes") {
        let Some(values) = value.as_array() else {
            return Err(InventoryEncodeError::InvalidComponentWireValue);
        };
        let mut bytes = Vec::with_capacity(values.len());
        for value in values {
            let number = value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or(InventoryEncodeError::InvalidComponentWireValue)?;
            bytes.push(number);
        }
        return Ok(Some(bytes));
    }
    if let Some(value) = object.get("varint") {
        let number = value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or(InventoryEncodeError::InvalidComponentWireValue)?;
        let mut bytes = Vec::new();
        write_varint(&mut bytes, number);
        return Ok(Some(bytes));
    }
    if let Some(value) = object.get("string") {
        let Some(value) = value.as_str() else {
            return Err(InventoryEncodeError::InvalidComponentWireValue);
        };
        let mut bytes = Vec::new();
        write_varint(&mut bytes, usize_to_varint(value.len())?);
        bytes.extend_from_slice(value.as_bytes());
        return Ok(Some(bytes));
    }
    if let Some(value) = object.get("bool") {
        let Some(value) = value.as_bool() else {
            return Err(InventoryEncodeError::InvalidComponentWireValue);
        };
        return Ok(Some(vec![u8::from(value)]));
    }
    for (key, width) in [
        ("i32", 4_usize),
        ("i64", 8_usize),
        ("f32", 4_usize),
        ("f64", 8_usize),
    ] {
        if let Some(value) = object.get(key) {
            let bytes = match key {
                "i32" => i32::try_from(
                    value
                        .as_i64()
                        .ok_or(InventoryEncodeError::InvalidComponentWireValue)?,
                )
                .map_err(|_| InventoryEncodeError::InvalidComponentWireValue)?
                .to_be_bytes()
                .to_vec(),
                "i64" => value
                    .as_i64()
                    .ok_or(InventoryEncodeError::InvalidComponentWireValue)?
                    .to_be_bytes()
                    .to_vec(),
                "f32" => (value
                    .as_f64()
                    .ok_or(InventoryEncodeError::InvalidComponentWireValue)?
                    as f32)
                    .to_be_bytes()
                    .to_vec(),
                "f64" => value
                    .as_f64()
                    .ok_or(InventoryEncodeError::InvalidComponentWireValue)?
                    .to_be_bytes()
                    .to_vec(),
                _ => unreachable!(),
            };
            debug_assert_eq!(bytes.len(), width);
            return Ok(Some(bytes));
        }
    }
    Ok(None)
}

pub fn decode_close_container(payload: &[u8]) -> Result<i32, InventoryDecodeError> {
    let mut reader = InventoryReader::new(payload);
    let container_id = reader.read_varint()?;
    reader.finish()?;
    if container_id < 0 {
        return Err(InventoryDecodeError::NegativeContainerId { container_id });
    }
    Ok(container_id)
}

pub fn decode_creative_slot_update(
    payload: &[u8],
    items: &ItemProtocolRegistry,
) -> Result<(i16, Option<ItemStack>), InventoryDecodeError> {
    let mut reader = InventoryReader::new(payload);
    let slot = reader.read_i16()?;
    let stack = reader.read_plain_stack(items)?;
    reader.finish()?;
    Ok((slot, stack))
}

pub fn decode_container_click(
    payload: &[u8],
    items: &ItemProtocolRegistry,
) -> Result<ContainerClick, InventoryDecodeError> {
    let mut reader = InventoryReader::new(payload);
    let container_id = reader.read_varint()?;
    let state_id = reader.read_varint()?;
    let slot = reader.read_i16()?;
    let button = reader.read_i8()?;
    let mode = reader.read_varint()?;
    let changed_count = reader.read_varint()?;
    if changed_count < 0
        || usize::try_from(changed_count)
            .ok()
            .is_none_or(|count| count > MAX_CONTAINER_SLOTS)
    {
        return Err(InventoryDecodeError::InvalidChangedSlotCount {
            count: changed_count,
        });
    }
    let mut client_changed_slots = Vec::with_capacity(changed_count as usize);
    for _ in 0..changed_count {
        client_changed_slots.push((reader.read_i16()?, reader.read_plain_stack(items)?));
    }
    let client_carried = reader.read_plain_stack(items)?;
    reader.finish()?;
    Ok(ContainerClick {
        container_id,
        state_id,
        slot,
        button,
        kind: ContainerClickKind::from_protocol_mode(mode)
            .map_err(|_| InventoryDecodeError::UnknownClickMode { mode })?,
        client_changed_slots,
        client_carried,
    })
}

struct InventoryReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> InventoryReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn read_i8(&mut self) -> Result<i8, InventoryDecodeError> {
        Ok(i8::from_be_bytes([self.read_u8()?]))
    }

    fn read_u8(&mut self) -> Result<u8, InventoryDecodeError> {
        let value = *self
            .bytes
            .get(self.cursor)
            .ok_or(InventoryDecodeError::Truncated)?;
        self.cursor += 1;
        Ok(value)
    }

    fn read_i16(&mut self) -> Result<i16, InventoryDecodeError> {
        let bytes = self.read_exact(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_varint(&mut self) -> Result<i32, InventoryDecodeError> {
        let mut value = 0_u32;
        for shift in (0..35).step_by(7) {
            let byte = self.read_u8()?;
            value |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        Err(InventoryDecodeError::VarIntTooLong)
    }

    fn read_plain_stack(
        &mut self,
        items: &ItemProtocolRegistry,
    ) -> Result<Option<ItemStack>, InventoryDecodeError> {
        let count = self.read_varint()?;
        if count == 0 {
            return Ok(None);
        }
        if count < 0 {
            return Err(InventoryDecodeError::NegativeStackCount { count });
        }
        let item_id = self.read_varint()?;
        let item = items
            .item_name(item_id)
            .ok_or(InventoryDecodeError::UnknownItemProtocolId {
                protocol_id: item_id,
            })?;
        let added = self.read_varint()?;
        let removed = self.read_varint()?;
        if added != 0 || removed != 0 {
            return Err(InventoryDecodeError::UnsupportedInboundComponents { added, removed });
        }
        let count = u32::try_from(count).expect("positive i32 fits u32");
        Ok(Some(
            ItemStack::new(item, count).map_err(InventoryDecodeError::InvalidStack)?,
        ))
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], InventoryDecodeError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(InventoryDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(InventoryDecodeError::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn finish(&self) -> Result<(), InventoryDecodeError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(InventoryDecodeError::TrailingBytes {
                count: self.bytes.len() - self.cursor,
            })
        }
    }
}

fn usize_to_varint(value: usize) -> Result<i32, InventoryEncodeError> {
    i32::try_from(value).map_err(|_| InventoryEncodeError::LengthOutOfRange { value })
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
    #[error("registry ID cannot be empty")]
    EmptyRegistryId,
    #[error("registry entry {name} has negative protocol ID {protocol_id}")]
    NegativeProtocolId { name: String, protocol_id: i32 },
    #[error("duplicate item ID {item}")]
    DuplicateItemId { item: String },
    #[error("duplicate item protocol ID {protocol_id}")]
    DuplicateItemProtocolId { protocol_id: i32 },
    #[error("duplicate data component ID {component}")]
    DuplicateComponentId { component: String },
    #[error("duplicate data component protocol ID {protocol_id}")]
    DuplicateComponentProtocolId { protocol_id: i32 },
    #[error("player inventory slot {slot} is outside 0..{PLAYER_INVENTORY_SLOTS}")]
    SlotOutOfRange { slot: usize },
    #[error("equipment update must contain at least one entry")]
    EmptyEquipmentEntries,
    #[error("duplicate equipment slot {slot:?}")]
    DuplicateEquipmentSlot { slot: EquipmentSlot },
    #[error("entity ID {entity_id} exceeds the protocol VarInt range")]
    EntityIdOutOfRange { entity_id: u32 },
    #[error("item stack count {count} exceeds the protocol VarInt range")]
    StackCountOutOfRange { count: u32 },
    #[error("container ID {container_id} is invalid")]
    NegativeContainerId { container_id: i32 },
    #[error("container state ID {state_id} is invalid")]
    NegativeStateId { state_id: i32 },
    #[error("container has {actual} slots; limit is {MAX_CONTAINER_SLOTS}")]
    TooManyContainerSlots { actual: usize },
    #[error("length {value} exceeds the VarInt range")]
    LengthOutOfRange { value: usize },
    #[error("invalid explicit component wire value")]
    InvalidComponentWireValue,
}

#[derive(Debug, Error)]
pub enum InventoryDecodeError {
    #[error("inventory payload is truncated")]
    Truncated,
    #[error("inventory VarInt is too long")]
    VarIntTooLong,
    #[error("inventory payload has {count} trailing bytes")]
    TrailingBytes { count: usize },
    #[error("container ID {container_id} cannot be negative")]
    NegativeContainerId { container_id: i32 },
    #[error("changed-slot count {count} is invalid")]
    InvalidChangedSlotCount { count: i32 },
    #[error("unknown inventory click mode {mode}")]
    UnknownClickMode { mode: i32 },
    #[error("item stack count {count} cannot be negative")]
    NegativeStackCount { count: i32 },
    #[error("unknown item protocol ID {protocol_id}")]
    UnknownItemProtocolId { protocol_id: i32 },
    #[error(
        "inbound item components require generated stream decoders: added={added}, removed={removed}"
    )]
    UnsupportedInboundComponents { added: i32, removed: i32 },
    #[error(transparent)]
    InvalidStack(#[from] rom_game::InventoryError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn items() -> ItemProtocolRegistry {
        ItemProtocolRegistry::new([
            ("minecraft:air", 0),
            ("minecraft:stone", 1),
            ("minecraft:diamond", 42),
        ])
        .unwrap()
    }

    #[test]
    fn encodes_empty_plain_and_explicit_component_stacks() {
        assert_eq!(
            encode_set_player_inventory(45, None, &items()).unwrap(),
            Some(vec![45, 0])
        );
        let stack = ItemStack::new("minecraft:stone", 64).unwrap();
        assert_eq!(
            encode_set_player_inventory(9, Some(&stack), &items()).unwrap(),
            Some(vec![9, 64, 1, 0, 0])
        );
        let components = DataComponentProtocolRegistry::new([("minecraft:damage", 3)]).unwrap();
        let damaged = ItemStack::new("minecraft:stone", 1)
            .unwrap()
            .with_component("minecraft:damage", json!({"varint": 7}));
        assert_eq!(
            encode_item_stack(Some(&damaged), &items(), &components).unwrap(),
            Some(vec![1, 1, 1, 0, 3, 7])
        );
    }

    #[test]
    fn container_content_and_slot_are_bounded() {
        let slots = vec![Some(ItemStack::new("minecraft:stone", 1).unwrap()), None];
        let encoded = encode_set_container_content(
            1,
            2,
            &slots,
            None,
            &items(),
            &DataComponentProtocolRegistry::default(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(encoded, vec![1, 2, 2, 1, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn decodes_click_and_creative_plain_stacks() {
        let click = decode_container_click(&[0, 0, 0, 9, 0, 0, 0, 0], &items()).unwrap();
        assert_eq!(click.container_id, 0);
        assert_eq!(click.slot, 9);
        assert_eq!(click.kind, ContainerClickKind::Pickup);
        let (slot, stack) = decode_creative_slot_update(&[0, 9, 1, 1, 0, 0], &items()).unwrap();
        assert_eq!(slot, 9);
        assert_eq!(stack.unwrap().item(), "minecraft:stone");
    }

    #[test]
    fn rejects_inbound_components_without_generated_decoder() {
        let error = decode_creative_slot_update(&[0, 9, 1, 1, 1, 0], &items()).unwrap_err();
        assert!(matches!(
            error,
            InventoryDecodeError::UnsupportedInboundComponents { .. }
        ));
    }

    #[test]
    fn encodes_equipment_continuation_slots_and_rejects_duplicates() {
        let entity_id = EntityId::new(9).unwrap();
        let stone = ItemStack::new("minecraft:stone", 1).unwrap();
        let entries = [
            EquipmentEntry::new(EquipmentSlot::MainHand, Some(stone.clone())),
            EquipmentEntry::new(EquipmentSlot::Head, None),
        ];
        assert_eq!(
            encode_set_equipment(
                entity_id,
                &entries,
                &items(),
                &DataComponentProtocolRegistry::default(),
            )
            .unwrap(),
            Some(vec![9, 0x80, 1, 1, 0, 0, 5, 0])
        );
        assert_eq!(
            encode_set_equipment(
                entity_id,
                &[
                    EquipmentEntry::new(EquipmentSlot::MainHand, Some(stone.clone())),
                    EquipmentEntry::new(EquipmentSlot::MainHand, Some(stone)),
                ],
                &items(),
                &DataComponentProtocolRegistry::default(),
            )
            .unwrap_err(),
            InventoryEncodeError::DuplicateEquipmentSlot {
                slot: EquipmentSlot::MainHand
            }
        );
        let unknown = ItemStack::new("minecraft:unknown", 1).unwrap();
        assert_eq!(
            encode_set_equipment(
                entity_id,
                &[EquipmentEntry::new(EquipmentSlot::MainHand, Some(unknown))],
                &items(),
                &DataComponentProtocolRegistry::default(),
            )
            .unwrap(),
            None
        );
    }
}
