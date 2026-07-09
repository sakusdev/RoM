//! Anvil region-file reading primitives.
//!
//! This module validates compressed chunk NBT and maps modern section palettes
//! into version-neutral `StaticChunk` values using a caller-provided ID profile.

use std::{
    collections::BTreeMap,
    io::{self, Cursor, Read, Seek, SeekFrom},
};

use ferrum_nbt::{DecodeLimits, NamedTag, NbtError, Tag, decode_named_with_limits};
use flate2::read::{GzDecoder, ZlibDecoder};
use thiserror::Error;

use crate::{
    BIOME_EDGE, BiomeId, BlockStateId, ChunkPos, ChunkStore, SECTION_EDGE, StaticChunk, WorldError,
};

pub const REGION_EDGE_CHUNKS: usize = 32;
pub const REGION_CHUNK_COUNT: usize = REGION_EDGE_CHUNKS * REGION_EDGE_CHUNKS;
pub const SECTOR_BYTES: usize = 4096;
pub const HEADER_BYTES: usize = SECTOR_BYTES * 2;
pub const DEFAULT_MAX_DECOMPRESSED_CHUNK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnvilDecodeLimits {
    pub nbt: DecodeLimits,
    pub max_decompressed_chunk_bytes: usize,
}

impl Default for AnvilDecodeLimits {
    fn default() -> Self {
        Self {
            nbt: DecodeLimits::default(),
            max_decompressed_chunk_bytes: DEFAULT_MAX_DECOMPRESSED_CHUNK_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionChunkLocation {
    pub sector_offset: u32,
    pub sector_count: u8,
    pub timestamp: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionHeader {
    locations: [Option<RegionChunkLocation>; REGION_CHUNK_COUNT],
}

impl RegionHeader {
    pub fn read(reader: impl Read) -> Result<Self, AnvilError> {
        let mut bytes = [0_u8; HEADER_BYTES];
        read_exact_with_unexpected_eof(reader, &mut bytes, "region header")?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: [u8; HEADER_BYTES]) -> Result<Self, AnvilError> {
        let mut locations = [None; REGION_CHUNK_COUNT];
        for index in 0..REGION_CHUNK_COUNT {
            let base = index * 4;
            let sector_offset =
                u32::from_be_bytes([0, bytes[base], bytes[base + 1], bytes[base + 2]]);
            let sector_count = bytes[base + 3];
            let timestamp_base = SECTOR_BYTES + base;
            let timestamp = u32::from_be_bytes([
                bytes[timestamp_base],
                bytes[timestamp_base + 1],
                bytes[timestamp_base + 2],
                bytes[timestamp_base + 3],
            ]);
            if sector_offset == 0 && sector_count == 0 {
                continue;
            }
            if sector_offset < 2 {
                return Err(AnvilError::ChunkStartsInsideHeader {
                    local_x: index % REGION_EDGE_CHUNKS,
                    local_z: index / REGION_EDGE_CHUNKS,
                    sector_offset,
                });
            }
            if sector_count == 0 {
                return Err(AnvilError::ZeroSectorCount {
                    local_x: index % REGION_EDGE_CHUNKS,
                    local_z: index / REGION_EDGE_CHUNKS,
                    sector_offset,
                });
            }
            locations[index] = Some(RegionChunkLocation {
                sector_offset,
                sector_count,
                timestamp,
            });
        }
        Ok(Self { locations })
    }

    #[must_use]
    pub fn location(&self, local_x: usize, local_z: usize) -> Option<RegionChunkLocation> {
        self.location_checked(local_x, local_z).ok().flatten()
    }

    pub fn location_checked(
        &self,
        local_x: usize,
        local_z: usize,
    ) -> Result<Option<RegionChunkLocation>, AnvilError> {
        Ok(self.locations[local_index(local_x, local_z)?])
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, RegionChunkLocation)> + '_ {
        self.locations
            .iter()
            .enumerate()
            .filter_map(|(index, location)| {
                location.map(|location| {
                    (
                        index % REGION_EDGE_CHUNKS,
                        index / REGION_EDGE_CHUNKS,
                        location,
                    )
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionPos {
    pub x: i32,
    pub z: i32,
}

impl RegionPos {
    #[must_use]
    pub fn from_chunk(chunk: ChunkPos) -> Self {
        Self {
            x: chunk.x.div_euclid(REGION_EDGE_CHUNKS as i32),
            z: chunk.z.div_euclid(REGION_EDGE_CHUNKS as i32),
        }
    }

    #[must_use]
    pub fn file_name(self) -> String {
        format!("r.{}.{}.mca", self.x, self.z)
    }
}

#[must_use]
pub fn local_chunk_coordinates(chunk: ChunkPos) -> (usize, usize) {
    (
        chunk.x.rem_euclid(REGION_EDGE_CHUNKS as i32) as usize,
        chunk.z.rem_euclid(REGION_EDGE_CHUNKS as i32) as usize,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnvilChunkSummary {
    pub position: ChunkPos,
    pub data_version: Option<i32>,
    pub min_section_y: Option<i32>,
    pub status: Option<String>,
    pub section_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnvilChunkConversionProfile {
    pub air: BlockStateId,
    pub default_biome: BiomeId,
    pub block_states: BTreeMap<String, BlockStateId>,
    pub biomes: BTreeMap<String, BiomeId>,
}

impl AnvilChunkConversionProfile {
    #[must_use]
    pub fn new(air: BlockStateId, default_biome: BiomeId) -> Self {
        Self {
            air,
            default_biome,
            block_states: BTreeMap::new(),
            biomes: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_block_state(mut self, name: impl Into<String>, id: BlockStateId) -> Self {
        self.block_states.insert(name.into(), id);
        self
    }

    #[must_use]
    pub fn with_biome(mut self, name: impl Into<String>, id: BiomeId) -> Self {
        self.biomes.insert(name.into(), id);
        self
    }
}

pub fn summarize_chunk_nbt(root: &NamedTag) -> Result<AnvilChunkSummary, AnvilError> {
    let compound = root_compound(root)?;
    let data_version = optional_int(compound, "DataVersion")?;
    let chunk = compound
        .get("Level")
        .and_then(as_compound)
        .unwrap_or(compound);
    let x = required_int(chunk, "xPos")?;
    let z = required_int(chunk, "zPos")?;
    let min_section_y = optional_int(chunk, "yPos")?;
    let status = match optional_string(chunk, "status")? {
        Some(status) => Some(status),
        None => optional_string(chunk, "Status")?,
    };
    let section_count = match optional_list_len(chunk, "sections")? {
        Some(section_count) => section_count,
        None => optional_list_len(chunk, "Sections")?.unwrap_or(0),
    };

    Ok(AnvilChunkSummary {
        position: ChunkPos { x, z },
        data_version,
        min_section_y,
        status,
        section_count,
    })
}

pub fn chunk_from_anvil_nbt(
    root: &NamedTag,
    profile: &AnvilChunkConversionProfile,
) -> Result<StaticChunk, AnvilError> {
    let compound = root_compound(root)?;
    let chunk = chunk_compound(compound);
    let summary = summarize_chunk_nbt(root)?;
    let sections = match optional_list(chunk, "sections")? {
        Some(sections) => sections,
        None => optional_list(chunk, "Sections")?
            .ok_or(AnvilError::MissingChunkField { field: "sections" })?,
    };
    let section_compounds = section_compounds(sections)?;
    let (min_section_y, max_section_y) = section_y_range(&section_compounds)?;
    let section_count_i32 = max_section_y
        .checked_sub(min_section_y)
        .and_then(|value| value.checked_add(1))
        .ok_or(AnvilError::SectionRangeOverflow)?;
    let section_count =
        usize::try_from(section_count_i32).map_err(|_| AnvilError::SectionRangeOverflow)?;
    let mut output = StaticChunk::new(
        summary.position,
        min_section_y,
        section_count,
        profile.air,
        profile.default_biome,
    )?;

    for section in section_compounds {
        let section_y = required_section_y(section)?;
        if let Some(block_states) = optional_compound(section, "block_states")? {
            apply_block_states(&mut output, section_y, block_states, profile)?;
        }
        if let Some(biomes) = optional_compound(section, "biomes")? {
            apply_biomes(&mut output, section_y, biomes, profile)?;
        }
    }

    Ok(output)
}

pub fn read_chunk_nbt(
    mut reader: impl Read,
    header: &RegionHeader,
    local_x: usize,
    local_z: usize,
    limits: DecodeLimits,
) -> Result<Option<NamedTag>, AnvilError> {
    read_chunk_nbt_with_limits(
        &mut reader,
        header,
        local_x,
        local_z,
        AnvilDecodeLimits {
            nbt: limits,
            ..AnvilDecodeLimits::default()
        },
    )
}

pub fn read_chunk_nbt_with_limits(
    mut reader: impl Read,
    header: &RegionHeader,
    local_x: usize,
    local_z: usize,
    limits: AnvilDecodeLimits,
) -> Result<Option<NamedTag>, AnvilError> {
    let location = match header.location_checked(local_x, local_z)? {
        Some(location) => location,
        None => return Ok(None),
    };
    let sector_offset =
        usize::try_from(location.sector_offset).map_err(|_| AnvilError::SectorOffsetTooLarge {
            local_x,
            local_z,
            sector_offset: location.sector_offset,
        })?;
    let start =
        sector_offset
            .checked_mul(SECTOR_BYTES)
            .ok_or(AnvilError::RegionOffsetOverflow {
                local_x,
                local_z,
                sector_offset: location.sector_offset,
            })?;
    let sector_count = usize::from(location.sector_count);
    let allocation =
        sector_count
            .checked_mul(SECTOR_BYTES)
            .ok_or(AnvilError::RegionOffsetOverflow {
                local_x,
                local_z,
                sector_offset: location.sector_offset,
            })?;

    let mut chunk_sector = vec![0_u8; allocation];
    skip_exact(&mut reader, start, "chunk sector offset")?;
    read_exact_with_unexpected_eof(&mut reader, &mut chunk_sector, "chunk sector")?;

    decode_chunk_sector(&chunk_sector, local_x, local_z, limits).map(Some)
}

pub fn read_chunk_nbt_at(
    reader: &mut (impl Read + Seek),
    header: &RegionHeader,
    local_x: usize,
    local_z: usize,
    limits: AnvilDecodeLimits,
) -> Result<Option<NamedTag>, AnvilError> {
    let location = match header.location_checked(local_x, local_z)? {
        Some(location) => location,
        None => return Ok(None),
    };
    let sector_offset =
        usize::try_from(location.sector_offset).map_err(|_| AnvilError::SectorOffsetTooLarge {
            local_x,
            local_z,
            sector_offset: location.sector_offset,
        })?;
    let start =
        sector_offset
            .checked_mul(SECTOR_BYTES)
            .ok_or(AnvilError::RegionOffsetOverflow {
                local_x,
                local_z,
                sector_offset: location.sector_offset,
            })?;
    let sector_count = usize::from(location.sector_count);
    let allocation =
        sector_count
            .checked_mul(SECTOR_BYTES)
            .ok_or(AnvilError::RegionOffsetOverflow {
                local_x,
                local_z,
                sector_offset: location.sector_offset,
            })?;
    let start = u64::try_from(start).map_err(|_| AnvilError::RegionOffsetOverflow {
        local_x,
        local_z,
        sector_offset: location.sector_offset,
    })?;
    reader.seek(SeekFrom::Start(start))?;
    let mut chunk_sector = vec![0_u8; allocation];
    read_exact_with_unexpected_eof(reader, &mut chunk_sector, "chunk sector")?;
    decode_chunk_sector(&chunk_sector, local_x, local_z, limits).map(Some)
}

pub fn load_chunk_from_region(
    reader: &mut (impl Read + Seek),
    header: &RegionHeader,
    chunk_pos: ChunkPos,
    profile: &AnvilChunkConversionProfile,
    limits: AnvilDecodeLimits,
) -> Result<Option<StaticChunk>, AnvilError> {
    let (local_x, local_z) = local_chunk_coordinates(chunk_pos);
    let Some(root) = read_chunk_nbt_at(reader, header, local_x, local_z, limits)? else {
        return Ok(None);
    };
    let chunk = chunk_from_anvil_nbt(&root, profile)?;
    if chunk.pos() != chunk_pos {
        return Err(AnvilError::ChunkPositionMismatch {
            expected: chunk_pos,
            actual: chunk.pos(),
        });
    }
    Ok(Some(chunk))
}

pub fn load_chunk_store_from_region(
    reader: &mut (impl Read + Seek),
    header: &RegionHeader,
    region: RegionPos,
    profile: &AnvilChunkConversionProfile,
    limits: AnvilDecodeLimits,
) -> Result<ChunkStore, AnvilError> {
    let mut store = ChunkStore::new();
    for (local_x, local_z, _) in header.iter() {
        let chunk_pos = region_chunk_pos(region, local_x, local_z)?;
        if let Some(chunk) = load_chunk_from_region(reader, header, chunk_pos, profile, limits)? {
            store.insert(chunk);
        }
    }
    Ok(store)
}

fn decode_chunk_sector(
    chunk_sector: &[u8],
    local_x: usize,
    local_z: usize,
    limits: AnvilDecodeLimits,
) -> Result<NamedTag, AnvilError> {
    let declared_len = u32::from_be_bytes([
        chunk_sector[0],
        chunk_sector[1],
        chunk_sector[2],
        chunk_sector[3],
    ]);
    if declared_len == 0 {
        return Err(AnvilError::EmptyChunkPayload { local_x, local_z });
    }
    let declared_len =
        usize::try_from(declared_len).map_err(|_| AnvilError::ChunkPayloadTooLarge {
            local_x,
            local_z,
            declared_len,
            sector_capacity: chunk_sector.len() - 4,
        })?;
    if declared_len > chunk_sector.len() - 4 {
        return Err(AnvilError::ChunkPayloadTooLarge {
            local_x,
            local_z,
            declared_len: declared_len as u32,
            sector_capacity: chunk_sector.len() - 4,
        });
    }

    let compression = CompressionType::try_from(chunk_sector[4])?;
    let compressed_payload = &chunk_sector[5..5 + declared_len - 1];
    let nbt_bytes = compression.decode(compressed_payload, limits.max_decompressed_chunk_bytes)?;
    decode_named_with_limits(Cursor::new(nbt_bytes), limits.nbt).map_err(AnvilError::Nbt)
}

fn region_chunk_pos(
    region: RegionPos,
    local_x: usize,
    local_z: usize,
) -> Result<ChunkPos, AnvilError> {
    local_index(local_x, local_z)?;
    let base_x = region
        .x
        .checked_mul(REGION_EDGE_CHUNKS as i32)
        .ok_or(AnvilError::RegionCoordinateOverflow { region })?;
    let base_z = region
        .z
        .checked_mul(REGION_EDGE_CHUNKS as i32)
        .ok_or(AnvilError::RegionCoordinateOverflow { region })?;
    let local_x = i32::try_from(local_x).expect("local chunk x is in 0..32");
    let local_z = i32::try_from(local_z).expect("local chunk z is in 0..32");
    Ok(ChunkPos {
        x: base_x
            .checked_add(local_x)
            .ok_or(AnvilError::RegionCoordinateOverflow { region })?,
        z: base_z
            .checked_add(local_z)
            .ok_or(AnvilError::RegionCoordinateOverflow { region })?,
    })
}

fn local_index(local_x: usize, local_z: usize) -> Result<usize, AnvilError> {
    if local_x >= REGION_EDGE_CHUNKS || local_z >= REGION_EDGE_CHUNKS {
        return Err(AnvilError::LocalChunkOutOfRange { local_x, local_z });
    }
    Ok(local_x + local_z * REGION_EDGE_CHUNKS)
}

fn root_compound(root: &NamedTag) -> Result<&BTreeMap<String, Tag>, AnvilError> {
    as_compound(&root.tag).ok_or(AnvilError::ExpectedCompoundRoot)
}

fn chunk_compound(compound: &BTreeMap<String, Tag>) -> &BTreeMap<String, Tag> {
    compound
        .get("Level")
        .and_then(as_compound)
        .unwrap_or(compound)
}

fn as_compound(tag: &Tag) -> Option<&BTreeMap<String, Tag>> {
    match tag {
        Tag::Compound(compound) => Some(compound),
        _ => None,
    }
}

fn required_int(compound: &BTreeMap<String, Tag>, key: &'static str) -> Result<i32, AnvilError> {
    optional_int(compound, key)?.ok_or(AnvilError::MissingChunkField { field: key })
}

fn optional_int(
    compound: &BTreeMap<String, Tag>,
    key: &'static str,
) -> Result<Option<i32>, AnvilError> {
    match compound.get(key) {
        Some(Tag::Int(value)) => Ok(Some(*value)),
        Some(_) => Err(AnvilError::InvalidChunkFieldType { field: key }),
        None => Ok(None),
    }
}

fn optional_string(
    compound: &BTreeMap<String, Tag>,
    key: &'static str,
) -> Result<Option<String>, AnvilError> {
    match compound.get(key) {
        Some(Tag::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(AnvilError::InvalidChunkFieldType { field: key }),
        None => Ok(None),
    }
}

fn optional_list_len(
    compound: &BTreeMap<String, Tag>,
    key: &'static str,
) -> Result<Option<usize>, AnvilError> {
    match compound.get(key) {
        Some(Tag::List { elements, .. }) => Ok(Some(elements.len())),
        Some(_) => Err(AnvilError::InvalidChunkFieldType { field: key }),
        None => Ok(None),
    }
}

fn optional_list<'a>(
    compound: &'a BTreeMap<String, Tag>,
    key: &'static str,
) -> Result<Option<&'a [Tag]>, AnvilError> {
    match compound.get(key) {
        Some(Tag::List { elements, .. }) => Ok(Some(elements)),
        Some(_) => Err(AnvilError::InvalidChunkFieldType { field: key }),
        None => Ok(None),
    }
}

fn optional_compound<'a>(
    compound: &'a BTreeMap<String, Tag>,
    key: &'static str,
) -> Result<Option<&'a BTreeMap<String, Tag>>, AnvilError> {
    match compound.get(key) {
        Some(Tag::Compound(value)) => Ok(Some(value)),
        Some(_) => Err(AnvilError::InvalidChunkFieldType { field: key }),
        None => Ok(None),
    }
}

fn section_compounds(sections: &[Tag]) -> Result<Vec<&BTreeMap<String, Tag>>, AnvilError> {
    sections
        .iter()
        .map(|section| as_compound(section).ok_or(AnvilError::InvalidSectionEntry))
        .collect()
}

fn section_y_range(sections: &[&BTreeMap<String, Tag>]) -> Result<(i32, i32), AnvilError> {
    let mut iter = sections.iter();
    let first = iter
        .next()
        .ok_or(AnvilError::MissingChunkField { field: "sections" })?;
    let mut min = required_section_y(first)?;
    let mut max = min;
    for section in iter {
        let section_y = required_section_y(section)?;
        min = min.min(section_y);
        max = max.max(section_y);
    }
    Ok((min, max))
}

fn required_section_y(section: &BTreeMap<String, Tag>) -> Result<i32, AnvilError> {
    match section.get("Y").or_else(|| section.get("y")) {
        Some(Tag::Byte(value)) => Ok(i32::from(*value)),
        Some(Tag::Int(value)) => Ok(*value),
        Some(_) => Err(AnvilError::InvalidChunkFieldType { field: "Y" }),
        None => Err(AnvilError::MissingChunkField { field: "Y" }),
    }
}

fn apply_block_states(
    chunk: &mut StaticChunk,
    section_y: i32,
    block_states: &BTreeMap<String, Tag>,
    profile: &AnvilChunkConversionProfile,
) -> Result<(), AnvilError> {
    let palette = required_palette(block_states, "palette")?
        .iter()
        .map(|entry| block_state_palette_entry(entry, profile))
        .collect::<Result<Vec<_>, _>>()?;
    let values = paletted_values(block_states.get("data"), &palette, SECTION_EDGE.pow(3), 4)?;
    for (index, state) in values.into_iter().enumerate() {
        if state == profile.air {
            continue;
        }
        let x = index & 15;
        let y = (index >> 8) & 15;
        let z = (index >> 4) & 15;
        chunk.section_mut(section_y)?.set_block(x, y, z, state)?;
    }
    Ok(())
}

fn apply_biomes(
    chunk: &mut StaticChunk,
    section_y: i32,
    biomes: &BTreeMap<String, Tag>,
    profile: &AnvilChunkConversionProfile,
) -> Result<(), AnvilError> {
    let palette = required_palette(biomes, "palette")?
        .iter()
        .map(|entry| biome_palette_entry(entry, profile))
        .collect::<Result<Vec<_>, _>>()?;
    let values = paletted_values(biomes.get("data"), &palette, BIOME_EDGE.pow(3), 1)?;
    for (index, biome) in values.into_iter().enumerate() {
        let x = index & 3;
        let y = (index >> 4) & 3;
        let z = (index >> 2) & 3;
        chunk.section_mut(section_y)?.set_biome(x, y, z, biome)?;
    }
    Ok(())
}

fn required_palette<'a>(
    container: &'a BTreeMap<String, Tag>,
    key: &'static str,
) -> Result<&'a [Tag], AnvilError> {
    optional_list(container, key)?.ok_or(AnvilError::MissingChunkField { field: key })
}

fn block_state_palette_entry(
    entry: &Tag,
    profile: &AnvilChunkConversionProfile,
) -> Result<BlockStateId, AnvilError> {
    let compound = as_compound(entry).ok_or(AnvilError::InvalidPaletteEntry)?;
    let name = required_string(compound, "Name")?;
    profile
        .block_states
        .get(name)
        .copied()
        .ok_or_else(|| AnvilError::UnknownBlockState { name: name.clone() })
}

fn biome_palette_entry(
    entry: &Tag,
    profile: &AnvilChunkConversionProfile,
) -> Result<BiomeId, AnvilError> {
    let name = match entry {
        Tag::String(name) => name,
        Tag::Compound(compound) => required_string(compound, "Name")?,
        _ => return Err(AnvilError::InvalidPaletteEntry),
    };
    profile
        .biomes
        .get(name)
        .copied()
        .ok_or_else(|| AnvilError::UnknownBiome { name: name.clone() })
}

fn required_string<'a>(
    compound: &'a BTreeMap<String, Tag>,
    key: &'static str,
) -> Result<&'a String, AnvilError> {
    match compound.get(key) {
        Some(Tag::String(value)) => Ok(value),
        Some(_) => Err(AnvilError::InvalidChunkFieldType { field: key }),
        None => Err(AnvilError::MissingChunkField { field: key }),
    }
}

fn paletted_values<T: Copy>(
    data: Option<&Tag>,
    palette: &[T],
    value_count: usize,
    min_bits_per_value: usize,
) -> Result<Vec<T>, AnvilError> {
    if palette.is_empty() {
        return Err(AnvilError::EmptyPalette);
    }
    if palette.len() == 1 {
        return Ok(vec![palette[0]; value_count]);
    }
    let data = match data {
        Some(Tag::LongArray(data)) => data,
        Some(_) => return Err(AnvilError::InvalidChunkFieldType { field: "data" }),
        None => return Err(AnvilError::MissingChunkField { field: "data" }),
    };
    let bits = bits_per_palette_value(palette.len(), min_bits_per_value)?;
    let indexes = unpack_palette_indexes(data, bits, value_count)?;
    indexes
        .into_iter()
        .map(|index| {
            palette
                .get(index)
                .copied()
                .ok_or(AnvilError::PaletteIndexOutOfRange {
                    index,
                    palette_len: palette.len(),
                })
        })
        .collect()
}

fn bits_per_palette_value(
    palette_len: usize,
    min_bits_per_value: usize,
) -> Result<usize, AnvilError> {
    if palette_len <= 1 {
        return Ok(0);
    }
    let bits = usize::BITS as usize - (palette_len - 1).leading_zeros() as usize;
    let bits = bits.max(min_bits_per_value);
    if bits > 31 {
        return Err(AnvilError::PaletteTooLarge { palette_len });
    }
    Ok(bits)
}

fn unpack_palette_indexes(
    data: &[i64],
    bits: usize,
    value_count: usize,
) -> Result<Vec<usize>, AnvilError> {
    if bits == 0 || bits > 31 {
        return Err(AnvilError::PaletteBitsOutOfRange { bits });
    }
    let values_per_long = 64 / bits;
    if values_per_long == 0 {
        return Err(AnvilError::PaletteBitsOutOfRange { bits });
    }
    let expected_longs = value_count.div_ceil(values_per_long);
    if data.len() < expected_longs {
        return Err(AnvilError::PaletteDataTooShort {
            expected_longs,
            actual_longs: data.len(),
        });
    }
    let mask = (1_u64 << bits) - 1;
    let words = data
        .iter()
        .map(|word| u64::from_le_bytes(word.to_le_bytes()))
        .collect::<Vec<_>>();
    let mut values = Vec::with_capacity(value_count);
    for index in 0..value_count {
        let word_index = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        values.push(((words[word_index] >> bit_offset) & mask) as usize);
    }
    Ok(values)
}

fn skip_exact(
    reader: &mut impl Read,
    mut bytes: usize,
    what: &'static str,
) -> Result<(), AnvilError> {
    let mut buffer = [0_u8; 8192];
    while bytes > 0 {
        let read_len = bytes.min(buffer.len());
        read_exact_with_unexpected_eof(&mut *reader, &mut buffer[..read_len], what)?;
        bytes -= read_len;
    }
    Ok(())
}

fn read_exact_with_unexpected_eof(
    mut reader: impl Read,
    buffer: &mut [u8],
    what: &'static str,
) -> Result<(), AnvilError> {
    reader.read_exact(buffer).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            AnvilError::UnexpectedEof { what }
        } else {
            AnvilError::Io(error)
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionType {
    Gzip,
    Zlib,
    Uncompressed,
}

impl CompressionType {
    fn decode(self, bytes: &[u8], max_decompressed_bytes: usize) -> Result<Vec<u8>, AnvilError> {
        if max_decompressed_bytes == 0 {
            return Err(AnvilError::ZeroDecompressedChunkLimit);
        }
        let mut decoded = Vec::new();
        let limit = u64::try_from(max_decompressed_bytes)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        match self {
            Self::Gzip => {
                GzDecoder::new(bytes)
                    .take(limit)
                    .read_to_end(&mut decoded)?;
            }
            Self::Zlib => {
                ZlibDecoder::new(bytes)
                    .take(limit)
                    .read_to_end(&mut decoded)?;
            }
            Self::Uncompressed => {
                let bounded_len = bytes.len().min(max_decompressed_bytes.saturating_add(1));
                decoded.extend_from_slice(&bytes[..bounded_len]);
            }
        }
        if decoded.len() > max_decompressed_bytes {
            return Err(AnvilError::DecompressedChunkTooLarge {
                limit: max_decompressed_bytes,
            });
        }
        Ok(decoded)
    }
}

impl TryFrom<u8> for CompressionType {
    type Error = AnvilError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Gzip),
            2 => Ok(Self::Zlib),
            3 => Ok(Self::Uncompressed),
            other => Err(AnvilError::UnsupportedCompression(other)),
        }
    }
}

#[derive(Debug, Error)]
pub enum AnvilError {
    #[error("I/O error while reading Anvil data: {0}")]
    Io(#[from] io::Error),
    #[error("unexpected end of file while reading {what}")]
    UnexpectedEof { what: &'static str },
    #[error("local chunk coordinate ({local_x}, {local_z}) is outside a 32x32 region")]
    LocalChunkOutOfRange { local_x: usize, local_z: usize },
    #[error(
        "chunk ({local_x}, {local_z}) starts inside the region header at sector {sector_offset}"
    )]
    ChunkStartsInsideHeader {
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
    },
    #[error("chunk ({local_x}, {local_z}) has zero sectors at offset {sector_offset}")]
    ZeroSectorCount {
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
    },
    #[error(
        "chunk ({local_x}, {local_z}) sector offset {sector_offset} is too large for this platform"
    )]
    SectorOffsetTooLarge {
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
    },
    #[error("chunk ({local_x}, {local_z}) sector offset {sector_offset} overflowed")]
    RegionOffsetOverflow {
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
    },
    #[error("chunk ({local_x}, {local_z}) has an empty payload")]
    EmptyChunkPayload { local_x: usize, local_z: usize },
    #[error(
        "chunk ({local_x}, {local_z}) declares {declared_len} bytes but only {sector_capacity} bytes fit in its sectors"
    )]
    ChunkPayloadTooLarge {
        local_x: usize,
        local_z: usize,
        declared_len: u32,
        sector_capacity: usize,
    },
    #[error("unsupported Anvil compression type {0}")]
    UnsupportedCompression(u8),
    #[error("decompressed chunk limit must be greater than zero")]
    ZeroDecompressedChunkLimit,
    #[error("decompressed chunk NBT exceeds configured limit {limit}")]
    DecompressedChunkTooLarge { limit: usize },
    #[error("Anvil chunk root must be a compound")]
    ExpectedCompoundRoot,
    #[error("Anvil chunk is missing required field {field}")]
    MissingChunkField { field: &'static str },
    #[error("Anvil chunk field {field} has an unexpected type")]
    InvalidChunkFieldType { field: &'static str },
    #[error("Anvil section entry must be a compound")]
    InvalidSectionEntry,
    #[error("Anvil palette entry has an unexpected shape")]
    InvalidPaletteEntry,
    #[error("Anvil chunk section range overflowed")]
    SectionRangeOverflow,
    #[error("Anvil palette must not be empty")]
    EmptyPalette,
    #[error("Anvil palette has too many entries: {palette_len}")]
    PaletteTooLarge { palette_len: usize },
    #[error("Anvil palette bits per value is outside the supported range: {bits}")]
    PaletteBitsOutOfRange { bits: usize },
    #[error("Anvil palette data is too short: expected {expected_longs} longs, got {actual_longs}")]
    PaletteDataTooShort {
        expected_longs: usize,
        actual_longs: usize,
    },
    #[error("Anvil palette index {index} is outside palette length {palette_len}")]
    PaletteIndexOutOfRange { index: usize, palette_len: usize },
    #[error("unknown Anvil block state {name}")]
    UnknownBlockState { name: String },
    #[error("unknown Anvil biome {name}")]
    UnknownBiome { name: String },
    #[error("region coordinate ({}, {}) overflowed while mapping chunk positions", region.x, region.z)]
    RegionCoordinateOverflow { region: RegionPos },
    #[error(
        "Anvil chunk position mismatch: expected ({}, {}), got ({}, {})",
        expected.x,
        expected.z,
        actual.x,
        actual.z
    )]
    ChunkPositionMismatch {
        expected: ChunkPos,
        actual: ChunkPos,
    },
    #[error("cannot build world chunk from Anvil data: {0}")]
    World(#[from] WorldError),
    #[error("invalid chunk NBT: {0}")]
    Nbt(#[from] NbtError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use ferrum_nbt::{NamedTag, Tag, encode_named};
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    const AIR: BlockStateId = BlockStateId::new(0);
    const STONE: BlockStateId = BlockStateId::new(1);
    const DIRT: BlockStateId = BlockStateId::new(2);
    const PLAINS: BiomeId = BiomeId::new(40);

    #[test]
    fn parses_region_header_locations_in_minecraft_order() {
        let mut header = [0_u8; HEADER_BYTES];
        write_location(&mut header, 3, 2, 7, 4);
        write_timestamp(&mut header, 3, 2, 123);

        let parsed = RegionHeader::from_bytes(header).unwrap();

        assert_eq!(
            parsed.location(3, 2),
            Some(RegionChunkLocation {
                sector_offset: 7,
                sector_count: 4,
                timestamp: 123,
            })
        );
        assert_eq!(parsed.location(2, 3), None);
    }

    #[test]
    fn rejects_locations_that_overlap_the_region_header() {
        let mut header = [0_u8; HEADER_BYTES];
        write_location(&mut header, 1, 0, 1, 1);

        assert!(matches!(
            RegionHeader::from_bytes(header).unwrap_err(),
            AnvilError::ChunkStartsInsideHeader {
                local_x: 1,
                local_z: 0,
                sector_offset: 1,
            }
        ));
    }

    #[test]
    fn reads_uncompressed_named_chunk_nbt() {
        let root = NamedTag::new("Level", Tag::Compound(Default::default()));
        let region = test_region_with_chunk(5, 6, 2, 1, 3, &encode_root(&root));
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();

        let decoded =
            read_chunk_nbt(Cursor::new(&region), &header, 5, 6, DecodeLimits::default()).unwrap();

        assert_eq!(decoded, Some(root));
    }

    #[test]
    fn returns_none_for_absent_chunks_without_reading_payloads() {
        let region = vec![0_u8; HEADER_BYTES];
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();

        assert!(
            read_chunk_nbt(Cursor::new(&region), &header, 0, 0, DecodeLimits::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rejects_out_of_range_chunk_coordinates() {
        let region = vec![0_u8; HEADER_BYTES];
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();

        assert!(matches!(
            read_chunk_nbt(
                Cursor::new(&region),
                &header,
                32,
                0,
                DecodeLimits::default()
            )
            .unwrap_err(),
            AnvilError::LocalChunkOutOfRange {
                local_x: 32,
                local_z: 0,
            }
        ));
    }

    #[test]
    fn reads_zlib_compressed_named_chunk_nbt() {
        let root = NamedTag::new("Level", Tag::Compound(Default::default()));
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&encode_root(&root)).unwrap();
        let compressed = encoder.finish().unwrap();
        let region = test_region_with_chunk(0, 1, 2, 1, 2, &compressed);
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();

        let decoded =
            read_chunk_nbt(Cursor::new(&region), &header, 0, 1, DecodeLimits::default()).unwrap();

        assert_eq!(decoded, Some(root));
    }

    #[test]
    fn maps_global_chunks_to_region_files_and_local_coordinates() {
        let cases = [
            (
                ChunkPos { x: 0, z: 0 },
                RegionPos { x: 0, z: 0 },
                (0, 0),
                "r.0.0.mca",
            ),
            (
                ChunkPos { x: 31, z: 31 },
                RegionPos { x: 0, z: 0 },
                (31, 31),
                "r.0.0.mca",
            ),
            (
                ChunkPos { x: 32, z: -1 },
                RegionPos { x: 1, z: -1 },
                (0, 31),
                "r.1.-1.mca",
            ),
            (
                ChunkPos { x: -1, z: -32 },
                RegionPos { x: -1, z: -1 },
                (31, 0),
                "r.-1.-1.mca",
            ),
        ];

        for (chunk, region, local, file_name) in cases {
            assert_eq!(RegionPos::from_chunk(chunk), region);
            assert_eq!(local_chunk_coordinates(chunk), local);
            assert_eq!(region.file_name(), file_name);
        }
    }

    #[test]
    fn rejects_chunks_that_exceed_the_decompressed_limit() {
        let root = NamedTag::new(
            "Level",
            Tag::String("x".repeat(DEFAULT_MAX_DECOMPRESSED_CHUNK_BYTES.min(128))),
        );
        let encoded = encode_root(&root);
        let region = test_region_with_chunk(2, 0, 2, 1, 3, &encoded);
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();

        assert!(matches!(
            read_chunk_nbt_with_limits(
                Cursor::new(&region),
                &header,
                2,
                0,
                AnvilDecodeLimits {
                    max_decompressed_chunk_bytes: encoded.len() - 1,
                    ..AnvilDecodeLimits::default()
                },
            )
            .unwrap_err(),
            AnvilError::DecompressedChunkTooLarge { .. }
        ));
    }

    #[test]
    fn summarizes_modern_chunk_nbt_metadata() {
        let mut root = BTreeMap::new();
        root.insert("DataVersion".to_owned(), Tag::Int(4444));
        root.insert("xPos".to_owned(), Tag::Int(-2));
        root.insert("yPos".to_owned(), Tag::Int(-4));
        root.insert("zPos".to_owned(), Tag::Int(3));
        root.insert(
            "status".to_owned(),
            Tag::String("minecraft:full".to_owned()),
        );
        root.insert(
            "sections".to_owned(),
            Tag::List {
                element_type: ferrum_nbt::TagType::Compound,
                elements: vec![
                    Tag::Compound(BTreeMap::new()),
                    Tag::Compound(BTreeMap::new()),
                ],
            },
        );

        assert_eq!(
            summarize_chunk_nbt(&NamedTag::new("", Tag::Compound(root))).unwrap(),
            AnvilChunkSummary {
                position: ChunkPos { x: -2, z: 3 },
                data_version: Some(4444),
                min_section_y: Some(-4),
                status: Some("minecraft:full".to_owned()),
                section_count: 2,
            }
        );
    }

    #[test]
    fn summarizes_legacy_level_wrapped_chunk_nbt_metadata() {
        let mut level = BTreeMap::new();
        level.insert("xPos".to_owned(), Tag::Int(12));
        level.insert("zPos".to_owned(), Tag::Int(-9));
        level.insert("Status".to_owned(), Tag::String("full".to_owned()));

        let mut root = BTreeMap::new();
        root.insert("DataVersion".to_owned(), Tag::Int(1500));
        root.insert("Level".to_owned(), Tag::Compound(level));

        assert_eq!(
            summarize_chunk_nbt(&NamedTag::new("", Tag::Compound(root))).unwrap(),
            AnvilChunkSummary {
                position: ChunkPos { x: 12, z: -9 },
                data_version: Some(1500),
                min_section_y: None,
                status: Some("full".to_owned()),
                section_count: 0,
            }
        );
    }

    #[test]
    fn rejects_chunk_nbt_without_coordinates() {
        let root = NamedTag::new("", Tag::Compound(BTreeMap::new()));

        assert!(matches!(
            summarize_chunk_nbt(&root).unwrap_err(),
            AnvilError::MissingChunkField { field: "xPos" }
        ));
    }

    #[test]
    fn converts_single_palette_anvil_chunk_to_static_chunk() {
        let root = chunk_root_with_sections(vec![section_with_palettes(
            -1,
            block_states_container(vec![block_state_entry("minecraft:stone")], None),
            biome_container(vec![Tag::String("minecraft:plains".to_owned())], None),
        )]);

        let chunk = chunk_from_anvil_nbt(&root, &conversion_profile()).unwrap();

        assert_eq!(chunk.pos(), ChunkPos { x: 2, z: -3 });
        assert_eq!(chunk.min_section_y(), -1);
        assert_eq!(chunk.sections().len(), 1);
        assert_eq!(chunk.block(0, -16, 0).unwrap(), STONE);
        assert_eq!(chunk.block(15, -1, 15).unwrap(), STONE);
        assert_eq!(chunk.section(-1).unwrap().non_empty_block_count(), 4096);
        assert_eq!(chunk.section(-1).unwrap().biome(0, 0, 0).unwrap(), PLAINS);
    }

    #[test]
    fn converts_packed_palette_block_data_to_static_chunk() {
        let mut indexes = vec![0_usize; SECTION_EDGE.pow(3)];
        indexes[1] = 1;
        indexes[256] = 2;
        indexes[16] = 1;
        let root = chunk_root_with_sections(vec![section_with_palettes(
            0,
            block_states_container(
                vec![
                    block_state_entry("minecraft:air"),
                    block_state_entry("minecraft:stone"),
                    block_state_entry("minecraft:dirt"),
                ],
                Some(pack_palette_indexes(&indexes, 4)),
            ),
            biome_container(vec![Tag::String("minecraft:plains".to_owned())], None),
        )]);

        let chunk = chunk_from_anvil_nbt(&root, &conversion_profile()).unwrap();

        assert_eq!(chunk.block(0, 0, 0).unwrap(), AIR);
        assert_eq!(chunk.block(1, 0, 0).unwrap(), STONE);
        assert_eq!(chunk.block(0, 0, 1).unwrap(), STONE);
        assert_eq!(chunk.block(0, 1, 0).unwrap(), DIRT);
        assert_eq!(chunk.section(0).unwrap().non_empty_block_count(), 3);
    }

    #[test]
    fn decodes_non_crossing_palette_longs_at_word_boundaries() {
        let mut indexes = vec![0_usize; SECTION_EDGE.pow(3)];
        indexes[15] = 1;
        indexes[16] = 2;
        let root = chunk_root_with_sections(vec![section_with_palettes(
            0,
            block_states_container(
                vec![
                    block_state_entry("minecraft:air"),
                    block_state_entry("minecraft:stone"),
                    block_state_entry("minecraft:dirt"),
                ],
                Some(pack_palette_indexes(&indexes, 4)),
            ),
            biome_container(vec![Tag::String("minecraft:plains".to_owned())], None),
        )]);

        let chunk = chunk_from_anvil_nbt(&root, &conversion_profile()).unwrap();

        assert_eq!(chunk.block(15, 0, 0).unwrap(), STONE);
        assert_eq!(chunk.block(0, 0, 1).unwrap(), DIRT);
    }

    #[test]
    fn reads_and_converts_chunk_from_seekable_region() {
        let root = chunk_root(
            ChunkPos { x: 2, z: -3 },
            vec![section_with_palettes(
                0,
                block_states_container(vec![block_state_entry("minecraft:stone")], None),
                biome_container(vec![Tag::String("minecraft:plains".to_owned())], None),
            )],
        );
        let region = test_region_with_chunk(2, 29, 2, 1, 3, &encode_root(&root));
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();
        let mut cursor = Cursor::new(region);
        cursor.set_position(123);

        let chunk = load_chunk_from_region(
            &mut cursor,
            &header,
            ChunkPos { x: 2, z: -3 },
            &conversion_profile(),
            AnvilDecodeLimits::default(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(chunk.pos(), ChunkPos { x: 2, z: -3 });
        assert_eq!(chunk.block(0, 0, 0).unwrap(), STONE);
    }

    #[test]
    fn loads_all_region_chunks_into_a_chunk_store() {
        let root = chunk_root(
            ChunkPos { x: 2, z: -3 },
            vec![section_with_palettes(
                0,
                block_states_container(vec![block_state_entry("minecraft:stone")], None),
                biome_container(vec![Tag::String("minecraft:plains".to_owned())], None),
            )],
        );
        let region = test_region_with_chunk(2, 29, 2, 1, 3, &encode_root(&root));
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();
        let mut cursor = Cursor::new(region);

        let store = load_chunk_store_from_region(
            &mut cursor,
            &header,
            RegionPos { x: 0, z: -1 },
            &conversion_profile(),
            AnvilDecodeLimits::default(),
        )
        .unwrap();

        assert_eq!(store.len(), 1);
        assert_eq!(
            store
                .chunk(ChunkPos { x: 2, z: -3 })
                .unwrap()
                .block(0, 0, 0)
                .unwrap(),
            STONE
        );
    }

    #[test]
    fn rejects_region_chunk_position_mismatch() {
        let root = chunk_root(
            ChunkPos { x: 3, z: -3 },
            vec![section_with_palettes(
                0,
                block_states_container(vec![block_state_entry("minecraft:stone")], None),
                biome_container(vec![Tag::String("minecraft:plains".to_owned())], None),
            )],
        );
        let region = test_region_with_chunk(2, 29, 2, 1, 3, &encode_root(&root));
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();
        let mut cursor = Cursor::new(region);

        assert!(matches!(
            load_chunk_from_region(
                &mut cursor,
                &header,
                ChunkPos { x: 2, z: -3 },
                &conversion_profile(),
                AnvilDecodeLimits::default(),
            )
            .unwrap_err(),
            AnvilError::ChunkPositionMismatch {
                expected: ChunkPos { x: 2, z: -3 },
                actual: ChunkPos { x: 3, z: -3 },
            }
        ));
    }

    #[test]
    fn rejects_unknown_block_state_during_chunk_conversion() {
        let root = chunk_root_with_sections(vec![section_with_palettes(
            0,
            block_states_container(vec![block_state_entry("minecraft:diamond_block")], None),
            biome_container(vec![Tag::String("minecraft:plains".to_owned())], None),
        )]);

        assert!(matches!(
            chunk_from_anvil_nbt(&root, &conversion_profile()).unwrap_err(),
            AnvilError::UnknownBlockState { name } if name == "minecraft:diamond_block"
        ));
    }

    #[test]
    fn rejects_chunk_payloads_larger_than_their_sector_allocation() {
        let mut region = vec![0_u8; HEADER_BYTES + SECTOR_BYTES];
        write_location(&mut region[..HEADER_BYTES], 0, 0, 2, 1);
        region[HEADER_BYTES..HEADER_BYTES + 4].copy_from_slice(&4097_u32.to_be_bytes());
        region[HEADER_BYTES + 4] = 3;
        let header = RegionHeader::read(Cursor::new(&region)).unwrap();

        assert!(matches!(
            read_chunk_nbt(Cursor::new(&region), &header, 0, 0, DecodeLimits::default())
                .unwrap_err(),
            AnvilError::ChunkPayloadTooLarge { .. }
        ));
    }

    fn encode_root(root: &NamedTag) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_named(&mut bytes, root).unwrap();
        bytes
    }

    fn conversion_profile() -> AnvilChunkConversionProfile {
        AnvilChunkConversionProfile::new(AIR, PLAINS)
            .with_block_state("minecraft:air", AIR)
            .with_block_state("minecraft:stone", STONE)
            .with_block_state("minecraft:dirt", DIRT)
            .with_biome("minecraft:plains", PLAINS)
    }

    fn chunk_root_with_sections(sections: Vec<Tag>) -> NamedTag {
        chunk_root(ChunkPos { x: 2, z: -3 }, sections)
    }

    fn chunk_root(pos: ChunkPos, sections: Vec<Tag>) -> NamedTag {
        let mut root = BTreeMap::new();
        root.insert("DataVersion".to_owned(), Tag::Int(4444));
        root.insert("xPos".to_owned(), Tag::Int(pos.x));
        root.insert("zPos".to_owned(), Tag::Int(pos.z));
        root.insert(
            "sections".to_owned(),
            Tag::List {
                element_type: ferrum_nbt::TagType::Compound,
                elements: sections,
            },
        );
        NamedTag::new("", Tag::Compound(root))
    }

    fn section_with_palettes(
        section_y: i8,
        block_states: BTreeMap<String, Tag>,
        biomes: BTreeMap<String, Tag>,
    ) -> Tag {
        let mut section = BTreeMap::new();
        section.insert("Y".to_owned(), Tag::Byte(section_y));
        section.insert("block_states".to_owned(), Tag::Compound(block_states));
        section.insert("biomes".to_owned(), Tag::Compound(biomes));
        Tag::Compound(section)
    }

    fn block_states_container(palette: Vec<Tag>, data: Option<Vec<i64>>) -> BTreeMap<String, Tag> {
        let mut container = BTreeMap::new();
        container.insert(
            "palette".to_owned(),
            Tag::List {
                element_type: ferrum_nbt::TagType::Compound,
                elements: palette,
            },
        );
        if let Some(data) = data {
            container.insert("data".to_owned(), Tag::LongArray(data));
        }
        container
    }

    fn biome_container(palette: Vec<Tag>, data: Option<Vec<i64>>) -> BTreeMap<String, Tag> {
        let mut container = BTreeMap::new();
        container.insert(
            "palette".to_owned(),
            Tag::List {
                element_type: ferrum_nbt::TagType::String,
                elements: palette,
            },
        );
        if let Some(data) = data {
            container.insert("data".to_owned(), Tag::LongArray(data));
        }
        container
    }

    fn block_state_entry(name: &str) -> Tag {
        let mut entry = BTreeMap::new();
        entry.insert("Name".to_owned(), Tag::String(name.to_owned()));
        Tag::Compound(entry)
    }

    fn pack_palette_indexes(indexes: &[usize], bits: usize) -> Vec<i64> {
        let values_per_long = 64 / bits;
        let mut words = vec![0_u64; indexes.len().div_ceil(values_per_long)];
        let mask = (1_u64 << bits) - 1;
        for (index, value) in indexes.iter().copied().enumerate() {
            let word_index = index / values_per_long;
            let bit_offset = (index % values_per_long) * bits;
            words[word_index] |= ((value as u64) & mask) << bit_offset;
        }
        words
            .into_iter()
            .map(|word| i64::from_le_bytes(word.to_le_bytes()))
            .collect()
    }

    fn test_region_with_chunk(
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
        sector_count: u8,
        compression: u8,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut region =
            vec![0_u8; (sector_offset as usize + sector_count as usize) * SECTOR_BYTES];
        write_location(
            &mut region[..HEADER_BYTES],
            local_x,
            local_z,
            sector_offset,
            sector_count,
        );
        let start = sector_offset as usize * SECTOR_BYTES;
        let declared_len = u32::try_from(payload.len() + 1).unwrap();
        region[start..start + 4].copy_from_slice(&declared_len.to_be_bytes());
        region[start + 4] = compression;
        region[start + 5..start + 5 + payload.len()].copy_from_slice(payload);
        region
    }

    fn write_location(
        header: &mut [u8],
        local_x: usize,
        local_z: usize,
        sector_offset: u32,
        sector_count: u8,
    ) {
        let index = local_x + local_z * REGION_EDGE_CHUNKS;
        let bytes = sector_offset.to_be_bytes();
        header[index * 4..index * 4 + 3].copy_from_slice(&bytes[1..]);
        header[index * 4 + 3] = sector_count;
    }

    fn write_timestamp(header: &mut [u8], local_x: usize, local_z: usize, timestamp: u32) {
        let index = local_x + local_z * REGION_EDGE_CHUNKS;
        let offset = SECTOR_BYTES + index * 4;
        header[offset..offset + 4].copy_from_slice(&timestamp.to_be_bytes());
    }
}
