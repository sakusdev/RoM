use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BIOMES_PER_SECTION, BLOCKS_PER_SECTION, BiomeId, BlockStateId, ChunkPos, ChunkSection,
    ChunkStore, StaticChunk,
};

pub const WORLD_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub schema_version: u32,
    pub chunks: Vec<ChunkSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSnapshot {
    pub x: i32,
    pub z: i32,
    pub min_section_y: i32,
    pub sections: Vec<ChunkSectionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSectionSnapshot {
    pub air: u32,
    pub blocks: Vec<u32>,
    pub biomes: Vec<u32>,
    pub fluid_count: u16,
}

impl WorldSnapshot {
    #[must_use]
    pub fn capture(store: &ChunkStore) -> Self {
        let chunks = store
            .chunks
            .values()
            .map(|chunk| ChunkSnapshot {
                x: chunk.pos.x,
                z: chunk.pos.z,
                min_section_y: chunk.min_section_y,
                sections: chunk
                    .sections
                    .iter()
                    .map(|section| ChunkSectionSnapshot {
                        air: section.air.get(),
                        blocks: section.blocks.iter().map(|state| state.get()).collect(),
                        biomes: section.biomes.iter().map(|biome| biome.get()).collect(),
                        fluid_count: section.fluid_count,
                    })
                    .collect(),
            })
            .collect();
        Self {
            schema_version: WORLD_SNAPSHOT_SCHEMA_VERSION,
            chunks,
        }
    }

    pub fn restore(self) -> Result<ChunkStore, WorldPersistenceError> {
        if self.schema_version != WORLD_SNAPSHOT_SCHEMA_VERSION {
            return Err(WorldPersistenceError::UnsupportedSchema {
                actual: self.schema_version,
                expected: WORLD_SNAPSHOT_SCHEMA_VERSION,
            });
        }

        let mut positions = BTreeSet::new();
        let mut store = ChunkStore::new();
        for chunk in self.chunks {
            let position = ChunkPos {
                x: chunk.x,
                z: chunk.z,
            };
            if !positions.insert(position) {
                return Err(WorldPersistenceError::DuplicateChunk { position });
            }
            if chunk.sections.is_empty() {
                return Err(WorldPersistenceError::EmptyChunk { position });
            }

            let mut sections = Vec::with_capacity(chunk.sections.len());
            for (section_index, section) in chunk.sections.into_iter().enumerate() {
                if section.blocks.len() != BLOCKS_PER_SECTION {
                    return Err(WorldPersistenceError::InvalidBlockCount {
                        position,
                        section_index,
                        actual: section.blocks.len(),
                        expected: BLOCKS_PER_SECTION,
                    });
                }
                if section.biomes.len() != BIOMES_PER_SECTION {
                    return Err(WorldPersistenceError::InvalidBiomeCount {
                        position,
                        section_index,
                        actual: section.biomes.len(),
                        expected: BIOMES_PER_SECTION,
                    });
                }

                let air = BlockStateId::new(section.air);
                let blocks = section
                    .blocks
                    .into_iter()
                    .map(BlockStateId::new)
                    .collect::<Vec<_>>();
                let non_empty = blocks.iter().filter(|state| **state != air).count();
                let non_empty_block_count = u16::try_from(non_empty).map_err(|_| {
                    WorldPersistenceError::NonEmptyBlockCountOverflow {
                        position,
                        section_index,
                        actual: non_empty,
                    }
                })?;
                if usize::from(section.fluid_count) > BLOCKS_PER_SECTION {
                    return Err(WorldPersistenceError::InvalidFluidCount {
                        position,
                        section_index,
                        fluid_count: section.fluid_count,
                        non_empty_block_count,
                    });
                }
                sections.push(ChunkSection {
                    air,
                    blocks,
                    biomes: section.biomes.into_iter().map(BiomeId::new).collect(),
                    non_empty_block_count,
                    fluid_count: section.fluid_count,
                });
            }

            store.insert(StaticChunk {
                pos: position,
                min_section_y: chunk.min_section_y,
                sections,
            });
        }
        Ok(store)
    }

    pub fn to_json_pretty(&self) -> Result<String, WorldPersistenceError> {
        serde_json::to_string_pretty(self).map_err(WorldPersistenceError::Serialize)
    }

    pub fn from_json(input: &str) -> Result<Self, WorldPersistenceError> {
        serde_json::from_str(input).map_err(WorldPersistenceError::Deserialize)
    }
}

impl ChunkStore {
    #[must_use]
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot::capture(self)
    }

    pub fn restore(snapshot: WorldSnapshot) -> Result<Self, WorldPersistenceError> {
        snapshot.restore()
    }
}

#[derive(Debug, Error)]
pub enum WorldPersistenceError {
    #[error("world snapshot schema {actual} is unsupported; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("world snapshot contains duplicate chunk ({}, {})", position.x, position.z)]
    DuplicateChunk { position: ChunkPos },
    #[error("world snapshot chunk ({}, {}) has no sections", position.x, position.z)]
    EmptyChunk { position: ChunkPos },
    #[error(
        "world snapshot chunk ({}, {}) section {section_index} has {actual} block states; expected {expected}",
        position.x,
        position.z
    )]
    InvalidBlockCount {
        position: ChunkPos,
        section_index: usize,
        actual: usize,
        expected: usize,
    },
    #[error(
        "world snapshot chunk ({}, {}) section {section_index} has {actual} biomes; expected {expected}",
        position.x,
        position.z
    )]
    InvalidBiomeCount {
        position: ChunkPos,
        section_index: usize,
        actual: usize,
        expected: usize,
    },
    #[error(
        "world snapshot chunk ({}, {}) section {section_index} has {actual} non-air blocks, exceeding u16",
        position.x,
        position.z
    )]
    NonEmptyBlockCountOverflow {
        position: ChunkPos,
        section_index: usize,
        actual: usize,
    },
    #[error(
        "world snapshot chunk ({}, {}) section {section_index} has fluid count {fluid_count} greater than non-empty count {non_empty_block_count}",
        position.x,
        position.z
    )]
    InvalidFluidCount {
        position: ChunkPos,
        section_index: usize,
        fluid_count: u16,
        non_empty_block_count: u16,
    },
    #[error("cannot serialize world snapshot")]
    Serialize(#[source] serde_json::Error),
    #[error("cannot deserialize world snapshot")]
    Deserialize(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockMutation, BlockPos, FlatWorldSpec};

    fn store() -> ChunkStore {
        let air = BlockStateId::new(0);
        let biome = BiomeId::new(1);
        let mut chunk = StaticChunk::flat_overworld(
            ChunkPos { x: 0, z: 0 },
            -4,
            24,
            FlatWorldSpec {
                floor_y: 64,
                air,
                bedrock: BlockStateId::new(2),
                stone: BlockStateId::new(3),
                dirt: BlockStateId::new(4),
                grass: BlockStateId::new(5),
                biome,
            },
        )
        .unwrap();
        chunk
            .apply_block_mutation(BlockMutation {
                position: BlockPos { x: 1, y: 65, z: 2 },
                state: BlockStateId::new(3),
            })
            .unwrap();
        let mut store = ChunkStore::new();
        store.insert(chunk);
        store
    }

    #[test]
    fn round_trips_chunk_store_through_json() {
        let original = store();
        let json = original.snapshot().to_json_pretty().unwrap();
        let restored = ChunkStore::restore(WorldSnapshot::from_json(&json).unwrap()).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn rejects_corrupted_section_lengths() {
        let mut snapshot = store().snapshot();
        snapshot.chunks[0].sections[0].blocks.pop();
        assert!(matches!(
            snapshot.restore(),
            Err(WorldPersistenceError::InvalidBlockCount { .. })
        ));
    }

    #[test]
    fn rejects_duplicate_chunk_positions() {
        let mut snapshot = store().snapshot();
        snapshot.chunks.push(snapshot.chunks[0].clone());
        assert!(matches!(
            snapshot.restore(),
            Err(WorldPersistenceError::DuplicateChunk { .. })
        ));
    }
}
