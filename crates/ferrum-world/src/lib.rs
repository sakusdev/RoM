//! Deterministic in-memory world primitives shared by Ferrum server subsystems.
//!
//! This crate owns version-neutral coordinates and chunk contents. Minecraft
//! protocol serialization remains in `ferrum-play`, while version-specific
//! numeric IDs remain in version crates.

pub mod anvil;
pub mod block;
pub mod block_entity;
mod chunk_view;
pub mod fluid;
pub mod geometry;
pub mod persistence;
pub mod redstone;
pub mod tick;

pub use block::{
    BlockBehavior, BlockBehaviorError, BlockBehaviorRegistry, BlockDrop, ToolKind, ToolProfile,
    ToolTier,
};
pub use block_entity::{
    BlockEntity, BlockEntityError, BlockEntityStore, MAX_BLOCK_ENTITIES,
    MAX_BLOCK_ENTITY_DATA_BYTES,
};
pub use chunk_view::{ChunkView, ChunkViewDelta, ChunkViewError};
pub use fluid::{FluidError, FluidFlowPlan, FluidKind, FluidState};
pub use geometry::{Aabb, GeometryError, RayHit, VoxelShape, normalized_direction};
pub use persistence::{
    ChunkSectionSnapshot, ChunkSnapshot, WORLD_SNAPSHOT_SCHEMA_VERSION, WorldPersistenceError,
    WorldSnapshot,
};
pub use redstone::{
    DEFAULT_MAX_REDSTONE_NODES, MAX_REDSTONE_POWER, RedstoneError, RedstoneNetwork,
};
pub use tick::{
    DEFAULT_MAX_SCHEDULED_TICKS, MAX_TICKS_PER_DRAIN, RandomTickSelector, ScheduledTick,
    ScheduledTickKey, TickKind, TickPriority, TickScheduler, TickSchedulerError,
};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const SECTION_EDGE: usize = 16;
pub const BIOME_EDGE: usize = 4;
pub const BLOCKS_PER_SECTION: usize = SECTION_EDGE * SECTION_EDGE * SECTION_EDGE;
pub const BIOMES_PER_SECTION: usize = BIOME_EDGE * BIOME_EDGE * BIOME_EDGE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BlockStateId(u32);

impl BlockStateId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BiomeId(u32);

impl BiomeId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockMutation {
    pub position: BlockPos,
    pub state: BlockStateId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppliedBlockMutation {
    pub position: BlockPos,
    pub chunk: ChunkPos,
    pub section_y: i32,
    pub local_x: usize,
    pub local_y: usize,
    pub local_z: usize,
    pub previous: BlockStateId,
    pub current: BlockStateId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldEvent {
    BlockMutation(BlockMutation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedWorldEvent {
    BlockMutation(AppliedBlockMutation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSection {
    air: BlockStateId,
    blocks: Vec<BlockStateId>,
    biomes: Vec<BiomeId>,
    non_empty_block_count: u16,
    fluid_count: u16,
}

impl ChunkSection {
    #[must_use]
    pub fn new(air: BlockStateId, biome: BiomeId) -> Self {
        Self {
            air,
            blocks: vec![air; BLOCKS_PER_SECTION],
            biomes: vec![biome; BIOMES_PER_SECTION],
            non_empty_block_count: 0,
            fluid_count: 0,
        }
    }

    pub fn set_block(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        state: BlockStateId,
    ) -> Result<BlockStateId, WorldError> {
        let index = block_index(x, y, z)?;
        let previous = self.blocks[index];
        if previous == self.air && state != self.air {
            self.non_empty_block_count = self
                .non_empty_block_count
                .checked_add(1)
                .ok_or(WorldError::NonEmptyBlockCountOverflow)?;
        } else if previous != self.air && state == self.air {
            self.non_empty_block_count = self
                .non_empty_block_count
                .checked_sub(1)
                .expect("non-empty count matches section contents");
        }
        self.blocks[index] = state;
        Ok(previous)
    }

    pub fn block(&self, x: usize, y: usize, z: usize) -> Result<BlockStateId, WorldError> {
        Ok(self.blocks[block_index(x, y, z)?])
    }

    pub fn set_biome(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        biome: BiomeId,
    ) -> Result<BiomeId, WorldError> {
        let index = biome_index(x, y, z)?;
        let previous = self.biomes[index];
        self.biomes[index] = biome;
        Ok(previous)
    }

    pub fn biome(&self, x: usize, y: usize, z: usize) -> Result<BiomeId, WorldError> {
        Ok(self.biomes[biome_index(x, y, z)?])
    }

    #[must_use]
    pub const fn non_empty_block_count(&self) -> u16 {
        self.non_empty_block_count
    }

    #[must_use]
    pub const fn fluid_count(&self) -> u16 {
        self.fluid_count
    }

    #[must_use]
    pub fn blocks(&self) -> &[BlockStateId] {
        &self.blocks
    }

    #[must_use]
    pub fn biomes(&self) -> &[BiomeId] {
        &self.biomes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatWorldSpec {
    pub floor_y: i32,
    pub air: BlockStateId,
    pub bedrock: BlockStateId,
    pub stone: BlockStateId,
    pub dirt: BlockStateId,
    pub grass: BlockStateId,
    pub biome: BiomeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticChunk {
    pos: ChunkPos,
    min_section_y: i32,
    sections: Vec<ChunkSection>,
}

impl StaticChunk {
    pub fn new(
        pos: ChunkPos,
        min_section_y: i32,
        section_count: usize,
        air: BlockStateId,
        biome: BiomeId,
    ) -> Result<Self, WorldError> {
        if section_count == 0 {
            return Err(WorldError::EmptyChunk);
        }
        if section_count > i32::MAX as usize {
            return Err(WorldError::TooManySections { section_count });
        }
        Ok(Self {
            pos,
            min_section_y,
            sections: (0..section_count)
                .map(|_| ChunkSection::new(air, biome))
                .collect(),
        })
    }

    pub fn flat_overworld(
        pos: ChunkPos,
        min_section_y: i32,
        section_count: usize,
        spec: FlatWorldSpec,
    ) -> Result<Self, WorldError> {
        let mut chunk = Self::new(pos, min_section_y, section_count, spec.air, spec.biome)?;
        for (world_y, state) in [
            (spec.floor_y - 3, spec.bedrock),
            (spec.floor_y - 2, spec.stone),
            (spec.floor_y - 1, spec.dirt),
            (spec.floor_y, spec.grass),
        ] {
            for z in 0..SECTION_EDGE {
                for x in 0..SECTION_EDGE {
                    chunk.set_block(x, world_y, z, state)?;
                }
            }
        }
        Ok(chunk)
    }

    pub fn set_block(
        &mut self,
        x: usize,
        world_y: i32,
        z: usize,
        state: BlockStateId,
    ) -> Result<BlockStateId, WorldError> {
        if x >= SECTION_EDGE || z >= SECTION_EDGE {
            return Err(WorldError::BlockCoordinateOutOfRange { x, y: 0, z });
        }
        let section_y = world_y.div_euclid(SECTION_EDGE as i32);
        let local_y = world_y.rem_euclid(SECTION_EDGE as i32) as usize;
        self.section_mut(section_y)?.set_block(x, local_y, z, state)
    }

    pub fn block(&self, x: usize, world_y: i32, z: usize) -> Result<BlockStateId, WorldError> {
        if x >= SECTION_EDGE || z >= SECTION_EDGE {
            return Err(WorldError::BlockCoordinateOutOfRange { x, y: 0, z });
        }
        let section_y = world_y.div_euclid(SECTION_EDGE as i32);
        let local_y = world_y.rem_euclid(SECTION_EDGE as i32) as usize;
        self.section(section_y)?.block(x, local_y, z)
    }

    pub fn apply_block_mutation(
        &mut self,
        mutation: BlockMutation,
    ) -> Result<AppliedBlockMutation, WorldError> {
        let (chunk, local_x, local_z) = split_world_block_xz(mutation.position)?;
        if chunk != self.pos {
            return Err(WorldError::BlockOutsideChunk {
                position: mutation.position,
                chunk: self.pos,
            });
        }
        let section_y = mutation.position.y.div_euclid(SECTION_EDGE as i32);
        let local_y = mutation.position.y.rem_euclid(SECTION_EDGE as i32) as usize;
        let previous =
            self.section_mut(section_y)?
                .set_block(local_x, local_y, local_z, mutation.state)?;
        Ok(AppliedBlockMutation {
            position: mutation.position,
            chunk,
            section_y,
            local_x,
            local_y,
            local_z,
            previous,
            current: mutation.state,
        })
    }

    pub fn world_block(&self, position: BlockPos) -> Result<BlockStateId, WorldError> {
        let (chunk, local_x, local_z) = split_world_block_xz(position)?;
        if chunk != self.pos {
            return Err(WorldError::BlockOutsideChunk {
                position,
                chunk: self.pos,
            });
        }
        self.block(local_x, position.y, local_z)
    }

    #[must_use]
    pub const fn pos(&self) -> ChunkPos {
        self.pos
    }

    #[must_use]
    pub const fn min_section_y(&self) -> i32 {
        self.min_section_y
    }

    #[must_use]
    pub fn sections(&self) -> &[ChunkSection] {
        &self.sections
    }

    pub fn section(&self, section_y: i32) -> Result<&ChunkSection, WorldError> {
        let index = self.section_index(section_y)?;
        Ok(&self.sections[index])
    }

    pub fn section_mut(&mut self, section_y: i32) -> Result<&mut ChunkSection, WorldError> {
        let index = self.section_index(section_y)?;
        Ok(&mut self.sections[index])
    }

    fn section_index(&self, section_y: i32) -> Result<usize, WorldError> {
        let relative = section_y
            .checked_sub(self.min_section_y)
            .ok_or(WorldError::SectionCoordinateOverflow)?;
        let index = usize::try_from(relative).map_err(|_| WorldError::SectionOutOfRange {
            section_y,
            min_section_y: self.min_section_y,
            section_count: self.sections.len(),
        })?;
        if index >= self.sections.len() {
            return Err(WorldError::SectionOutOfRange {
                section_y,
                min_section_y: self.min_section_y,
                section_count: self.sections.len(),
            });
        }
        Ok(index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChunkStore {
    chunks: BTreeMap<ChunkPos, StaticChunk>,
}

impl ChunkStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, chunk: StaticChunk) -> Option<StaticChunk> {
        self.chunks.insert(chunk.pos(), chunk)
    }

    #[must_use]
    pub fn chunk(&self, pos: ChunkPos) -> Option<&StaticChunk> {
        self.chunks.get(&pos)
    }

    pub fn chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut StaticChunk> {
        self.chunks.get_mut(&pos)
    }

    pub fn world_block(&self, position: BlockPos) -> Result<BlockStateId, WorldError> {
        let (chunk_pos, _, _) = split_world_block_xz(position)?;
        self.chunks
            .get(&chunk_pos)
            .ok_or(WorldError::MissingChunk { chunk: chunk_pos })?
            .world_block(position)
    }

    pub fn apply_block_mutation(
        &mut self,
        mutation: BlockMutation,
    ) -> Result<AppliedBlockMutation, WorldError> {
        let (chunk_pos, _, _) = split_world_block_xz(mutation.position)?;
        self.chunks
            .get_mut(&chunk_pos)
            .ok_or(WorldError::MissingChunk { chunk: chunk_pos })?
            .apply_block_mutation(mutation)
    }

    pub fn apply_event(&mut self, event: WorldEvent) -> Result<AppliedWorldEvent, WorldError> {
        match event {
            WorldEvent::BlockMutation(mutation) => Ok(AppliedWorldEvent::BlockMutation(
                self.apply_block_mutation(mutation)?,
            )),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ChunkPos, &StaticChunk)> {
        self.chunks.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

fn block_index(x: usize, y: usize, z: usize) -> Result<usize, WorldError> {
    if x >= SECTION_EDGE || y >= SECTION_EDGE || z >= SECTION_EDGE {
        return Err(WorldError::BlockCoordinateOutOfRange { x, y, z });
    }
    Ok((y << 8) | (z << 4) | x)
}

fn biome_index(x: usize, y: usize, z: usize) -> Result<usize, WorldError> {
    if x >= BIOME_EDGE || y >= BIOME_EDGE || z >= BIOME_EDGE {
        return Err(WorldError::BiomeCoordinateOutOfRange { x, y, z });
    }
    Ok((y << 4) | (z << 2) | x)
}

fn split_world_block_xz(position: BlockPos) -> Result<(ChunkPos, usize, usize), WorldError> {
    let chunk_x = position
        .x
        .checked_div_euclid(SECTION_EDGE as i32)
        .ok_or(WorldError::BlockCoordinateOverflow)?;
    let chunk_z = position
        .z
        .checked_div_euclid(SECTION_EDGE as i32)
        .ok_or(WorldError::BlockCoordinateOverflow)?;
    let local_x = usize::try_from(position.x.rem_euclid(SECTION_EDGE as i32))
        .map_err(|_| WorldError::BlockCoordinateOverflow)?;
    let local_z = usize::try_from(position.z.rem_euclid(SECTION_EDGE as i32))
        .map_err(|_| WorldError::BlockCoordinateOverflow)?;
    Ok((
        ChunkPos {
            x: chunk_x,
            z: chunk_z,
        },
        local_x,
        local_z,
    ))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorldError {
    #[error("a chunk must contain at least one section")]
    EmptyChunk,
    #[error("section count {section_count} exceeds the supported range")]
    TooManySections { section_count: usize },
    #[error("section coordinate arithmetic overflowed")]
    SectionCoordinateOverflow,
    #[error(
        "section y {section_y} is outside chunk range starting at {min_section_y} with {section_count} sections"
    )]
    SectionOutOfRange {
        section_y: i32,
        min_section_y: i32,
        section_count: usize,
    },
    #[error("block coordinate is outside a 16x16x16 section: ({x}, {y}, {z})")]
    BlockCoordinateOutOfRange { x: usize, y: usize, z: usize },
    #[error("block coordinate arithmetic overflowed")]
    BlockCoordinateOverflow,
    #[error(
        "world block ({}, {}, {}) is outside chunk ({}, {})",
        position.x,
        position.y,
        position.z,
        chunk.x,
        chunk.z
    )]
    BlockOutsideChunk { position: BlockPos, chunk: ChunkPos },
    #[error("chunk ({}, {}) is not loaded", chunk.x, chunk.z)]
    MissingChunk { chunk: ChunkPos },
    #[error("biome coordinate is outside a 4x4x4 section: ({x}, {y}, {z})")]
    BiomeCoordinateOutOfRange { x: usize, y: usize, z: usize },
    #[error("non-empty block count overflow")]
    NonEmptyBlockCountOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_runtime::{ConnectionId, DeterministicRuntime, Tick};
    use std::num::NonZeroUsize;

    const AIR: BlockStateId = BlockStateId::new(0);
    const STONE: BlockStateId = BlockStateId::new(1);
    const GRASS: BlockStateId = BlockStateId::new(9);
    const DIRT: BlockStateId = BlockStateId::new(10);
    const BEDROCK: BlockStateId = BlockStateId::new(85);
    const PLAINS: BiomeId = BiomeId::new(40);

    #[test]
    fn block_index_matches_minecraft_section_order() {
        assert_eq!(block_index(0, 0, 0).unwrap(), 0);
        assert_eq!(block_index(1, 0, 0).unwrap(), 1);
        assert_eq!(block_index(0, 0, 1).unwrap(), 16);
        assert_eq!(block_index(0, 1, 0).unwrap(), 256);
        assert_eq!(block_index(15, 15, 15).unwrap(), 4_095);
    }

    #[test]
    fn builds_a_four_layer_flat_overworld_floor() {
        let chunk = StaticChunk::flat_overworld(
            ChunkPos { x: 0, z: 0 },
            -4,
            24,
            FlatWorldSpec {
                floor_y: 63,
                air: AIR,
                bedrock: BEDROCK,
                stone: STONE,
                dirt: DIRT,
                grass: GRASS,
                biome: PLAINS,
            },
        )
        .unwrap();

        let floor_section = chunk.section(3).unwrap();
        assert_eq!(floor_section.non_empty_block_count(), 1_024);
        assert_eq!(floor_section.block(0, 12, 0).unwrap(), BEDROCK);
        assert_eq!(floor_section.block(0, 13, 0).unwrap(), STONE);
        assert_eq!(floor_section.block(0, 14, 0).unwrap(), DIRT);
        assert_eq!(floor_section.block(0, 15, 0).unwrap(), GRASS);
        assert_eq!(chunk.section(2).unwrap().non_empty_block_count(), 0);
        assert_eq!(chunk.section(4).unwrap().non_empty_block_count(), 0);
    }

    #[test]
    fn tracks_air_transitions_without_recounting_the_section() {
        let mut section = ChunkSection::new(AIR, PLAINS);
        assert_eq!(section.set_block(1, 2, 3, STONE).unwrap(), AIR);
        assert_eq!(section.non_empty_block_count(), 1);
        assert_eq!(section.set_block(1, 2, 3, DIRT).unwrap(), STONE);
        assert_eq!(section.non_empty_block_count(), 1);
        assert_eq!(section.set_block(1, 2, 3, AIR).unwrap(), DIRT);
        assert_eq!(section.non_empty_block_count(), 0);
    }

    #[test]
    fn rejects_world_y_outside_the_chunk() {
        let mut chunk = StaticChunk::new(ChunkPos { x: 0, z: 0 }, -4, 24, AIR, PLAINS).unwrap();
        let error = chunk.set_block(0, 320, 0, STONE).unwrap_err();
        assert!(matches!(error, WorldError::SectionOutOfRange { .. }));
    }

    #[test]
    fn applies_world_block_mutations_with_negative_coordinate_flooring() {
        let mut chunk = StaticChunk::new(ChunkPos { x: -1, z: -1 }, -4, 24, AIR, PLAINS).unwrap();
        let applied = chunk
            .apply_block_mutation(BlockMutation {
                position: BlockPos {
                    x: -1,
                    y: -1,
                    z: -16,
                },
                state: STONE,
            })
            .unwrap();

        assert_eq!(applied.chunk, ChunkPos { x: -1, z: -1 });
        assert_eq!(applied.section_y, -1);
        assert_eq!(applied.local_x, 15);
        assert_eq!(applied.local_y, 15);
        assert_eq!(applied.local_z, 0);
        assert_eq!(applied.previous, AIR);
        assert_eq!(applied.current, STONE);
        assert_eq!(
            chunk
                .world_block(BlockPos {
                    x: -1,
                    y: -1,
                    z: -16
                })
                .unwrap(),
            STONE
        );
        assert_eq!(chunk.section(-1).unwrap().non_empty_block_count(), 1);
    }

    #[test]
    fn rejects_world_block_mutations_for_other_chunks() {
        let mut chunk = StaticChunk::new(ChunkPos { x: 0, z: 0 }, -4, 24, AIR, PLAINS).unwrap();
        let error = chunk
            .apply_block_mutation(BlockMutation {
                position: BlockPos { x: 16, y: 0, z: 0 },
                state: STONE,
            })
            .unwrap_err();
        assert_eq!(
            error,
            WorldError::BlockOutsideChunk {
                position: BlockPos { x: 16, y: 0, z: 0 },
                chunk: ChunkPos { x: 0, z: 0 },
            }
        );
    }

    #[test]
    fn chunk_store_applies_world_mutations_to_the_target_chunk() {
        let mut store = ChunkStore::new();
        store.insert(StaticChunk::new(ChunkPos { x: -1, z: 2 }, -4, 24, AIR, PLAINS).unwrap());

        let applied = store
            .apply_block_mutation(BlockMutation {
                position: BlockPos {
                    x: -1,
                    y: 65,
                    z: 32,
                },
                state: GRASS,
            })
            .unwrap();

        assert_eq!(applied.chunk, ChunkPos { x: -1, z: 2 });
        assert_eq!(
            store
                .world_block(BlockPos {
                    x: -1,
                    y: 65,
                    z: 32
                })
                .unwrap(),
            GRASS
        );
        assert_eq!(
            store
                .chunk(ChunkPos { x: -1, z: 2 })
                .unwrap()
                .section(4)
                .unwrap()
                .non_empty_block_count(),
            1
        );
    }

    #[test]
    fn chunk_store_iteration_is_deterministic_and_reports_missing_chunks() {
        let mut store = ChunkStore::new();
        for pos in [
            ChunkPos { x: 2, z: 0 },
            ChunkPos { x: -1, z: 5 },
            ChunkPos { x: 0, z: -3 },
        ] {
            store.insert(StaticChunk::new(pos, -4, 24, AIR, PLAINS).unwrap());
        }

        assert_eq!(
            store.iter().map(|(pos, _)| *pos).collect::<Vec<_>>(),
            [
                ChunkPos { x: -1, z: 5 },
                ChunkPos { x: 0, z: -3 },
                ChunkPos { x: 2, z: 0 },
            ]
        );
        assert_eq!(
            store
                .apply_block_mutation(BlockMutation {
                    position: BlockPos { x: 16, y: 0, z: 0 },
                    state: STONE,
                })
                .unwrap_err(),
            WorldError::MissingChunk {
                chunk: ChunkPos { x: 1, z: 0 },
            }
        );
    }

    #[test]
    fn chunk_store_can_run_as_authoritative_runtime_state() {
        let mut store = ChunkStore::new();
        store.insert(StaticChunk::new(ChunkPos { x: 0, z: 0 }, -4, 24, AIR, PLAINS).unwrap());
        let mut runtime = DeterministicRuntime::new(
            store,
            NonZeroUsize::new(8).unwrap(),
            NonZeroUsize::new(8).unwrap(),
        );

        runtime
            .push_input(
                ConnectionId::new(2),
                WorldEvent::BlockMutation(BlockMutation {
                    position: BlockPos { x: 1, y: 64, z: 1 },
                    state: STONE,
                }),
            )
            .unwrap();
        runtime
            .push_input(
                ConnectionId::new(1),
                WorldEvent::BlockMutation(BlockMutation {
                    position: BlockPos { x: 1, y: 64, z: 1 },
                    state: DIRT,
                }),
            )
            .unwrap();

        let mut applied = Vec::new();
        assert_eq!(
            runtime.execute_tick(Tick::new(12), |store, tick, event| {
                let result = store.apply_event(event.payload).unwrap();
                applied.push((tick.get(), event.connection.get(), result));
            }),
            2
        );

        assert_eq!(
            applied,
            [
                (
                    12,
                    1,
                    AppliedWorldEvent::BlockMutation(AppliedBlockMutation {
                        position: BlockPos { x: 1, y: 64, z: 1 },
                        chunk: ChunkPos { x: 0, z: 0 },
                        section_y: 4,
                        local_x: 1,
                        local_y: 0,
                        local_z: 1,
                        previous: AIR,
                        current: DIRT,
                    })
                ),
                (
                    12,
                    2,
                    AppliedWorldEvent::BlockMutation(AppliedBlockMutation {
                        position: BlockPos { x: 1, y: 64, z: 1 },
                        chunk: ChunkPos { x: 0, z: 0 },
                        section_y: 4,
                        local_x: 1,
                        local_y: 0,
                        local_z: 1,
                        previous: DIRT,
                        current: STONE,
                    })
                ),
            ]
        );
        assert_eq!(
            runtime
                .state()
                .world_block(BlockPos { x: 1, y: 64, z: 1 })
                .unwrap(),
            STONE
        );
    }
}
