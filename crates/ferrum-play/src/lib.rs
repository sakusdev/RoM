//! Deterministic Minecraft Java Edition Play-state payload codecs.
//!
//! Packet IDs are version metadata and intentionally live outside this crate.

mod block_interaction;
mod chunk_stream;
mod entity;
mod entity_data;
mod generic_entity;
mod health;
mod inventory;
mod movement;

pub use block_interaction::{
    BlockFace, BlockInteractionDecodeError, InteractionHand, PlayerAction, PlayerActionStatus,
    UseItemOnBlock, block_position_to_world, decode_player_action, decode_use_item_on_block,
    player_action_to_world_event, use_item_on_block_to_world_event,
};
pub use chunk_stream::encode_forget_level_chunk;
pub use entity::{
    EncodedEntityMovement, EntityEncodeError, EntityMovementKind, EntityProtocolRegistry,
    PlayerInfoEntry, encode_add_entity, encode_empty_entity_data, encode_entity_movement,
    encode_player_info_remove, encode_player_info_update, encode_remove_entities,
    encode_rotate_head, encode_teleport_entity,
};
pub use entity_data::{
    ENTITY_DATA_TERMINATOR, EntityDataEncodeError, EntityDataEntry, MAX_ENTITY_DATA_ENTRIES,
    MAX_ENTITY_DATA_VALUE_BYTES, encode_entity_data,
};
pub use generic_entity::{GenericEntityEncodeError, encode_add_world_entity};
pub use health::{HealthEncodeError, encode_set_health};
pub use inventory::{
    DataComponentProtocolRegistry, EquipmentEntry, InventoryDecodeError, InventoryEncodeError,
    ItemProtocolRegistry, decode_close_container, decode_container_click,
    decode_creative_slot_update, encode_item_stack, encode_set_container_content,
    encode_set_container_slot, encode_set_equipment, encode_set_player_inventory,
    encode_set_player_inventory_with_components,
};
pub use movement::{
    MAX_PLAYER_COORDINATE, MovementDecodeError, MovementFlags, PlayerMovement, PlayerState,
    decode_move_player_position, decode_move_player_position_rotation, decode_move_player_rotation,
    decode_move_player_status,
};

use std::collections::BTreeMap;

use ferrum_game::EntityId;
use ferrum_nbt::{Tag, encode_anonymous};
use ferrum_world::{BlockStateId, ChunkSection, StaticChunk};
use thiserror::Error;

const MAX_RESOURCE_LOCATION_BYTES: usize = 32_767;
const BLOCK_POS_XZ_MIN: i32 = -33_554_432;
const BLOCK_POS_XZ_MAX: i32 = 33_554_431;
const BLOCK_POS_Y_MIN: i32 = -2_048;
const BLOCK_POS_Y_MAX: i32 = 2_047;
const BLOCK_PALETTE_MIN_BITS: u8 = 4;
const BLOCK_PALETTE_MAX_INDIRECT_BITS: u8 = 8;
const BIOME_PALETTE_MIN_BITS: u8 = 1;
const BIOME_PALETTE_MAX_INDIRECT_BITS: u8 = 3;
const LIGHT_BYTES_PER_SECTION: usize = 2_048;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RespawnDataToKeep {
    Nothing = 0,
    Attributes = 1,
    EntityData = 2,
    All = 3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Respawn {
    pub spawn_info: CommonPlayerSpawnInfo,
    pub data_to_keep: RespawnDataToKeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalPosition {
    pub dimension: String,
    pub position: BlockPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    #[error("{kind} numeric ID {value} exceeds the protocol VarInt range")]
    NumericIdOutOfRange { kind: &'static str, value: u32 },
    #[error("palette requires {bits} bits per value, exceeding the supported maximum")]
    PaletteBitsOutOfRange { bits: u8 },
    #[error("cannot encode an empty paletted container")]
    EmptyPalettedContainer,
    #[error("component encoding failed: {message}")]
    ComponentEncoding { message: String },
}

#[must_use]
pub fn encode_keep_alive(id: i64) -> Vec<u8> {
    id.to_be_bytes().to_vec()
}

#[must_use]
pub fn encode_chunk_batch_start() -> Vec<u8> {
    Vec::new()
}

pub fn encode_chunk_batch_finished(batch_size: i32) -> Result<Vec<u8>, PlayEncodeError> {
    require_non_negative("chunk batch size", batch_size)?;
    let mut output = Vec::new();
    write_varint(&mut output, batch_size);
    Ok(output)
}

#[must_use]
pub fn encode_set_chunk_cache_center(x: i32, z: i32) -> Vec<u8> {
    let mut output = Vec::new();
    write_varint(&mut output, x);
    write_varint(&mut output, z);
    output
}

pub fn encode_hurt_animation(entity_id: EntityId, yaw: f32) -> Result<Vec<u8>, PlayEncodeError> {
    if !yaw.is_finite() {
        return Err(PlayEncodeError::NonFinite { field: "hurt yaw" });
    }
    let mut output = Vec::new();
    write_numeric_id(&mut output, "entity", entity_id.get())?;
    output.extend_from_slice(&yaw.to_be_bytes());
    Ok(output)
}

pub fn encode_player_combat_kill(
    entity_id: EntityId,
    message: &str,
) -> Result<Vec<u8>, PlayEncodeError> {
    let mut output = Vec::new();
    write_numeric_id(&mut output, "entity", entity_id.get())?;
    output.extend_from_slice(&encode_component(message)?);
    Ok(output)
}

pub fn encode_respawn(packet: &Respawn) -> Result<Vec<u8>, PlayEncodeError> {
    require_non_negative("dimension_type_id", packet.spawn_info.dimension_type_id)?;
    require_non_negative("portal_cooldown", packet.spawn_info.portal_cooldown)?;
    let mut output = Vec::new();
    encode_common_spawn_info(&mut output, &packet.spawn_info)?;
    output.push(packet.data_to_keep as u8);
    Ok(output)
}

pub fn encode_system_chat(message: &str, overlay: bool) -> Result<Vec<u8>, PlayEncodeError> {
    let mut output = encode_component(message)?;
    write_bool(&mut output, overlay);
    Ok(output)
}

pub fn encode_play_disconnect(message: &str) -> Result<Vec<u8>, PlayEncodeError> {
    encode_component(message)
}

pub fn encode_block_changed_ack(sequence: i32) -> Result<Vec<u8>, PlayEncodeError> {
    require_non_negative("block change sequence", sequence)?;
    let mut output = Vec::new();
    write_varint(&mut output, sequence);
    Ok(output)
}

pub fn encode_block_update(
    position: BlockPosition,
    state: BlockStateId,
) -> Result<Vec<u8>, PlayEncodeError> {
    let mut output = Vec::new();
    output.extend_from_slice(&pack_block_position(position)?.to_be_bytes());
    write_numeric_id(&mut output, "block state", state.get())?;
    Ok(output)
}

pub fn encode_level_chunk_with_light(chunk: &StaticChunk) -> Result<Vec<u8>, PlayEncodeError> {
    let mut section_data = Vec::new();
    for section in chunk.sections() {
        encode_chunk_section(&mut section_data, section)?;
    }

    let mut output = Vec::new();
    output.extend_from_slice(&chunk.pos().x.to_be_bytes());
    output.extend_from_slice(&chunk.pos().z.to_be_bytes());

    write_varint(&mut output, 0);
    write_len(&mut output, section_data.len())?;
    output.extend_from_slice(&section_data);

    write_varint(&mut output, 0);

    encode_full_sky_light(&mut output, chunk.sections().len())?;
    Ok(output)
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

fn encode_common_spawn_info(
    output: &mut Vec<u8>,
    info: &CommonPlayerSpawnInfo,
) -> Result<(), PlayEncodeError> {
    require_non_negative("dimension_type_id", info.dimension_type_id)?;
    require_non_negative("portal_cooldown", info.portal_cooldown)?;
    write_varint(output, info.dimension_type_id);
    write_resource_location(output, &info.dimension)?;
    output.extend_from_slice(&info.seed.to_be_bytes());
    output.push(info.game_mode as u8);
    output.push(info.previous_game_mode as u8);
    write_bool(output, info.is_debug);
    write_bool(output, info.is_flat);
    encode_optional_global_position(output, info.last_death_location.as_ref())?;
    write_varint(output, info.portal_cooldown);
    write_varint(output, info.sea_level);
    Ok(())
}

fn encode_optional_global_position(
    output: &mut Vec<u8>,
    position: Option<&GlobalPosition>,
) -> Result<(), PlayEncodeError> {
    write_bool(output, position.is_some());
    if let Some(position) = position {
        write_resource_location(output, &position.dimension)?;
        output.extend_from_slice(&pack_block_position(position.position)?.to_be_bytes());
    }
    Ok(())
}

fn write_resource_location(
    output: &mut Vec<u8>,
    resource: &str,
) -> Result<(), PlayEncodeError> {
    if resource.is_empty() {
        return Err(PlayEncodeError::EmptyResourceLocation);
    }
    if resource.len() > MAX_RESOURCE_LOCATION_BYTES {
        return Err(PlayEncodeError::ResourceLocationTooLong {
            length: resource.len(),
        });
    }
    write_string(output, resource)
}

fn write_string(output: &mut Vec<u8>, value: &str) -> Result<(), PlayEncodeError> {
    write_len(output, value.len())?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_len(output: &mut Vec<u8>, length: usize) -> Result<(), PlayEncodeError> {
    let value = i32::try_from(length).map_err(|_| PlayEncodeError::CollectionTooLong { length })?;
    write_varint(output, value);
    Ok(())
}

fn write_numeric_id(
    output: &mut Vec<u8>,
    kind: &'static str,
    value: u32,
) -> Result<(), PlayEncodeError> {
    let value = i32::try_from(value).map_err(|_| PlayEncodeError::NumericIdOutOfRange {
        kind,
        value,
    })?;
    write_varint(output, value);
    Ok(())
}

fn require_non_negative(field: &'static str, value: i32) -> Result<(), PlayEncodeError> {
    if value < 0 {
        return Err(PlayEncodeError::NegativeValue { field, value });
    }
    Ok(())
}

fn write_bool(output: &mut Vec<u8>, value: bool) {
    output.push(u8::from(value));
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
    Ok(((i64::from(position.x) & 0x3ffffff) << 38)
        | ((i64::from(position.z) & 0x3ffffff) << 12)
        | (i64::from(position.y) & 0xfff))
}

fn encode_component(message: &str) -> Result<Vec<u8>, PlayEncodeError> {
    encode_anonymous(&Tag::String(message.to_owned())).map_err(|error| {
        PlayEncodeError::ComponentEncoding {
            message: error.to_string(),
        }
    })
}

fn encode_full_sky_light(
    output: &mut Vec<u8>,
    section_count: usize,
) -> Result<(), PlayEncodeError> {
    let light_section_count = section_count
        .checked_add(2)
        .ok_or(PlayEncodeError::CollectionTooLong {
            length: section_count,
        })?;
    let mask_word = if light_section_count >= 64 {
        u64::MAX
    } else {
        (1_u64 << light_section_count) - 1
    };
    write_varint(output, 1);
    output.extend_from_slice(&mask_word.to_be_bytes());
    write_varint(output, 1);
    output.extend_from_slice(&mask_word.to_be_bytes());
    write_varint(output, 0);
    write_varint(output, 0);
    write_varint(output, usize_to_varint(light_section_count)?);
    for _ in 0..light_section_count {
        write_varint(output, LIGHT_BYTES_PER_SECTION as i32);
        output.extend(std::iter::repeat_n(0xff, LIGHT_BYTES_PER_SECTION));
    }
    write_varint(output, 0);
    Ok(())
}

fn usize_to_varint(value: usize) -> Result<i32, PlayEncodeError> {
    i32::try_from(value).map_err(|_| PlayEncodeError::CollectionTooLong { length: value })
}

fn bits_needed(value: u32) -> u8 {
    if value == 0 {
        0
    } else {
        (u32::BITS - value.leading_zeros()) as u8
    }
}

fn encode_chunk_section(
    output: &mut Vec<u8>,
    section: &ChunkSection,
) -> Result<(), PlayEncodeError> {
    output.extend_from_slice(&section.non_air_count().to_be_bytes());
    encode_block_states(output, section)?;
    encode_biomes(output, section)?;
    Ok(())
}

fn encode_block_states(
    output: &mut Vec<u8>,
    section: &ChunkSection,
) -> Result<(), PlayEncodeError> {
    let mut palette = Vec::new();
    let mut indices = Vec::with_capacity(ChunkSection::BLOCK_COUNT);
    let mut palette_lookup = BTreeMap::new();
    for index in 0..ChunkSection::BLOCK_COUNT {
        let state = section
            .block_state_by_index(index)
            .expect("chunk section index is bounded");
        let palette_index = if let Some(index) = palette_lookup.get(&state) {
            *index
        } else {
            let index = palette.len() as u32;
            palette.push(state);
            palette_lookup.insert(state, index);
            index
        };
        indices.push(palette_index);
    }

    if palette.len() == 1 {
        output.push(0);
        write_numeric_id(output, "block state", palette[0].get())?;
        write_varint(output, 0);
        return Ok(());
    }

    let bits = bits_needed((palette.len() - 1) as u32).max(BLOCK_PALETTE_MIN_BITS);
    if bits <= BLOCK_PALETTE_MAX_INDIRECT_BITS {
        output.push(bits);
        write_len(output, palette.len())?;
        for state in &palette {
            write_numeric_id(output, "block state", state.get())?;
        }
        encode_packed_values(output, &indices, bits)?;
        return Ok(());
    }

    let max_state = section
        .block_states()
        .iter()
        .map(|state| state.get())
        .max()
        .unwrap_or(0);
    let direct_bits = bits_needed(max_state).max(BLOCK_PALETTE_MAX_INDIRECT_BITS + 1);
    if direct_bits > 32 {
        return Err(PlayEncodeError::PaletteBitsOutOfRange { bits: direct_bits });
    }
    output.push(direct_bits);
    let direct_values = section
        .block_states()
        .iter()
        .map(|state| state.get())
        .collect::<Vec<_>>();
    encode_packed_values(output, &direct_values, direct_bits)?;
    Ok(())
}

fn encode_biomes(
    output: &mut Vec<u8>,
    section: &ChunkSection,
) -> Result<(), PlayEncodeError> {
    let mut palette = Vec::new();
    let mut indices = Vec::with_capacity(ChunkSection::BIOME_COUNT);
    let mut palette_lookup = BTreeMap::new();
    for index in 0..ChunkSection::BIOME_COUNT {
        let biome = section
            .biome_by_index(index)
            .expect("chunk section biome index is bounded");
        let palette_index = if let Some(index) = palette_lookup.get(&biome) {
            *index
        } else {
            let index = palette.len() as u32;
            palette.push(biome);
            palette_lookup.insert(biome, index);
            index
        };
        indices.push(palette_index);
    }

    if palette.len() == 1 {
        output.push(0);
        write_numeric_id(output, "biome", palette[0])?;
        write_varint(output, 0);
        return Ok(());
    }

    let bits = bits_needed((palette.len() - 1) as u32).max(BIOME_PALETTE_MIN_BITS);
    if bits <= BIOME_PALETTE_MAX_INDIRECT_BITS {
        output.push(bits);
        write_len(output, palette.len())?;
        for biome in &palette {
            write_numeric_id(output, "biome", *biome)?;
        }
        encode_packed_values(output, &indices, bits)?;
        return Ok(());
    }

    let max_biome = section.biomes().iter().copied().max().unwrap_or(0);
    let direct_bits = bits_needed(max_biome).max(BIOME_PALETTE_MAX_INDIRECT_BITS + 1);
    if direct_bits > 32 {
        return Err(PlayEncodeError::PaletteBitsOutOfRange { bits: direct_bits });
    }
    output.push(direct_bits);
    encode_packed_values(output, section.biomes(), direct_bits)?;
    Ok(())
}

fn encode_packed_values(
    output: &mut Vec<u8>,
    values: &[u32],
    bits_per_value: u8,
) -> Result<(), PlayEncodeError> {
    if bits_per_value == 0 || bits_per_value > 32 {
        return Err(PlayEncodeError::PaletteBitsOutOfRange {
            bits: bits_per_value,
        });
    }
    let values_per_long = 64 / usize::from(bits_per_value);
    if values_per_long == 0 {
        return Err(PlayEncodeError::PaletteBitsOutOfRange {
            bits: bits_per_value,
        });
    }
    let long_count = values.len().div_ceil(values_per_long);
    write_len(output, long_count)?;
    let mask = if bits_per_value == 32 {
        u64::from(u32::MAX)
    } else {
        (1_u64 << bits_per_value) - 1
    };
    for chunk in values.chunks(values_per_long) {
        let mut packed = 0_u64;
        for (index, value) in chunk.iter().enumerate() {
            packed |= (u64::from(*value) & mask) << (index * usize::from(bits_per_value));
        }
        output.extend_from_slice(&packed.to_be_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_game_prefix_uses_profile_numeric_dimension_id() {
        let payload = encode_join_game(&JoinGame {
            player_id: 7,
            hardcore: false,
            levels: vec!["minecraft:overworld".into()],
            max_players: 20,
            chunk_radius: 8,
            simulation_distance: 8,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            spawn_info: CommonPlayerSpawnInfo {
                dimension_type_id: 3,
                dimension: "minecraft:overworld".into(),
                seed: 0,
                game_mode: 0,
                previous_game_mode: -1,
                is_debug: false,
                is_flat: true,
                last_death_location: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            enforces_secure_chat: false,
        })
        .unwrap();
        assert_eq!(&payload[..5], &[0, 0, 0, 7, 0]);
    }

    #[test]
    fn block_position_limits_are_enforced() {
        let position = BlockPosition {
            x: BLOCK_POS_XZ_MAX,
            y: BLOCK_POS_Y_MAX,
            z: BLOCK_POS_XZ_MIN,
        };
        assert!(pack_block_position(position).is_ok());
        assert!(pack_block_position(BlockPosition { x: BLOCK_POS_XZ_MAX + 1, ..position }).is_err());
    }

    #[test]
    fn block_palette_single_value_has_zero_bits() {
        let section = ChunkSection::new(BlockStateId::new(7), 2);
        let mut output = Vec::new();
        encode_block_states(&mut output, &section).unwrap();
        assert_eq!(output, vec![0, 7, 0]);
    }

    #[test]
    fn block_palette_uses_indirect_encoding_for_small_palettes() {
        let mut section = ChunkSection::new(BlockStateId::new(1), 2);
        section
            .set_block_state(0, 0, 0, BlockStateId::new(2))
            .unwrap();
        let mut output = Vec::new();
        encode_block_states(&mut output, &section).unwrap();
        assert_eq!(output[0], BLOCK_PALETTE_MIN_BITS);
        assert_eq!(output[1], 2);
        assert_eq!(output[2], 2);
        assert_eq!(output[3], 1);
    }

    #[test]
    fn chunk_payload_begins_with_chunk_coordinates() {
        let chunk = StaticChunk::new(1, 2, -4, 1, BlockStateId::new(0), 0).unwrap();
        let payload = encode_level_chunk_with_light(&chunk).unwrap();
        assert_eq!(&payload[..4], &1_i32.to_be_bytes());
        assert_eq!(&payload[4..8], &2_i32.to_be_bytes());
    }
}
