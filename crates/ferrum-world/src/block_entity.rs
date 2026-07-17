use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::BlockPos;

pub const MAX_BLOCK_ENTITIES: usize = 1_000_000;
pub const MAX_BLOCK_ENTITY_DATA_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockEntity {
    pub position: BlockPos,
    pub kind: String,
    pub data: Value,
    pub dirty: bool,
    pub last_changed_tick: u64,
}

impl BlockEntity {
    pub fn new(
        position: BlockPos,
        kind: impl Into<String>,
        data: Value,
        tick: u64,
    ) -> Result<Self, BlockEntityError> {
        let kind = kind.into();
        validate_kind(&kind)?;
        validate_data(&data)?;
        Ok(Self {
            position,
            kind,
            data,
            dirty: true,
            last_changed_tick: tick,
        })
    }

    pub fn replace_data(&mut self, data: Value, tick: u64) -> Result<Value, BlockEntityError> {
        validate_data(&data)?;
        self.dirty = true;
        self.last_changed_tick = tick;
        Ok(std::mem::replace(&mut self.data, data))
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlockEntityStore {
    entries: BTreeMap<BlockPos, BlockEntity>,
}

impl BlockEntityStore {
    pub fn insert(&mut self, entity: BlockEntity) -> Result<Option<BlockEntity>, BlockEntityError> {
        if !self.entries.contains_key(&entity.position) && self.entries.len() >= MAX_BLOCK_ENTITIES
        {
            return Err(BlockEntityError::StoreFull {
                limit: MAX_BLOCK_ENTITIES,
            });
        }
        Ok(self.entries.insert(entity.position, entity))
    }

    #[must_use]
    pub fn get(&self, position: BlockPos) -> Option<&BlockEntity> {
        self.entries.get(&position)
    }

    pub fn get_mut(&mut self, position: BlockPos) -> Option<&mut BlockEntity> {
        self.entries.get_mut(&position)
    }

    pub fn remove(&mut self, position: BlockPos) -> Option<BlockEntity> {
        self.entries.remove(&position)
    }

    pub fn dirty(&self) -> impl Iterator<Item = &BlockEntity> {
        self.entries.values().filter(|entity| entity.dirty)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn validate_kind(kind: &str) -> Result<(), BlockEntityError> {
    let Some((namespace, path)) = kind.split_once(':') else {
        return Err(BlockEntityError::InvalidKind {
            kind: kind.to_owned(),
        });
    };
    if namespace.is_empty()
        || path.is_empty()
        || !kind.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b':' | b'_' | b'-' | b'.' | b'/')
        })
    {
        return Err(BlockEntityError::InvalidKind {
            kind: kind.to_owned(),
        });
    }
    Ok(())
}

fn validate_data(data: &Value) -> Result<(), BlockEntityError> {
    let bytes = serde_json::to_vec(data).map_err(BlockEntityError::Serialize)?;
    if bytes.len() > MAX_BLOCK_ENTITY_DATA_BYTES {
        return Err(BlockEntityError::DataTooLarge {
            actual: bytes.len(),
            limit: MAX_BLOCK_ENTITY_DATA_BYTES,
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum BlockEntityError {
    #[error("invalid block-entity kind {kind}")]
    InvalidKind { kind: String },
    #[error("block-entity data cannot be serialized")]
    Serialize(#[source] serde_json::Error),
    #[error("block-entity data has {actual} bytes; limit is {limit}")]
    DataTooLarge { actual: usize, limit: usize },
    #[error("block-entity store reached limit {limit}")]
    StoreFull { limit: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn block_entities_track_dirty_state_and_updates() {
        let position = BlockPos { x: 1, y: 64, z: 2 };
        let mut store = BlockEntityStore::default();
        store
            .insert(BlockEntity::new(position, "minecraft:chest", json!({"items": []}), 1).unwrap())
            .unwrap();
        store.get_mut(position).unwrap().mark_clean();
        assert_eq!(store.dirty().count(), 0);
        store
            .get_mut(position)
            .unwrap()
            .replace_data(json!({"items": [1]}), 2)
            .unwrap();
        assert_eq!(store.dirty().count(), 1);
    }
}
