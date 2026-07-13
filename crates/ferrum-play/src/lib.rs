//! Deterministic Minecraft Java Edition Play-state payload codecs.
//!
//! Packet IDs are version metadata and intentionally live outside this crate.

mod block_interaction;
mod chunk_stream;
mod entity;
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
pub use inventory::{
    DataComponentProtocolRegistry, InventoryDecodeError, InventoryEncodeError,
    ItemProtocolRegistry, decode_close_container, decode_container_click,
    decode_creative_slot_update, encode_item_stack, encode_set_container_content,
    encode_set_container_slot, encode_set_player_inventory,
    encode_set_player_inventory_with_components,
};
pub use movement::{
    MAX_PLAYER_COORDINATE, MovementDecodeError, MovementFlags, PlayerMovement, PlayerState,
    decode_move_player_position, decode_move_player_position_rotation, decode_move_player_rotation,
    decode_move_player_status,
};

use std::collections::BTreeMap;

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

#[derive(Debug, Clone, PartialEq)]
pub struct PlayLogin {
    pub entity_id: i32,
    pub hardcore: bool,
    pub dimensions: Vec<String>,
    pub max_players: i32,
    pub view_distance: i32,
    pub simulation_distance: i32,
    pub reduced_debug_info: bool,
    pub show_death_screen: bool,
    pub limited_crafting: bool,
    pub common: CommonPlayerSpawnInfo,
    pub enforces_secure_chat: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalPosition {
    pub dimension: String,
    pub position: [i32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerPosition {
    pub teleport_id: i32,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub relative_flags: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkBatchStart;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkBatchFinished {
    pub batch_size: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetChunkCacheCenter {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetChunkCacheRadius {
    pub radius: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSimulationDistance {
    pub distance: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultSpawnPosition {
    pub position: [i32; 3],
    pub angle: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelChunkWithLight {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub heightmaps: BTreeMap<String, Vec<i64>>,
    pub sections: Vec<ChunkSection>,
    pub block_entities: Vec<Vec<u8>>,
    pub trust_edges: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlayEncodeError {
    #[error("resource location is invalid: {value}")]
    InvalidResourceLocation { value: String },
    #[error("resource location exceeds the protocol length limit")]
    ResourceLocationTooLong,
    #[error("collection length exceeds the VarInt range")]
    CollectionLengthOutOfRange,
    #[error("numeric value exceeds the protocol range")]
    NumericValueOutOfRange,
    #[error("block position is outside the protocol range")]
    BlockPositionOutOfRange,
    #[error("chunk section count is outside the configured range")]
    InvalidSectionCount,
    #[error("chunk section data is invalid")]
    InvalidChunkSection,
    #[error("palette contains too many values")]
    PaletteTooLarge,
    #[error("palette index exceeds the selected storage width")]
    PaletteIndexOutOfRange,
}

pub fn encode_play_login(value: &PlayLogin) -> Result<Vec<u8>, PlayEncodeError> {
    let mut output = Vec::new();
    output.extend_from_slice(&value.entity_id.to_be_bytes());
    output.push(u8::from(value.hardcore));
    write_varint(&mut output, value.dimensions.len())?;
    for dimension in &value.dimensions {
        write_resource_location(&mut output, dimension)?;
    }
    write_varint_i32(&mut output, value.max_players);
    write_varint_i32(&mut output, value.view_distance);
    write_varint_i32(&mut output, value.simulation_distance);
    output.push(u8::from(value.reduced_debug_info));
    output.push(u8::from(value.show_death_screen));
    output.push(u8::from(value.limited_crafting));
    write_common_player_spawn_info(&mut output, &value.common)?;
    output.push(u8::from(value.enforces_secure_chat));
    Ok(output)
}

fn write_common_player_spawn_info(
    output: &mut Vec<u8>,
    value: &CommonPlayerSpawnInfo,
) -> Result<(), PlayEncodeError> {
    write_varint_i32(output, value.dimension_type_id);
    write_resource_location(output, &value.dimension)?;
    output.extend_from_slice(&value.seed.to_be_bytes());
    output.push(value.game_mode as u8);
    output.push(value.previous_game_mode as u8);
    output.push(u8::from(value.is_debug));
    output.push(u8::from(value.is_flat));
    match &value.last_death_location {
        Some(position) => {
            output.push(1);
            write_resource_location(output, &position.dimension)?;
            write_block_position(output, position.position)?;
        }
        None => output.push(0),
    }
    write_varint_i32(output, value.portal_cooldown);
    write_varint_i32(output, value.sea_level);
    Ok(())
}

pub fn encode_player_position(value: &PlayerPosition) -> Vec<u8> {
    let mut output = Vec::new();
    write_varint_i32(&mut output, value.teleport_id);
    for coordinate in value.position {
        output.extend_from_slice(&coordinate.to_be_bytes());
    }
    for velocity in value.velocity {
        output.extend_from_slice(&velocity.to_be_bytes());
    }
    output.extend_from_slice(&value.yaw.to_be_bytes());
    output.extend_from_slice(&value.pitch.to_be_bytes());
    output.extend_from_slice(&value.relative_flags.to_be_bytes());
    output
}

pub fn encode_default_spawn_position(
    value: &DefaultSpawnPosition,
) -> Result<Vec<u8>, PlayEncodeError> {
    let mut output = Vec::new();
    write_block_position(&mut output, value.position)?;
    output.push(value.angle);
    Ok(output)
}

pub fn encode_chunk_batch_start(_: &ChunkBatchStart) -> Vec<u8> {
    Vec::new()
}

pub fn encode_chunk_batch_finished(value: &ChunkBatchFinished) -> Vec<u8> {
    let mut output = Vec::new();
    write_varint_i32(&mut output, value.batch_size);
    output
}

pub fn encode_set_chunk_cache_center(value: &SetChunkCacheCenter) -> Vec<u8> {
    let mut output = Vec::new();
    write_varint_i32(&mut output, value.chunk_x);
    write_varint_i32(&mut output, value.chunk_z);
    output
}

pub fn encode_set_chunk_cache_radius(value: &SetChunkCacheRadius) -> Vec<u8> {
    let mut output = Vec::new();
    write_varint_i32(&mut output, value.radius);
    output
}

pub fn encode_set_simulation_distance(value: &SetSimulationDistance) -> Vec<u8> {
    let mut output = Vec::new();
    write_varint_i32(&mut output, value.distance);
    output
}

pub fn encode_level_chunk_with_light(
    value: &LevelChunkWithLight,
) -> Result<Vec<u8>, PlayEncodeError> {
    if value.sections.is_empty() {
        return Err(PlayEncodeError::InvalidSectionCount);
    }
    let mut output = Vec::new();
    output.extend_from_slice(&value.chunk_x.to_be_bytes());
    output.extend_from_slice(&value.chunk_z.to_be_bytes());

    let heightmaps = Tag::Compound(
        value
            .heightmaps
            .iter()
            .map(|(name, values)| (name.clone(), Tag::LongArray(values.clone())))
            .collect(),
    );
    encode_anonymous(&heightmaps, &mut output);

    let mut section_bytes = Vec::new();
    for section in &value.sections {
        encode_chunk_section(section, &mut section_bytes)?;
    }
    write_varint(&mut output, section_bytes.len())?;
    output.extend_from_slice(&section_bytes);

    write_varint(&mut output, value.block_entities.len())?;
    for block_entity in &value.block_entities {
        output.extend_from_slice(block_entity);
    }

    write_varint_i32(&mut output, 0);
    output.push(u8::from(value.trust_edges));

    let section_light_count = value
        .sections
        .len()
        .checked_add(2)
        .ok_or(PlayEncodeError::CollectionLengthOutOfRange)?;
    let sky_mask = vec![true; section_light_count];
    let block_mask = vec![false; section_light_count];
    let empty_sky_mask = vec![false; section_light_count];
    let empty_block_mask = vec![true; section_light_count];
    write_bitset(&mut output, &sky_mask)?;
    write_bitset(&mut output, &block_mask)?;
    write_bitset(&mut output, &empty_sky_mask)?;
    write_bitset(&mut output, &empty_block_mask)?;

    write_varint(&mut output, section_light_count)?;
    let full_sky_light = vec![0xff; LIGHT_BYTES_PER_SECTION];
    for _ in 0..section_light_count {
        write_varint(&mut output, full_sky_light.len())?;
        output.extend_from_slice(&full_sky_light);
    }
    write_varint_i32(&mut output, 0);
    Ok(output)
}

fn encode_chunk_section(
    section: &ChunkSection,
    output: &mut Vec<u8>,
) -> Result<(), PlayEncodeError> {
    output.extend_from_slice(&section.non_empty_block_count().to_be_bytes());
    encode_paletted_container(
        output,
        section.blocks(),
        BLOCK_PALETTE_MIN_BITS,
        BLOCK_PALETTE_MAX_INDIRECT_BITS,
        |state| state.get(),
    )?;
    encode_paletted_container(
        output,
        section.biomes(),
        BIOME_PALETTE_MIN_BITS,
        BIOME_PALETTE_MAX_INDIRECT_BITS,
        |biome| biome.get(),
    )?;
    Ok(())
}

fn encode_paletted_container<T, F>(
    output: &mut Vec<u8>,
    values: &[T],
    min_indirect_bits: u8,
    max_indirect_bits: u8,
    id: F,
) -> Result<(), PlayEncodeError>
where
    F: Fn(&T) -> u32,
{
    if values.is_empty() {
        return Err(PlayEncodeError::InvalidChunkSection);
    }
    let mut palette = Vec::<u32>::new();
    let mut palette_indices = Vec::<usize>::with_capacity(values.len());
    let mut palette_lookup = BTreeMap::<u32, usize>::new();
    for value in values {
        let value_id = id(value);
        let index = if let Some(index) = palette_lookup.get(&value_id) {
            *index
        } else {
            let index = palette.len();
            palette.push(value_id);
            palette_lookup.insert(value_id, index);
            index
        };
        palette_indices.push(index);
    }

    if palette.len() == 1 {
        output.push(0);
        write_varint_u32(output, palette[0]);
        write_varint_i32(output, 0);
        return Ok(());
    }

    let needed_bits = (usize::BITS - (palette.len() - 1).leading_zeros()) as u8;
    if needed_bits <= max_indirect_bits {
        let bits = needed_bits.max(min_indirect_bits);
        output.push(bits);
        write_varint(output, palette.len())?;
        for value in &palette {
            write_varint_u32(output, *value);
        }
        let values = palette_indices
            .into_iter()
            .map(|index| u64::try_from(index).map_err(|_| PlayEncodeError::PaletteIndexOutOfRange))
            .collect::<Result<Vec<_>, _>>()?;
        write_packed_values(output, &values, bits)?;
    } else {
        let max_id = values.iter().map(&id).max().unwrap_or_default();
        let bits = ((u32::BITS - max_id.leading_zeros()) as u8).max(1);
        output.push(bits);
        let values = values.iter().map(|value| u64::from(id(value))).collect::<Vec<_>>();
        write_packed_values(output, &values, bits)?;
    }
    Ok(())
}

fn write_packed_values(
    output: &mut Vec<u8>,
    values: &[u64],
    bits: u8,
) -> Result<(), PlayEncodeError> {
    let values_per_long = 64_usize / usize::from(bits);
    if values_per_long == 0 {
        return Err(PlayEncodeError::PaletteTooLarge);
    }
    let long_count = values.len().div_ceil(values_per_long);
    write_varint(output, long_count)?;
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    for chunk in values.chunks(values_per_long) {
        let mut packed = 0_u64;
        for (index, value) in chunk.iter().enumerate() {
            if *value > mask {
                return Err(PlayEncodeError::PaletteIndexOutOfRange);
            }
            packed |= value << (index * usize::from(bits));
        }
        output.extend_from_slice(&packed.to_be_bytes());
    }
    Ok(())
}

fn write_bitset(output: &mut Vec<u8>, bits: &[bool]) -> Result<(), PlayEncodeError> {
    let longs = bits.len().div_ceil(64);
    write_varint(output, longs)?;
    for long_index in 0..longs {
        let mut value = 0_u64;
        for bit_index in 0..64 {
            let index = long_index * 64 + bit_index;
            if index < bits.len() && bits[index] {
                value |= 1_u64 << bit_index;
            }
        }
        output.extend_from_slice(&value.to_be_bytes());
    }
    Ok(())
}

fn write_resource_location(
    output: &mut Vec<u8>,
    value: &str,
) -> Result<(), PlayEncodeError> {
    if value.len() > MAX_RESOURCE_LOCATION_BYTES {
        return Err(PlayEncodeError::ResourceLocationTooLong);
    }
    if !is_resource_location(value) {
        return Err(PlayEncodeError::InvalidResourceLocation {
            value: value.to_owned(),
        });
    }
    write_varint(output, value.len())?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_block_position(
    output: &mut Vec<u8>,
    position: [i32; 3],
) -> Result<(), PlayEncodeError> {
    let [x, y, z] = position;
    if !(BLOCK_POS_XZ_MIN..=BLOCK_POS_XZ_MAX).contains(&x)
        || !(BLOCK_POS_XZ_MIN..=BLOCK_POS_XZ_MAX).contains(&z)
        || !(BLOCK_POS_Y_MIN..=BLOCK_POS_Y_MAX).contains(&y)
    {
        return Err(PlayEncodeError::BlockPositionOutOfRange);
    }
    let packed = ((i64::from(x) & 0x3ff_ffff) << 38)
        | ((i64::from(z) & 0x3ff_ffff) << 12)
        | (i64::from(y) & 0xfff);
    output.extend_from_slice(&packed.to_be_bytes());
    Ok(())
}

fn write_varint(output: &mut Vec<u8>, value: usize) -> Result<(), PlayEncodeError> {
    let value = i32::try_from(value).map_err(|_| PlayEncodeError::CollectionLengthOutOfRange)?;
    write_varint_i32(output, value);
    Ok(())
}

fn write_varint_u32(output: &mut Vec<u8>, value: u32) {
    write_varint_i32(output, value as i32);
}

fn write_varint_i32(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            output.push(value as u8);
            break;
        }
        output.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

fn is_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.'))
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'.' | b'/')
        })
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_world::{BiomeId, FlatWorldSpec, StaticChunk};

    #[test]
    fn encodes_player_position_with_absolute_relative_mask() {
        let payload = encode_player_position(&PlayerPosition {
            teleport_id: 2,
            position: [1.0, 65.0, -2.5],
            velocity: [0.0, 0.0, 0.0],
            yaw: 90.0,
            pitch: 45.0,
            relative_flags: 0,
        });
        assert_eq!(payload[0], 2);
        assert_eq!(payload.len(), 57);
    }

    #[test]
    fn rejects_invalid_resource_locations() {
        let error = encode_play_login(&PlayLogin {
            entity_id: 1,
            hardcore: false,
            dimensions: vec!["not namespaced".to_owned()],
            max_players: 20,
            view_distance: 8,
            simulation_distance: 8,
            reduced_debug_info: false,
            show_death_screen: true,
            limited_crafting: false,
            common: CommonPlayerSpawnInfo {
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
            },
            enforces_secure_chat: false,
        })
        .unwrap_err();
        assert!(matches!(
            error,
            PlayEncodeError::InvalidResourceLocation { .. }
        ));
    }

    #[test]
    fn encodes_flat_chunk_sections_and_light() {
        let chunk = StaticChunk::flat_overworld(
            ferrum_world::ChunkPos { x: 0, z: 0 },
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
        .unwrap();
        let payload = encode_level_chunk_with_light(&LevelChunkWithLight {
            chunk_x: 0,
            chunk_z: 0,
            heightmaps: BTreeMap::from([("MOTION_BLOCKING".to_owned(), vec![0; 36])]),
            sections: chunk.sections().to_vec(),
            block_entities: Vec::new(),
            trust_edges: true,
        })
        .unwrap();
        assert!(!payload.is_empty());
    }

    #[test]
    fn encodes_forget_level_chunk_as_fixed_width_coordinates() {
        assert_eq!(
            encode_forget_level_chunk(ferrum_world::ChunkPos { x: 3, z: -7 }),
            [0, 0, 0, 3, 0xff, 0xff, 0xff, 0xf9]
        );
    }
}
