use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::BlockPos;

pub const MAX_REDSTONE_POWER: u8 = 15;
pub const DEFAULT_MAX_REDSTONE_NODES: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedstoneNetwork {
    power: BTreeMap<BlockPos, u8>,
    sources: BTreeMap<BlockPos, u8>,
    max_nodes: usize,
}

impl Default for RedstoneNetwork {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_REDSTONE_NODES).expect("default redstone limit is non-zero")
    }
}

impl RedstoneNetwork {
    pub fn new(max_nodes: usize) -> Result<Self, RedstoneError> {
        if max_nodes == 0 {
            return Err(RedstoneError::ZeroNodeLimit);
        }
        Ok(Self {
            power: BTreeMap::new(),
            sources: BTreeMap::new(),
            max_nodes,
        })
    }

    pub fn set_source(&mut self, position: BlockPos, power: u8) -> Result<(), RedstoneError> {
        validate_power(power)?;
        if power == 0 {
            self.sources.remove(&position);
        } else {
            self.sources.insert(position, power);
        }
        Ok(())
    }

    pub fn recompute<I>(&mut self, conductors: I) -> Result<usize, RedstoneError>
    where
        I: IntoIterator<Item = BlockPos>,
    {
        let conductors = conductors.into_iter().collect::<BTreeSet<_>>();
        if conductors.len() > self.max_nodes {
            return Err(RedstoneError::TooManyNodes {
                actual: conductors.len(),
                limit: self.max_nodes,
            });
        }
        self.power.clear();
        let mut queue = VecDeque::new();
        for (&position, &power) in &self.sources {
            if conductors.contains(&position) {
                self.power.insert(position, power);
                queue.push_back(position);
            }
        }
        while let Some(position) = queue.pop_front() {
            let current = self.power.get(&position).copied().unwrap_or(0);
            if current <= 1 {
                continue;
            }
            for neighbor in neighbors(position) {
                if !conductors.contains(&neighbor) {
                    continue;
                }
                let next = current - 1;
                if self.power.get(&neighbor).copied().unwrap_or(0) >= next {
                    continue;
                }
                self.power.insert(neighbor, next);
                queue.push_back(neighbor);
            }
        }
        Ok(self.power.len())
    }

    #[must_use]
    pub fn power_at(&self, position: BlockPos) -> u8 {
        self.power.get(&position).copied().unwrap_or(0)
    }
}

fn neighbors(position: BlockPos) -> [BlockPos; 6] {
    [
        BlockPos {
            x: position.x - 1,
            ..position
        },
        BlockPos {
            x: position.x + 1,
            ..position
        },
        BlockPos {
            y: position.y - 1,
            ..position
        },
        BlockPos {
            y: position.y + 1,
            ..position
        },
        BlockPos {
            z: position.z - 1,
            ..position
        },
        BlockPos {
            z: position.z + 1,
            ..position
        },
    ]
}

fn validate_power(power: u8) -> Result<(), RedstoneError> {
    if power > MAX_REDSTONE_POWER {
        return Err(RedstoneError::PowerOutOfRange { power });
    }
    Ok(())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RedstoneError {
    #[error("redstone node limit must be greater than zero")]
    ZeroNodeLimit,
    #[error("redstone power {power} exceeds {MAX_REDSTONE_POWER}")]
    PowerOutOfRange { power: u8 },
    #[error("redstone network has {actual} nodes; limit is {limit}")]
    TooManyNodes { actual: usize, limit: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_decays_across_connected_nodes() {
        let nodes = (0..4)
            .map(|x| BlockPos { x, y: 64, z: 0 })
            .collect::<Vec<_>>();
        let mut network = RedstoneNetwork::new(16).unwrap();
        network.set_source(nodes[0], 15).unwrap();
        network.recompute(nodes.clone()).unwrap();
        assert_eq!(network.power_at(nodes[0]), 15);
        assert_eq!(network.power_at(nodes[3]), 12);
    }
}
