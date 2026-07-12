use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{PacketDirection, PacketKind, PacketTable, ProtocolPhase};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PacketDescriptor {
    pub phase: ProtocolPhase,
    pub direction: PacketDirection,
    pub name: String,
    pub id: i32,
}

impl PacketDescriptor {
    pub fn new(
        phase: ProtocolPhase,
        direction: PacketDirection,
        name: impl Into<String>,
        id: i32,
    ) -> Result<Self, PacketCatalogError> {
        if phase == ProtocolPhase::Closed {
            return Err(PacketCatalogError::ClosedPhase);
        }
        if id < 0 {
            return Err(PacketCatalogError::NegativeId { id });
        }
        let name = normalize_packet_name(&name.into())?;
        Ok(Self {
            phase,
            direction,
            name,
            id,
        })
    }

    #[must_use]
    pub fn known_kind(&self) -> Option<PacketKind> {
        known_packet_kind(self.phase, self.direction, &self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PacketCatalog {
    entries: Vec<PacketDescriptor>,
}

impl PacketCatalog {
    pub fn new(
        entries: impl IntoIterator<Item = PacketDescriptor>,
    ) -> Result<Self, PacketCatalogError> {
        let mut entries = entries.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            (left.phase, left.direction, left.id, left.name.as_str()).cmp(&(
                right.phase,
                right.direction,
                right.id,
                right.name.as_str(),
            ))
        });

        let mut names = BTreeMap::new();
        let mut ids = BTreeMap::new();
        for entry in &entries {
            if entry.phase == ProtocolPhase::Closed {
                return Err(PacketCatalogError::ClosedPhase);
            }
            if entry.id < 0 {
                return Err(PacketCatalogError::NegativeId { id: entry.id });
            }
            let normalized = normalize_packet_name(&entry.name)?;
            if normalized != entry.name {
                return Err(PacketCatalogError::NonCanonicalName {
                    name: entry.name.clone(),
                    canonical: normalized,
                });
            }
            let name_key = (entry.phase, entry.direction, entry.name.clone());
            if let Some(previous_id) = names.insert(name_key, entry.id) {
                return Err(PacketCatalogError::DuplicateName {
                    phase: entry.phase,
                    direction: entry.direction,
                    name: entry.name.clone(),
                    first_id: previous_id,
                    second_id: entry.id,
                });
            }
            let id_key = (entry.phase, entry.direction, entry.id);
            if let Some(previous_name) = ids.insert(id_key, entry.name.clone()) {
                return Err(PacketCatalogError::DuplicateId {
                    phase: entry.phase,
                    direction: entry.direction,
                    id: entry.id,
                    first_name: previous_name,
                    second_name: entry.name.clone(),
                });
            }
        }
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &[PacketDescriptor] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn find(
        &self,
        phase: ProtocolPhase,
        direction: PacketDirection,
        name: &str,
    ) -> Option<&PacketDescriptor> {
        let name = normalize_packet_name(name).ok()?;
        self.entries.iter().find(|entry| {
            entry.phase == phase && entry.direction == direction && entry.name == name
        })
    }

    #[must_use]
    pub fn resolve(
        &self,
        phase: ProtocolPhase,
        direction: PacketDirection,
        id: i32,
    ) -> Option<&PacketDescriptor> {
        self.entries
            .iter()
            .find(|entry| entry.phase == phase && entry.direction == direction && entry.id == id)
    }

    pub fn typed_table(&self) -> Result<PacketTable, PacketCatalogError> {
        let mut table = PacketTable::new();
        let mut kinds = BTreeSet::new();
        for entry in &self.entries {
            let Some(kind) = entry.known_kind() else {
                continue;
            };
            if !kinds.insert(kind) {
                return Err(PacketCatalogError::DuplicateKnownKind { kind });
            }
            table
                .insert(kind, entry.id)
                .map_err(|error| PacketCatalogError::TypedTable {
                    message: error.to_string(),
                })?;
        }
        Ok(table)
    }
}

pub fn normalize_packet_name(name: &str) -> Result<String, PacketCatalogError> {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err(PacketCatalogError::EmptyName);
    }
    let canonical = if name.contains(':') {
        name
    } else {
        format!("minecraft:{name}")
    };
    let Some((namespace, path)) = canonical.split_once(':') else {
        return Err(PacketCatalogError::InvalidName { name: canonical });
    };
    if namespace.is_empty()
        || path.is_empty()
        || namespace
            .bytes()
            .any(|byte| !matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
        || path.bytes().any(|byte| {
            !matches!(
                byte,
                b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/'
            )
        })
    {
        return Err(PacketCatalogError::InvalidName { name: canonical });
    }
    Ok(canonical)
}

#[must_use]
pub fn known_packet_kind(
    phase: ProtocolPhase,
    direction: PacketDirection,
    name: &str,
) -> Option<PacketKind> {
    let name = normalize_packet_name(name).ok()?;
    let path = name.strip_prefix("minecraft:").unwrap_or(&name);
    match (phase, direction, path) {
        (ProtocolPhase::Handshake, PacketDirection::Serverbound, "intention" | "handshake") => {
            Some(PacketKind::Handshake)
        }
        (ProtocolPhase::Status, PacketDirection::Serverbound, "status_request" | "status") => {
            Some(PacketKind::StatusRequest)
        }
        (ProtocolPhase::Status, PacketDirection::Serverbound, "ping_request" | "ping") => {
            Some(PacketKind::PingRequest)
        }
        (ProtocolPhase::Status, PacketDirection::Clientbound, "status_response") => {
            Some(PacketKind::StatusResponse)
        }
        (ProtocolPhase::Status, PacketDirection::Clientbound, "pong_response" | "pong") => {
            Some(PacketKind::PongResponse)
        }
        (ProtocolPhase::Login, PacketDirection::Serverbound, "hello" | "login_start") => {
            Some(PacketKind::LoginStart)
        }
        (ProtocolPhase::Login, PacketDirection::Serverbound, "login_acknowledged") => {
            Some(PacketKind::LoginAcknowledged)
        }
        (ProtocolPhase::Login, PacketDirection::Clientbound, "login_disconnect" | "disconnect") => {
            Some(PacketKind::LoginDisconnect)
        }
        (ProtocolPhase::Login, PacketDirection::Clientbound, "game_profile" | "login_success") => {
            Some(PacketKind::LoginSuccess)
        }
        (
            ProtocolPhase::Configuration,
            PacketDirection::Serverbound,
            "configuration_acknowledged" | "finish_configuration",
        ) => Some(PacketKind::ConfigurationAcknowledged),
        (ProtocolPhase::Configuration, PacketDirection::Serverbound, "client_information") => {
            Some(PacketKind::ConfigurationClientInformation)
        }
        (ProtocolPhase::Configuration, PacketDirection::Clientbound, "disconnect") => {
            Some(PacketKind::ConfigurationDisconnect)
        }
        (ProtocolPhase::Configuration, PacketDirection::Clientbound, "finish_configuration") => {
            Some(PacketKind::FinishConfiguration)
        }
        (ProtocolPhase::Configuration, PacketDirection::Clientbound, "registry_data") => {
            Some(PacketKind::RegistryData)
        }
        (
            ProtocolPhase::Configuration,
            PacketDirection::Clientbound,
            "update_enabled_features" | "feature_flags",
        ) => Some(PacketKind::FeatureFlags),
        (ProtocolPhase::Configuration, PacketDirection::Clientbound, "update_tags") => {
            Some(PacketKind::UpdateTags)
        }
        (ProtocolPhase::Configuration, PacketDirection::Clientbound, "select_known_packs") => {
            Some(PacketKind::SelectKnownPacksRequest)
        }
        (ProtocolPhase::Configuration, PacketDirection::Serverbound, "select_known_packs") => {
            Some(PacketKind::SelectKnownPacksResponse)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "login") => Some(PacketKind::PlayLogin),
        (ProtocolPhase::Play, PacketDirection::Clientbound, "chunk_batch_start") => {
            Some(PacketKind::ChunkBatchStart)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "chunk_batch_finished") => {
            Some(PacketKind::ChunkBatchFinished)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "chunk_batch_received") => {
            Some(PacketKind::ChunkBatchReceived)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "level_chunk_with_light") => {
            Some(PacketKind::LevelChunkWithLight)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_chunk_cache_center") => {
            Some(PacketKind::SetChunkCacheCenter)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_default_spawn_position") => {
            Some(PacketKind::DefaultSpawnPosition)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "player_position") => {
            Some(PacketKind::PlayerPosition)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "system_chat") => {
            Some(PacketKind::SystemChat)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "accept_teleportation") => {
            Some(PacketKind::AcceptTeleportation)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "disconnect") => {
            Some(PacketKind::PlayDisconnect)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "keep_alive") => {
            Some(PacketKind::KeepAliveRequest)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "keep_alive") => {
            Some(PacketKind::KeepAliveResponse)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "client_tick_end") => {
            Some(PacketKind::ClientTickEnd)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "move_player_pos") => {
            Some(PacketKind::MovePlayerPosition)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "move_player_pos_rot") => {
            Some(PacketKind::MovePlayerPositionRotation)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "move_player_rot") => {
            Some(PacketKind::MovePlayerRotation)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "move_player_status_only") => {
            Some(PacketKind::MovePlayerStatusOnly)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "player_action") => {
            Some(PacketKind::PlayerAction)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "use_item_on") => {
            Some(PacketKind::UseItemOn)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "block_changed_ack") => {
            Some(PacketKind::BlockChangedAck)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "block_update") => {
            Some(PacketKind::BlockUpdate)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "forget_level_chunk") => {
            Some(PacketKind::ForgetLevelChunk)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "chat_command") => {
            Some(PacketKind::ChatCommand)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "chat") => {
            Some(PacketKind::ChatMessage)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "set_carried_item") => {
            Some(PacketKind::SetCarriedItem)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "container_click") => {
            Some(PacketKind::ContainerClick)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "container_close") => {
            Some(PacketKind::CloseContainer)
        }
        (ProtocolPhase::Play, PacketDirection::Serverbound, "set_creative_mode_slot") => {
            Some(PacketKind::SetCreativeModeSlot)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_held_slot") => {
            Some(PacketKind::SetHeldSlot)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "container_set_content") => {
            Some(PacketKind::SetContainerContent)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "container_set_slot") => {
            Some(PacketKind::SetContainerSlot)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "add_entity") => {
            Some(PacketKind::AddEntity)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "remove_entities") => {
            Some(PacketKind::RemoveEntities)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "move_entity_pos") => {
            Some(PacketKind::MoveEntityPosition)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "move_entity_pos_rot") => {
            Some(PacketKind::MoveEntityPositionRotation)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "move_entity_rot") => {
            Some(PacketKind::MoveEntityRotation)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "teleport_entity") => {
            Some(PacketKind::TeleportEntity)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "rotate_head") => {
            Some(PacketKind::RotateHead)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_entity_data") => {
            Some(PacketKind::SetEntityData)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_equipment") => {
            Some(PacketKind::SetEquipment)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "player_info_update") => {
            Some(PacketKind::PlayerInfoUpdate)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "player_info_remove") => {
            Some(PacketKind::PlayerInfoRemove)
        }
        _ => None,
    }
}

#[must_use]
pub const fn canonical_packet_name(kind: PacketKind) -> &'static str {
    match kind {
        PacketKind::Handshake => "minecraft:intention",
        PacketKind::StatusRequest => "minecraft:status_request",
        PacketKind::PingRequest => "minecraft:ping_request",
        PacketKind::StatusResponse => "minecraft:status_response",
        PacketKind::PongResponse => "minecraft:pong_response",
        PacketKind::LoginStart => "minecraft:hello",
        PacketKind::LoginAcknowledged => "minecraft:login_acknowledged",
        PacketKind::LoginDisconnect => "minecraft:login_disconnect",
        PacketKind::LoginSuccess => "minecraft:game_profile",
        PacketKind::ConfigurationAcknowledged => "minecraft:configuration_acknowledged",
        PacketKind::ConfigurationClientInformation => "minecraft:client_information",
        PacketKind::ConfigurationDisconnect => "minecraft:disconnect",
        PacketKind::RegistryData => "minecraft:registry_data",
        PacketKind::FeatureFlags => "minecraft:update_enabled_features",
        PacketKind::UpdateTags => "minecraft:update_tags",
        PacketKind::SelectKnownPacksRequest | PacketKind::SelectKnownPacksResponse => {
            "minecraft:select_known_packs"
        }
        PacketKind::FinishConfiguration => "minecraft:finish_configuration",
        PacketKind::PlayLogin => "minecraft:login",
        PacketKind::ChunkBatchStart => "minecraft:chunk_batch_start",
        PacketKind::ChunkBatchFinished => "minecraft:chunk_batch_finished",
        PacketKind::ChunkBatchReceived => "minecraft:chunk_batch_received",
        PacketKind::LevelChunkWithLight => "minecraft:level_chunk_with_light",
        PacketKind::SetChunkCacheCenter => "minecraft:set_chunk_cache_center",
        PacketKind::DefaultSpawnPosition => "minecraft:set_default_spawn_position",
        PacketKind::PlayerPosition => "minecraft:player_position",
        PacketKind::SystemChat => "minecraft:system_chat",
        PacketKind::AcceptTeleportation => "minecraft:accept_teleportation",
        PacketKind::PlayDisconnect => "minecraft:disconnect",
        PacketKind::KeepAliveRequest | PacketKind::KeepAliveResponse => "minecraft:keep_alive",
        PacketKind::ClientTickEnd => "minecraft:client_tick_end",
        PacketKind::MovePlayerPosition => "minecraft:move_player_pos",
        PacketKind::MovePlayerPositionRotation => "minecraft:move_player_pos_rot",
        PacketKind::MovePlayerRotation => "minecraft:move_player_rot",
        PacketKind::MovePlayerStatusOnly => "minecraft:move_player_status_only",
        PacketKind::PlayerAction => "minecraft:player_action",
        PacketKind::UseItemOn => "minecraft:use_item_on",
        PacketKind::BlockChangedAck => "minecraft:block_changed_ack",
        PacketKind::BlockUpdate => "minecraft:block_update",
        PacketKind::ForgetLevelChunk => "minecraft:forget_level_chunk",
        PacketKind::ChatCommand => "minecraft:chat_command",
        PacketKind::ChatMessage => "minecraft:chat",
        PacketKind::SetCarriedItem => "minecraft:set_carried_item",
        PacketKind::ContainerClick => "minecraft:container_click",
        PacketKind::CloseContainer => "minecraft:container_close",
        PacketKind::SetCreativeModeSlot => "minecraft:set_creative_mode_slot",
        PacketKind::SetHeldSlot => "minecraft:set_held_slot",
        PacketKind::SetContainerContent => "minecraft:container_set_content",
        PacketKind::SetContainerSlot => "minecraft:container_set_slot",
        PacketKind::AddEntity => "minecraft:add_entity",
        PacketKind::RemoveEntities => "minecraft:remove_entities",
        PacketKind::MoveEntityPosition => "minecraft:move_entity_pos",
        PacketKind::MoveEntityPositionRotation => "minecraft:move_entity_pos_rot",
        PacketKind::MoveEntityRotation => "minecraft:move_entity_rot",
        PacketKind::TeleportEntity => "minecraft:teleport_entity",
        PacketKind::RotateHead => "minecraft:rotate_head",
        PacketKind::SetEntityData => "minecraft:set_entity_data",
        PacketKind::SetEquipment => "minecraft:set_equipment",
        PacketKind::PlayerInfoUpdate => "minecraft:player_info_update",
        PacketKind::PlayerInfoRemove => "minecraft:player_info_remove",
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PacketCatalogError {
    #[error("packet catalog cannot contain the closed protocol phase")]
    ClosedPhase,
    #[error("packet ID cannot be negative: {id}")]
    NegativeId { id: i32 },
    #[error("packet name cannot be empty")]
    EmptyName,
    #[error("invalid packet name {name}")]
    InvalidName { name: String },
    #[error("packet name {name} is not canonical; expected {canonical}")]
    NonCanonicalName { name: String, canonical: String },
    #[error(
        "duplicate packet name {name} in {phase:?}/{direction:?}: IDs {first_id} and {second_id}"
    )]
    DuplicateName {
        phase: ProtocolPhase,
        direction: PacketDirection,
        name: String,
        first_id: i32,
        second_id: i32,
    },
    #[error("duplicate packet ID {id} in {phase:?}/{direction:?}: {first_name} and {second_name}")]
    DuplicateId {
        phase: ProtocolPhase,
        direction: PacketDirection,
        id: i32,
        first_name: String,
        second_name: String,
    },
    #[error("multiple catalog entries map to known packet kind {kind:?}")]
    DuplicateKnownKind { kind: PacketKind },
    #[error("cannot build typed packet table: {message}")]
    TypedTable { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_sorts_report_packet_names() {
        let catalog = PacketCatalog::new([
            PacketDescriptor::new(
                ProtocolPhase::Play,
                PacketDirection::Serverbound,
                "set_carried_item",
                52,
            )
            .unwrap(),
            PacketDescriptor::new(
                ProtocolPhase::Play,
                PacketDirection::Serverbound,
                "minecraft:chat_command",
                6,
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(catalog.entries()[0].id, 6);
        assert_eq!(catalog.entries()[1].name, "minecraft:set_carried_item");
        assert_eq!(
            catalog.entries()[0].known_kind(),
            Some(PacketKind::ChatCommand)
        );
    }

    #[test]
    fn rejects_duplicate_ids_and_names() {
        let duplicate_id = PacketCatalog::new([
            PacketDescriptor::new(
                ProtocolPhase::Play,
                PacketDirection::Clientbound,
                "minecraft:system_chat",
                1,
            )
            .unwrap(),
            PacketDescriptor::new(
                ProtocolPhase::Play,
                PacketDirection::Clientbound,
                "minecraft:disconnect",
                1,
            )
            .unwrap(),
        ]);
        assert!(matches!(
            duplicate_id,
            Err(PacketCatalogError::DuplicateId { .. })
        ));
    }

    #[test]
    fn derives_typed_table_without_discarding_unknown_packets() {
        let catalog = PacketCatalog::new([
            PacketDescriptor::new(
                ProtocolPhase::Play,
                PacketDirection::Clientbound,
                "minecraft:system_chat",
                121,
            )
            .unwrap(),
            PacketDescriptor::new(
                ProtocolPhase::Play,
                PacketDirection::Clientbound,
                "minecraft:future_packet",
                122,
            )
            .unwrap(),
        ])
        .unwrap();
        let table = catalog.typed_table().unwrap();
        assert_eq!(table.id(PacketKind::SystemChat), Some(121));
        assert_eq!(catalog.len(), 2);
    }
}
