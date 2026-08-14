//! Deterministic Minecraft Java Edition Play-state payload codecs.
//!
//! Packet IDs are version metadata and intentionally live outside this crate.

mod block_interaction;
mod chunk_stream;
mod entity;
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

use rom_game::EntityId;
use rom_nbt::{Tag, encode_anonymous};
use rom_world::{BlockStateId, ChunkSection, StaticChunk};
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

    // Heightmap map. The first static-world milestone intentionally sends no
    // heightmap entries; clients can derive visible geometry from chunk data.
    write_varint(&mut output, 0);
    write_len(&mut output, section_data.len())?;
    output.extend_from_slice(&section_data);

    // Block entity list.
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

fn encode_chunk_section(
    output: &mut Vec<u8>,
    section: &ChunkSection,
) -> Result<(), PlayEncodeError> {
    output.extend_from_slice(&section.non_empty_block_count().to_be_bytes());
    output.extend_from_slice(&section.fluid_count().to_be_bytes());

    let block_values: Vec<u32> = section.blocks().iter().map(|id| id.get()).collect();
    encode_paletted_container(
        output,
        &block_values,
        "block state",
        BLOCK_PALETTE_MIN_BITS,
        BLOCK_PALETTE_MAX_INDIRECT_BITS,
    )?;

    let biome_values: Vec<u32> = section.biomes().iter().map(|id| id.get()).collect();
    encode_paletted_container(
        output,
        &biome_values,
        "biome",
        BIOME_PALETTE_MIN_BITS,
        BIOME_PALETTE_MAX_INDIRECT_BITS,
    )?;
    Ok(())
}

fn encode_paletted_container(
    output: &mut Vec<u8>,
    values: &[u32],
    kind: &'static str,
    minimum_indirect_bits: u8,
    maximum_indirect_bits: u8,
) -> Result<(), PlayEncodeError> {
    if values.is_empty() {
        return Err(PlayEncodeError::EmptyPalettedContainer);
    }

    let mut palette = Vec::new();
    let mut palette_indexes = BTreeMap::new();
    let mut indexes = Vec::with_capacity(values.len());
    for value in values {
        let index = if let Some(index) = palette_indexes.get(value) {
            *index
        } else {
            let index =
                u32::try_from(palette.len()).map_err(|_| PlayEncodeError::CollectionTooLong {
                    length: palette.len(),
                })?;
            palette.push(*value);
            palette_indexes.insert(*value, index);
            index
        };
        indexes.push(index);
    }

    if palette.len() == 1 {
        output.push(0);
        write_numeric_id(output, kind, palette[0])?;
        return Ok(());
    }

    let required_bits = ceil_log2(palette.len());
    let bits = required_bits.max(minimum_indirect_bits);
    if bits <= maximum_indirect_bits {
        output.push(bits);
        write_len(output, palette.len())?;
        for value in palette {
            write_numeric_id(output, kind, value)?;
        }
        for packed in pack_values(&indexes, bits)? {
            output.extend_from_slice(&packed.to_be_bytes());
        }
        return Ok(());
    }

    let maximum_value = values.iter().copied().max().unwrap_or(0);
    let global_bits = (u32::BITS - maximum_value.leading_zeros()) as u8;
    let global_bits = global_bits.max(1);
    output.push(global_bits);
    for packed in pack_values(values, global_bits)? {
        output.extend_from_slice(&packed.to_be_bytes());
    }
    Ok(())
}

fn pack_values(values: &[u32], bits: u8) -> Result<Vec<u64>, PlayEncodeError> {
    if bits == 0 || bits > 32 {
        return Err(PlayEncodeError::PaletteBitsOutOfRange { bits });
    }
    let values_per_long = 64 / usize::from(bits);
    let long_count = values.len().div_ceil(values_per_long);
    let mask = (1_u64 << bits) - 1;
    let mut packed = vec![0_u64; long_count];
    for (index, value) in values.iter().copied().enumerate() {
        if u64::from(value) > mask {
            return Err(PlayEncodeError::NumericIdOutOfRange {
                kind: "palette",
                value,
            });
        }
        let long_index = index / values_per_long;
        let bit_index = (index % values_per_long) * usize::from(bits);
        packed[long_index] |= u64::from(value) << bit_index;
    }
    Ok(packed)
}

fn encode_full_sky_light(
    output: &mut Vec<u8>,
    section_count: usize,
) -> Result<(), PlayEncodeError> {
    let light_section_count =
        section_count
            .checked_add(2)
            .ok_or(PlayEncodeError::CollectionTooLong {
                length: section_count,
            })?;
    let all_sections_mask = bitset_with_low_bits(light_section_count);

    write_bitset(output, &all_sections_mask)?;
    write_bitset(output, &[])?;
    write_bitset(output, &[])?;
    write_bitset(output, &all_sections_mask)?;

    write_len(output, light_section_count)?;
    for _ in 0..light_section_count {
        write_len(output, LIGHT_BYTES_PER_SECTION)?;
        output.extend(std::iter::repeat_n(0xff, LIGHT_BYTES_PER_SECTION));
    }
    write_varint(output, 0);
    Ok(())
}

fn bitset_with_low_bits(bit_count: usize) -> Vec<u64> {
    if bit_count == 0 {
        return Vec::new();
    }
    let long_count = bit_count.div_ceil(64);
    let mut longs = vec![u64::MAX; long_count];
    let remainder = bit_count % 64;
    if remainder != 0 {
        longs[long_count - 1] = (1_u64 << remainder) - 1;
    }
    longs
}

fn write_bitset(output: &mut Vec<u8>, values: &[u64]) -> Result<(), PlayEncodeError> {
    write_len(output, values.len())?;
    for value in values {
        output.extend_from_slice(&value.to_be_bytes());
    }
    Ok(())
}

fn encode_component(message: &str) -> Result<Vec<u8>, PlayEncodeError> {
    let mut output = Vec::new();
    encode_anonymous(&mut output, &Tag::String(message.to_owned())).map_err(|error| {
        PlayEncodeError::ComponentEncoding {
            message: error.to_string(),
        }
    })?;
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

fn write_numeric_id(
    output: &mut Vec<u8>,
    kind: &'static str,
    value: u32,
) -> Result<(), PlayEncodeError> {
    let value =
        i32::try_from(value).map_err(|_| PlayEncodeError::NumericIdOutOfRange { kind, value })?;
    write_varint(output, value);
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

fn ceil_log2(value_count: usize) -> u8 {
    let value = value_count.saturating_sub(1);
    (usize::BITS - value.leading_zeros()) as u8
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
    use rom_world::{BiomeId, BlockStateId, ChunkPos, FlatWorldSpec, StaticChunk};

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

    fn static_chunk() -> StaticChunk {
        StaticChunk::flat_overworld(
            ChunkPos { x: 0, z: 0 },
            -4,
            24,
            FlatWorldSpec {
                floor_y: 63,
                air: BlockStateId::new(0),
                bedrock: BlockStateId::new(85),
                stone: BlockStateId::new(1),
                dirt: BlockStateId::new(10),
                grass: BlockStateId::new(9),
                biome: BiomeId::new(40),
            },
        )
        .unwrap()
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
    fn encodes_chunk_batch_and_cache_center_payloads() {
        assert!(encode_chunk_batch_start().is_empty());
        assert_eq!(encode_chunk_batch_finished(1).unwrap(), vec![1]);
        assert_eq!(encode_set_chunk_cache_center(0, 0), vec![0, 0]);
    }

    #[test]
    fn encodes_block_change_ack_exactly() {
        assert_eq!(encode_block_changed_ack(300).unwrap(), vec![0xac, 0x02]);
        assert_eq!(
            encode_block_changed_ack(-1).unwrap_err(),
            PlayEncodeError::NegativeValue {
                field: "block change sequence",
                value: -1,
            }
        );
    }

    #[test]
    fn encodes_block_update_exactly() {
        let payload =
            encode_block_update(BlockPosition { x: 1, y: 65, z: -2 }, BlockStateId::new(300))
                .unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(
            &pack_block_position(BlockPosition { x: 1, y: 65, z: -2 })
                .unwrap()
                .to_be_bytes(),
        );
        expected.extend_from_slice(&[0xac, 0x02]);
        assert_eq!(payload, expected);
    }

    #[test]
    fn encodes_string_components_for_system_chat_and_disconnect() {
        let mut expected = vec![8, 0, 3];
        expected.extend_from_slice(b"RoM");
        assert_eq!(encode_play_disconnect("RoM").unwrap(), expected);
        expected.push(0);
        assert_eq!(encode_system_chat("RoM", false).unwrap(), expected);
    }

    #[test]
    fn encodes_flat_chunk_with_expected_section_layout_and_full_sky_light() {
        let chunk = static_chunk();
        let payload = encode_level_chunk_with_light(&chunk).unwrap();
        assert_eq!(&payload[..8], &[0; 8]);
        assert_eq!(payload[8], 0, "heightmap map must be empty");

        let (section_length, length_bytes) = read_varint(&payload[9..]);
        assert_eq!(section_length, 2_245);
        let section_start = 9 + length_bytes;
        let section_end = section_start + section_length as usize;
        assert_eq!(
            section_end + 1 + 20 + 1 + 26 * (2 + LIGHT_BYTES_PER_SECTION) + 1,
            payload.len()
        );
        assert_eq!(payload[section_end], 0, "block entity list must be empty");

        let first_section = &payload[section_start..];
        assert_eq!(&first_section[..4], &[0, 0, 0, 0]);
        assert_eq!(&first_section[4..8], &[0, 0, 0, 40]);

        let floor_offset = section_start + 7 * 8;
        assert_eq!(&payload[floor_offset..floor_offset + 4], &[4, 0, 0, 0]);
        assert_eq!(payload[floor_offset + 4], 4);
        assert_eq!(payload[floor_offset + 5], 5);
        assert_eq!(
            &payload[floor_offset + 6..floor_offset + 11],
            &[0, 85, 1, 10, 9]
        );
    }

    #[test]
    fn packs_palette_values_without_crossing_long_boundaries() {
        let packed =
            pack_values(&(0_u32..17).map(|value| value % 16).collect::<Vec<_>>(), 4).unwrap();
        assert_eq!(packed.len(), 2);
        assert_eq!(packed[0], 0xfedc_ba98_7654_3210);
        assert_eq!(packed[1], 0);
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

    fn read_varint(input: &[u8]) -> (i32, usize) {
        let mut value = 0_i32;
        for (position, byte) in input.iter().copied().enumerate().take(5) {
            value |= i32::from(byte & 0x7f) << (7 * position);
            if byte & 0x80 == 0 {
                return (value, position + 1);
            }
        }
        panic!("invalid test VarInt")
    }

    #[test]
    fn encodes_hurt_combat_kill_and_respawn_packets() {
        let entity_id = EntityId::new(7).unwrap();
        assert_eq!(
            encode_hurt_animation(entity_id, 90.0).unwrap(),
            [vec![7], 90.0_f32.to_be_bytes().to_vec()].concat()
        );

        let mut expected_kill = vec![7];
        expected_kill.extend_from_slice(&encode_component("Steve died").unwrap());
        assert_eq!(
            encode_player_combat_kill(entity_id, "Steve died").unwrap(),
            expected_kill
        );

        let packet = Respawn {
            spawn_info: CommonPlayerSpawnInfo {
                dimension_type_id: 0,
                dimension: "minecraft:overworld".to_owned(),
                seed: 0,
                game_mode: 0,
                previous_game_mode: 0,
                is_debug: false,
                is_flat: true,
                last_death_location: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            data_to_keep: RespawnDataToKeep::Attributes,
        };
        let payload = encode_respawn(&packet).unwrap();
        assert_eq!(payload.last(), Some(&(RespawnDataToKeep::Attributes as u8)));
        assert!(
            payload
                .windows("minecraft:overworld".len())
                .any(|window| { window == "minecraft:overworld".as_bytes() })
        );
    }
}
