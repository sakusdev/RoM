from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-protocol/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''//! Socket I/O and gameplay state intentionally live outside this crate.

use serde::{Deserialize, Serialize};''',
    '''//! Socket I/O and gameplay state intentionally live outside this crate.

mod packet_catalog;

pub use packet_catalog::{
    PacketCatalog, PacketCatalogError, PacketDescriptor, known_packet_kind,
    normalize_packet_name,
};

use serde::{Deserialize, Serialize};''',
    "catalog module export",
)
text = replace_once(
    text,
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum ProtocolPhase {",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\npub enum ProtocolPhase {",
    "serializable protocol phase",
)
text = replace_once(
    text,
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum PacketDirection {",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]\n#[serde(rename_all = \"snake_case\")]\npub enum PacketDirection {",
    "serializable packet direction",
)
text = replace_once(
    text,
    '''    BlockUpdate,
    ForgetLevelChunk,
}''',
    '''    BlockUpdate,
    ForgetLevelChunk,
    ChatCommand,
    ChatMessage,
    SetCarriedItem,
    ContainerClick,
    CloseContainer,
    SetCreativeModeSlot,
    SetHeldSlot,
    SetContainerContent,
    SetContainerSlot,
    AddEntity,
    RemoveEntities,
    MoveEntityPosition,
    MoveEntityPositionRotation,
    MoveEntityRotation,
    TeleportEntity,
    RotateHead,
    SetEntityData,
    SetEquipment,
    PlayerInfoUpdate,
    PlayerInfoRemove,
}''',
    "optional packet kinds",
)
text = replace_once(
    text,
    '''        Self::BlockUpdate,
        Self::ForgetLevelChunk,
    ];''',
    '''        Self::BlockUpdate,
        Self::ForgetLevelChunk,
        Self::ChatCommand,
        Self::ChatMessage,
        Self::SetCarriedItem,
        Self::ContainerClick,
        Self::CloseContainer,
        Self::SetCreativeModeSlot,
        Self::SetHeldSlot,
        Self::SetContainerContent,
        Self::SetContainerSlot,
        Self::AddEntity,
        Self::RemoveEntities,
        Self::MoveEntityPosition,
        Self::MoveEntityPositionRotation,
        Self::MoveEntityRotation,
        Self::TeleportEntity,
        Self::RotateHead,
        Self::SetEntityData,
        Self::SetEquipment,
        Self::PlayerInfoUpdate,
        Self::PlayerInfoRemove,
    ];

    /// Packet kinds required by the current native server core. Optional kinds
    /// may be present in a generated packet catalog without being required by
    /// hand-authored built-in profiles.
    pub const CORE: &'static [Self] = &[
        Self::Handshake,
        Self::StatusRequest,
        Self::PingRequest,
        Self::StatusResponse,
        Self::PongResponse,
        Self::LoginStart,
        Self::LoginAcknowledged,
        Self::LoginDisconnect,
        Self::LoginSuccess,
        Self::ConfigurationAcknowledged,
        Self::ConfigurationClientInformation,
        Self::ConfigurationDisconnect,
        Self::RegistryData,
        Self::FeatureFlags,
        Self::UpdateTags,
        Self::SelectKnownPacksRequest,
        Self::SelectKnownPacksResponse,
        Self::FinishConfiguration,
        Self::PlayLogin,
        Self::ChunkBatchStart,
        Self::ChunkBatchFinished,
        Self::ChunkBatchReceived,
        Self::LevelChunkWithLight,
        Self::SetChunkCacheCenter,
        Self::DefaultSpawnPosition,
        Self::PlayerPosition,
        Self::SystemChat,
        Self::AcceptTeleportation,
        Self::PlayDisconnect,
        Self::KeepAliveRequest,
        Self::KeepAliveResponse,
        Self::ClientTickEnd,
        Self::MovePlayerPosition,
        Self::MovePlayerPositionRotation,
        Self::MovePlayerRotation,
        Self::MovePlayerStatusOnly,
        Self::PlayerAction,
        Self::UseItemOn,
        Self::BlockChangedAck,
        Self::BlockUpdate,
        Self::ForgetLevelChunk,
    ];''',
    "all and core packet kind sets",
)
text = replace_once(
    text,
    '''             | Self::BlockChangedAck
             | Self::BlockUpdate
             | Self::ForgetLevelChunk => ProtocolPhase::Play,''',
    '''             | Self::BlockChangedAck
             | Self::BlockUpdate
             | Self::ForgetLevelChunk
             | Self::ChatCommand
             | Self::ChatMessage
             | Self::SetCarriedItem
             | Self::ContainerClick
             | Self::CloseContainer
             | Self::SetCreativeModeSlot
             | Self::SetHeldSlot
             | Self::SetContainerContent
             | Self::SetContainerSlot
             | Self::AddEntity
             | Self::RemoveEntities
             | Self::MoveEntityPosition
             | Self::MoveEntityPositionRotation
             | Self::MoveEntityRotation
             | Self::TeleportEntity
             | Self::RotateHead
             | Self::SetEntityData
             | Self::SetEquipment
             | Self::PlayerInfoUpdate
             | Self::PlayerInfoRemove => ProtocolPhase::Play,''',
    "optional packet phases",
)
text = replace_once(
    text,
    '''             | Self::PlayerAction
             | Self::UseItemOn => PacketDirection::Serverbound,
             _ => PacketDirection::Clientbound,''',
    '''             | Self::PlayerAction
             | Self::UseItemOn
             | Self::ChatCommand
             | Self::ChatMessage
             | Self::SetCarriedItem
             | Self::ContainerClick
             | Self::CloseContainer
             | Self::SetCreativeModeSlot => PacketDirection::Serverbound,
             _ => PacketDirection::Clientbound,''',
    "optional packet directions",
)
path.write_text(text, encoding="utf-8")
