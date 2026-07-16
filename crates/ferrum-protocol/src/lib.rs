//! Versioned packet metadata and deterministic connection-state handling for Ferrum.
//!
//! Socket I/O and gameplay state intentionally live outside this crate.

mod packet_catalog;

pub use packet_catalog::{
    PacketCatalog, PacketCatalogError, PacketDescriptor, canonical_packet_name, known_packet_kind,
    normalize_packet_name,
};

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{self, Display, Formatter},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolPhase {
    Handshake,
    Status,
    Login,
    Configuration,
    Play,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketDirection {
    Serverbound,
    Clientbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeIntent {
    Status,
    Login,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketKind {
    Handshake,
    StatusRequest,
    PingRequest,
    StatusResponse,
    PongResponse,
    LoginStart,
    LoginAcknowledged,
    LoginDisconnect,
    LoginSuccess,
    ConfigurationAcknowledged,
    ConfigurationClientInformation,
    ConfigurationDisconnect,
    RegistryData,
    FeatureFlags,
    UpdateTags,
    SelectKnownPacksRequest,
    SelectKnownPacksResponse,
    FinishConfiguration,
    PlayLogin,
    ChunkBatchStart,
    ChunkBatchFinished,
    ChunkBatchReceived,
    LevelChunkWithLight,
    SetChunkCacheCenter,
    DefaultSpawnPosition,
    PlayerPosition,
    SystemChat,
    AcceptTeleportation,
    PlayDisconnect,
    KeepAliveRequest,
    KeepAliveResponse,
    ClientTickEnd,
    ClientCommand,
    MovePlayerPosition,
    MovePlayerPositionRotation,
    MovePlayerRotation,
    MovePlayerStatusOnly,
    PlayerAction,
    UseItemOn,
    BlockChangedAck,
    BlockUpdate,
    ForgetLevelChunk,
    ChatCommand,
    ChatMessage,
    SetCarriedItem,
    ContainerClick,
    CloseContainer,
    SetCreativeModeSlot,
    SetHeldSlot,
    SetPlayerInventory,
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
    SetHealth,
    HurtAnimation,
    PlayerCombatKill,
    Respawn,
    PlayerInfoUpdate,
    PlayerInfoRemove,
}

impl PacketKind {
    pub const ALL: &'static [Self] = &[
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
        Self::ClientCommand,
        Self::MovePlayerPosition,
        Self::MovePlayerPositionRotation,
        Self::MovePlayerRotation,
        Self::MovePlayerStatusOnly,
        Self::PlayerAction,
        Self::UseItemOn,
        Self::BlockChangedAck,
        Self::BlockUpdate,
        Self::ForgetLevelChunk,
        Self::ChatCommand,
        Self::ChatMessage,
        Self::SetCarriedItem,
        Self::ContainerClick,
        Self::CloseContainer,
        Self::SetCreativeModeSlot,
        Self::SetHeldSlot,
        Self::SetPlayerInventory,
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
        Self::SetHealth,
        Self::HurtAnimation,
        Self::PlayerCombatKill,
        Self::Respawn,
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
    ];

    #[must_use]
    pub const fn phase(self) -> ProtocolPhase {
        match self {
            Self::Handshake => ProtocolPhase::Handshake,
            Self::StatusRequest | Self::PingRequest | Self::StatusResponse | Self::PongResponse => {
                ProtocolPhase::Status
            }
            Self::LoginStart
            | Self::LoginAcknowledged
            | Self::LoginDisconnect
            | Self::LoginSuccess => ProtocolPhase::Login,
            Self::ConfigurationAcknowledged
            | Self::ConfigurationClientInformation
            | Self::ConfigurationDisconnect
            | Self::RegistryData
            | Self::FeatureFlags
            | Self::UpdateTags
            | Self::SelectKnownPacksRequest
            | Self::SelectKnownPacksResponse
            | Self::FinishConfiguration => ProtocolPhase::Configuration,
            Self::PlayLogin
            | Self::ChunkBatchStart
            | Self::ChunkBatchFinished
            | Self::ChunkBatchReceived
            | Self::LevelChunkWithLight
            | Self::SetChunkCacheCenter
            | Self::DefaultSpawnPosition
            | Self::PlayerPosition
            | Self::SystemChat
            | Self::AcceptTeleportation
            | Self::PlayDisconnect
            | Self::KeepAliveRequest
            | Self::KeepAliveResponse
            | Self::ClientTickEnd
            | Self::ClientCommand
            | Self::MovePlayerPosition
            | Self::MovePlayerPositionRotation
            | Self::MovePlayerRotation
            | Self::MovePlayerStatusOnly
            | Self::PlayerAction
            | Self::UseItemOn
            | Self::BlockChangedAck
            | Self::BlockUpdate
            | Self::ForgetLevelChunk
            | Self::ChatCommand
            | Self::ChatMessage
            | Self::SetCarriedItem
            | Self::ContainerClick
            | Self::CloseContainer
            | Self::SetCreativeModeSlot
            | Self::SetHeldSlot
            | Self::SetPlayerInventory
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
            | Self::SetHealth
            | Self::HurtAnimation
            | Self::PlayerCombatKill
            | Self::Respawn
            | Self::PlayerInfoUpdate
            | Self::PlayerInfoRemove => ProtocolPhase::Play,
        }
    }

    #[must_use]
    pub const fn direction(self) -> PacketDirection {
        match self {
            Self::Handshake
            | Self::StatusRequest
            | Self::PingRequest
            | Self::LoginStart
            | Self::LoginAcknowledged
            | Self::ConfigurationAcknowledged
            | Self::ConfigurationClientInformation
            | Self::SelectKnownPacksResponse
            | Self::ChunkBatchReceived
            | Self::AcceptTeleportation
            | Self::KeepAliveResponse
            | Self::ClientTickEnd
            | Self::ClientCommand
            | Self::MovePlayerPosition
            | Self::MovePlayerPositionRotation
            | Self::MovePlayerRotation
            | Self::MovePlayerStatusOnly
            | Self::PlayerAction
            | Self::UseItemOn
            | Self::ChatCommand
            | Self::ChatMessage
            | Self::SetCarriedItem
            | Self::ContainerClick
            | Self::CloseContainer
            | Self::SetCreativeModeSlot => PacketDirection::Serverbound,
            _ => PacketDirection::Clientbound,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PacketTable(BTreeMap<PacketKind, i32>);

impl PacketTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, kind: PacketKind, id: i32) -> Result<Option<i32>, ProfileError> {
        if id < 0 {
            return Err(ProfileError::NegativePacketId { kind, id });
        }
        if let Some(first) = self.0.iter().find_map(|(other, other_id)| {
            (*other != kind
                && other.phase() == kind.phase()
                && other.direction() == kind.direction()
                && *other_id == id)
                .then_some(*other)
        }) {
            return Err(ProfileError::DuplicatePacketId {
                phase: kind.phase(),
                direction: kind.direction(),
                id,
                first,
                second: kind,
            });
        }
        Ok(self.0.insert(kind, id))
    }

    #[must_use]
    pub fn id(&self, kind: PacketKind) -> Option<i32> {
        self.0.get(&kind).copied()
    }

    pub fn require(&self, kind: PacketKind) -> Result<i32, ProfileError> {
        self.id(kind).ok_or(ProfileError::MissingPacketId(kind))
    }

    #[must_use]
    pub fn resolve(
        &self,
        phase: ProtocolPhase,
        direction: PacketDirection,
        id: i32,
    ) -> Option<PacketKind> {
        self.0.iter().find_map(|(kind, candidate)| {
            (kind.phase() == phase && kind.direction() == direction && *candidate == id)
                .then_some(*kind)
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (PacketKind, i32)> + '_ {
        self.0.iter().map(|(kind, id)| (*kind, *id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolProfile {
    version_name: String,
    protocol_number: i32,
    packets: PacketTable,
}

impl ProtocolProfile {
    pub fn new(
        version_name: impl Into<String>,
        protocol_number: i32,
        packets: PacketTable,
    ) -> Result<Self, ProfileError> {
        let version_name = version_name.into();
        if version_name.trim().is_empty() {
            return Err(ProfileError::EmptyVersionName);
        }
        if protocol_number < 0 {
            return Err(ProfileError::NegativeProtocolNumber(protocol_number));
        }
        Ok(Self {
            version_name,
            protocol_number,
            packets,
        })
    }

    #[must_use]
    pub fn version_name(&self) -> &str {
        &self.version_name
    }

    #[must_use]
    pub const fn protocol_number(&self) -> i32 {
        self.protocol_number
    }

    #[must_use]
    pub fn packets(&self) -> &PacketTable {
        &self.packets
    }

    #[must_use]
    pub const fn supports(&self, protocol_number: i32) -> bool {
        self.protocol_number == protocol_number
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    EmptyVersionName,
    NegativeProtocolNumber(i32),
    NegativePacketId {
        kind: PacketKind,
        id: i32,
    },
    DuplicatePacketId {
        phase: ProtocolPhase,
        direction: PacketDirection,
        id: i32,
        first: PacketKind,
        second: PacketKind,
    },
    MissingPacketId(PacketKind),
}

impl Display for ProfileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVersionName => f.write_str("protocol version name cannot be empty"),
            Self::NegativeProtocolNumber(value) => write!(f, "negative protocol number {value}"),
            Self::NegativePacketId { kind, id } => write!(f, "negative ID {id} for {kind:?}"),
            Self::DuplicatePacketId {
                phase,
                direction,
                id,
                first,
                second,
            } => write!(
                f,
                "duplicate packet ID {id} in {phase:?}/{direction:?}: {first:?} and {second:?}"
            ),
            Self::MissingPacketId(kind) => write!(f, "missing packet ID for {kind:?}"),
        }
    }
}

impl Error for ProfileError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub phase: ProtocolPhase,
    pub protocol_number: Option<i32>,
    pub username: Option<String>,
    pub pending_keep_alive: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSession {
    phase: ProtocolPhase,
    protocol_number: Option<i32>,
    username: Option<String>,
    status_response_sent: bool,
    login_success_sent: bool,
    configuration_finished_sent: bool,
    pending_keep_alive: Option<i64>,
}

impl Default for ProtocolSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolSession {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: ProtocolPhase::Handshake,
            protocol_number: None,
            username: None,
            status_response_sent: false,
            login_success_sent: false,
            configuration_finished_sent: false,
            pending_keep_alive: None,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> ProtocolPhase {
        self.phase
    }

    #[must_use]
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            phase: self.phase,
            protocol_number: self.protocol_number,
            username: self.username.clone(),
            pending_keep_alive: self.pending_keep_alive,
        }
    }

    pub fn handshake(
        &mut self,
        protocol_number: i32,
        intent: HandshakeIntent,
    ) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Handshake, "handshake")?;
        if protocol_number < 0 {
            return Err(TransitionError::InvalidProtocolNumber(protocol_number));
        }
        self.protocol_number = Some(protocol_number);
        self.phase = match intent {
            HandshakeIntent::Status => ProtocolPhase::Status,
            HandshakeIntent::Login => ProtocolPhase::Login,
        };
        Ok(())
    }

    pub fn status_request(&self) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Status, "status request")
    }

    pub fn status_response_sent(&mut self) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Status, "status response")?;
        if self.status_response_sent {
            return Err(TransitionError::Duplicate("status response"));
        }
        self.status_response_sent = true;
        Ok(())
    }

    pub fn ping(&self) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Status, "status ping")?;
        self.require(self.status_response_sent, "status ping", "status response")
    }

    pub fn pong_sent(&mut self) -> Result<(), TransitionError> {
        self.ping()?;
        self.phase = ProtocolPhase::Closed;
        Ok(())
    }

    pub fn login_start(&mut self, username: impl Into<String>) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Login, "login start")?;
        if self.username.is_some() {
            return Err(TransitionError::Duplicate("login start"));
        }
        let username = username.into();
        validate_username(&username)?;
        self.username = Some(username);
        Ok(())
    }

    pub fn login_success_sent(&mut self) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Login, "login success")?;
        self.require(self.username.is_some(), "login success", "login start")?;
        if self.login_success_sent {
            return Err(TransitionError::Duplicate("login success"));
        }
        self.login_success_sent = true;
        Ok(())
    }

    pub fn login_acknowledged(&mut self) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Login, "login acknowledged")?;
        self.require(
            self.login_success_sent,
            "login acknowledged",
            "login success",
        )?;
        self.phase = ProtocolPhase::Configuration;
        Ok(())
    }

    pub fn finish_configuration_sent(&mut self) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Configuration, "finish configuration")?;
        if self.configuration_finished_sent {
            return Err(TransitionError::Duplicate("finish configuration"));
        }
        self.configuration_finished_sent = true;
        Ok(())
    }

    pub fn configuration_acknowledged(&mut self) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Configuration, "configuration acknowledged")?;
        self.require(
            self.configuration_finished_sent,
            "configuration acknowledged",
            "finish configuration",
        )?;
        self.phase = ProtocolPhase::Play;
        Ok(())
    }

    pub fn keep_alive_sent(&mut self, id: i64) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Play, "keep alive request")?;
        if let Some(pending) = self.pending_keep_alive {
            return Err(TransitionError::KeepAlivePending(pending));
        }
        self.pending_keep_alive = Some(id);
        Ok(())
    }

    pub fn keep_alive_response(&mut self, id: i64) -> Result<(), TransitionError> {
        self.expect(ProtocolPhase::Play, "keep alive response")?;
        match self.pending_keep_alive {
            Some(expected) if expected == id => {
                self.pending_keep_alive = None;
                Ok(())
            }
            Some(expected) => Err(TransitionError::KeepAliveMismatch {
                expected,
                received: id,
            }),
            None => Err(TransitionError::NoKeepAlivePending),
        }
    }

    pub fn disconnect(&mut self) {
        self.phase = ProtocolPhase::Closed;
    }

    fn expect(
        &self,
        expected: ProtocolPhase,
        operation: &'static str,
    ) -> Result<(), TransitionError> {
        if self.phase == ProtocolPhase::Closed {
            return Err(TransitionError::ConnectionClosed);
        }
        if self.phase != expected {
            return Err(TransitionError::UnexpectedPhase {
                operation,
                expected,
                actual: self.phase,
            });
        }
        Ok(())
    }

    fn require(
        &self,
        condition: bool,
        operation: &'static str,
        prerequisite: &'static str,
    ) -> Result<(), TransitionError> {
        condition.then_some(()).ok_or(TransitionError::Missing {
            operation,
            prerequisite,
        })
    }
}

fn validate_username(username: &str) -> Result<(), TransitionError> {
    let length = username.len();
    if !(1..=16).contains(&length) {
        return Err(TransitionError::InvalidUsernameLength(length));
    }
    if !username
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(TransitionError::InvalidUsernameCharacters);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    ConnectionClosed,
    InvalidProtocolNumber(i32),
    InvalidUsernameLength(usize),
    InvalidUsernameCharacters,
    Duplicate(&'static str),
    UnexpectedPhase {
        operation: &'static str,
        expected: ProtocolPhase,
        actual: ProtocolPhase,
    },
    Missing {
        operation: &'static str,
        prerequisite: &'static str,
    },
    KeepAlivePending(i64),
    KeepAliveMismatch {
        expected: i64,
        received: i64,
    },
    NoKeepAlivePending,
}

impl Display for TransitionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionClosed => f.write_str("connection is closed"),
            Self::InvalidProtocolNumber(value) => write!(f, "negative protocol number {value}"),
            Self::InvalidUsernameLength(length) => write!(f, "invalid username length {length}"),
            Self::InvalidUsernameCharacters => f.write_str("invalid username characters"),
            Self::Duplicate(operation) => write!(f, "duplicate {operation}"),
            Self::UnexpectedPhase {
                operation,
                expected,
                actual,
            } => write!(
                f,
                "cannot process {operation} in {actual:?}; expected {expected:?}"
            ),
            Self::Missing {
                operation,
                prerequisite,
            } => write!(f, "cannot process {operation} before {prerequisite}"),
            Self::KeepAlivePending(id) => write!(f, "keep alive {id} is still pending"),
            Self::KeepAliveMismatch { expected, received } => {
                write!(f, "expected keep alive {expected}, received {received}")
            }
            Self::NoKeepAlivePending => f.write_str("no keep alive is pending"),
        }
    }
}

impl Error for TransitionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaches_play_through_modern_login_sequence() {
        let mut session = ProtocolSession::new();
        session.handshake(1_234, HandshakeIntent::Login).unwrap();
        session.login_start("Steve").unwrap();
        session.login_success_sent().unwrap();
        session.login_acknowledged().unwrap();
        assert_eq!(session.phase(), ProtocolPhase::Configuration);
        session.finish_configuration_sent().unwrap();
        session.configuration_acknowledged().unwrap();
        assert_eq!(session.phase(), ProtocolPhase::Play);
        session.keep_alive_sent(42).unwrap();
        session.keep_alive_response(42).unwrap();
        assert_eq!(session.snapshot().pending_keep_alive, None);
    }

    #[test]
    fn status_sequence_closes_after_pong() {
        let mut session = ProtocolSession::new();
        session.handshake(1, HandshakeIntent::Status).unwrap();
        session.status_request().unwrap();
        session.status_response_sent().unwrap();
        session.ping().unwrap();
        session.pong_sent().unwrap();
        assert_eq!(session.phase(), ProtocolPhase::Closed);
    }

    #[test]
    fn login_ack_requires_success() {
        let mut session = ProtocolSession::new();
        session.handshake(1, HandshakeIntent::Login).unwrap();
        session.login_start("Alex").unwrap();
        assert_eq!(
            session.login_acknowledged(),
            Err(TransitionError::Missing {
                operation: "login acknowledged",
                prerequisite: "login success",
            })
        );
    }

    #[test]
    fn configuration_ack_requires_finish_packet() {
        let mut session = configuration_session();
        assert_eq!(
            session.configuration_acknowledged(),
            Err(TransitionError::Missing {
                operation: "configuration acknowledged",
                prerequisite: "finish configuration",
            })
        );
    }

    #[test]
    fn keep_alive_must_match() {
        let mut session = play_session();
        session.keep_alive_sent(7).unwrap();
        assert_eq!(
            session.keep_alive_response(8),
            Err(TransitionError::KeepAliveMismatch {
                expected: 7,
                received: 8,
            })
        );
        assert_eq!(session.snapshot().pending_keep_alive, Some(7));
    }

    #[test]
    fn packet_ids_can_repeat_across_phases() {
        let mut table = PacketTable::new();
        table.insert(PacketKind::StatusRequest, 0).unwrap();
        table.insert(PacketKind::LoginStart, 0).unwrap();
        table
            .insert(PacketKind::ConfigurationAcknowledged, 0)
            .unwrap();
        assert_eq!(
            table.resolve(ProtocolPhase::Login, PacketDirection::Serverbound, 0),
            Some(PacketKind::LoginStart)
        );
    }

    #[test]
    fn packet_ids_cannot_collide_inside_one_state_and_direction() {
        let mut table = PacketTable::new();
        table.insert(PacketKind::StatusRequest, 0).unwrap();
        assert!(matches!(
            table.insert(PacketKind::PingRequest, 0),
            Err(ProfileError::DuplicatePacketId { .. })
        ));
    }

    #[test]
    fn profile_owns_version_specific_packet_table() {
        let mut table = PacketTable::new();
        table.insert(PacketKind::Handshake, 0).unwrap();
        let profile = ProtocolProfile::new("Test Version", 900, table).unwrap();
        assert!(profile.supports(900));
        assert_eq!(profile.packets().require(PacketKind::Handshake).unwrap(), 0);
        assert_eq!(
            profile.packets().require(PacketKind::LoginStart),
            Err(ProfileError::MissingPacketId(PacketKind::LoginStart))
        );
    }

    #[test]
    fn invalid_username_does_not_mutate_session() {
        let mut session = ProtocolSession::new();
        session.handshake(1, HandshakeIntent::Login).unwrap();
        assert_eq!(
            session.login_start("bad name"),
            Err(TransitionError::InvalidUsernameCharacters)
        );
        assert_eq!(session.snapshot().username, None);
    }

    fn configuration_session() -> ProtocolSession {
        let mut session = ProtocolSession::new();
        session.handshake(1, HandshakeIntent::Login).unwrap();
        session.login_start("Steve").unwrap();
        session.login_success_sent().unwrap();
        session.login_acknowledged().unwrap();
        session
    }

    fn play_session() -> ProtocolSession {
        let mut session = configuration_session();
        session.finish_configuration_sent().unwrap();
        session.configuration_acknowledged().unwrap();
        session
    }
}

#[cfg(test)]
mod movement_packet_tests {
    use super::*;

    #[test]
    fn movement_and_chunk_view_packets_have_expected_metadata() {
        for kind in [
            PacketKind::ClientTickEnd,
            PacketKind::MovePlayerPosition,
            PacketKind::MovePlayerPositionRotation,
            PacketKind::MovePlayerRotation,
            PacketKind::MovePlayerStatusOnly,
            PacketKind::PlayerAction,
            PacketKind::UseItemOn,
        ] {
            assert_eq!(kind.phase(), ProtocolPhase::Play);
            assert_eq!(kind.direction(), PacketDirection::Serverbound);
        }
        assert_eq!(PacketKind::ForgetLevelChunk.phase(), ProtocolPhase::Play);
        assert_eq!(
            PacketKind::ForgetLevelChunk.direction(),
            PacketDirection::Clientbound
        );
        assert_eq!(PacketKind::BlockUpdate.phase(), ProtocolPhase::Play);
        assert_eq!(
            PacketKind::BlockUpdate.direction(),
            PacketDirection::Clientbound
        );
    }
}
