//! Built-in protocol metadata for Minecraft Java Edition 26.1.2.
//!
//! This crate contains public wire metadata, the vanilla known-pack declaration,
//! and the generated synchronized-registry manifest required during Configuration.
//! Gameplay state remains separate from version data.

pub mod entity_metadata;
mod registries;
mod tags;

pub use registries::{
    OFFICIAL_SERVER_SHA1, REGISTRY_COUNT, REGISTRY_ENTRY_COUNT, REGISTRY_MANIFEST_SHA256,
    RegistryManifest, SYNCHRONIZED_REGISTRIES, configuration_registries,
};
pub use tags::{TAG_COUNT, TAG_ENTRY_COUNT, TAG_MANIFEST_SHA256, TAG_REGISTRY_COUNT};

use rom_configuration::{KnownPack, TagRegistry};
use rom_protocol::{PacketKind, PacketTable, ProfileError, ProtocolProfile};

pub const PROFILE_NAME: &str = "26.1.2";
pub const VERSION_NAME: &str = "Minecraft Java Edition 26.1.2";
pub const PROTOCOL_VERSION: i32 = 775;
pub const WORLD_VERSION: i32 = 4_790;
pub const VANILLA_FEATURE: &str = "minecraft:vanilla";
pub const OVERWORLD_MIN_SECTION_Y: i32 = -4;
pub const OVERWORLD_SECTION_COUNT: usize = 24;
pub const AIR_BLOCK_STATE_ID: u32 = 0;
pub const STONE_BLOCK_STATE_ID: u32 = 1;
pub const GRASS_BLOCK_STATE_ID: u32 = 9;
pub const DIRT_BLOCK_STATE_ID: u32 = 10;
pub const BEDROCK_BLOCK_STATE_ID: u32 = 85;
pub const PLAINS_BIOME_ID: u32 = 40;
pub const OVERWORLD_DIMENSION: &str = "minecraft:overworld";
pub const OVERWORLD_DIMENSION_TYPE_ID: i32 = 0;
pub const OVERWORLD_SEA_LEVEL: i32 = 63;
pub const FLAT_WORLD_FLOOR_Y: i32 = 63;
pub const FLAT_WORLD_SPAWN_X: i32 = 0;
pub const FLAT_WORLD_SPAWN_Z: i32 = 0;

/// Number of tag registries that are valid in the client Configuration registry access.
pub const NETWORK_TAG_REGISTRY_COUNT: usize = 16;
/// Number of tags sent after filtering server-only world-generation registries.
pub const NETWORK_TAG_COUNT: usize = 734;
/// Number of protocol registry ID references sent in the filtered tag payload.
pub const NETWORK_TAG_ENTRY_COUNT: usize = 8_132;

const SERVER_ONLY_TAG_REGISTRIES: &[&str] = &[
    "minecraft:worldgen/configured_feature",
    "minecraft:worldgen/flat_level_generator_preset",
    "minecraft:worldgen/structure",
    "minecraft:worldgen/world_preset",
];

/// Build the tag registries that the 26.1.2 client can resolve during Configuration.
///
/// The generated source manifest also contains server data-pack tag registries. Those
/// registries are not synchronized through Registry Data and must not be included in
/// `ClientboundUpdateTagsPacket`, otherwise the client fails at Finish Configuration.
#[must_use]
pub fn configuration_tags() -> Vec<TagRegistry> {
    tags::configuration_tags()
        .into_iter()
        .filter(|registry| !SERVER_ONLY_TAG_REGISTRIES.contains(&registry.id.as_str()))
        .collect()
}

/// Construct the exact packet table used by Minecraft Java Edition 26.1.2.
pub fn protocol_profile() -> Result<ProtocolProfile, ProfileError> {
    let mut packets = PacketTable::new();
    for (kind, id) in [
        (PacketKind::Handshake, 0x00),
        (PacketKind::StatusRequest, 0x00),
        (PacketKind::StatusResponse, 0x00),
        (PacketKind::PingRequest, 0x01),
        (PacketKind::PongResponse, 0x01),
        (PacketKind::LoginStart, 0x00),
        (PacketKind::LoginDisconnect, 0x00),
        (PacketKind::LoginSuccess, 0x02),
        (PacketKind::LoginAcknowledged, 0x03),
        (PacketKind::ConfigurationClientInformation, 0x00),
        (PacketKind::ConfigurationAcknowledged, 0x03),
        (PacketKind::ConfigurationDisconnect, 0x02),
        (PacketKind::FinishConfiguration, 0x03),
        (PacketKind::RegistryData, 0x07),
        (PacketKind::FeatureFlags, 0x0c),
        (PacketKind::UpdateTags, 0x0d),
        (PacketKind::SelectKnownPacksRequest, 0x0e),
        (PacketKind::SelectKnownPacksResponse, 0x07),
        (PacketKind::PlayLogin, 0x31),
        (PacketKind::ChunkBatchStart, 0x0c),
        (PacketKind::ChunkBatchFinished, 0x0b),
        (PacketKind::ChunkBatchReceived, 0x0b),
        (PacketKind::LevelChunkWithLight, 0x2d),
        (PacketKind::SetChunkCacheCenter, 0x5e),
        (PacketKind::DefaultSpawnPosition, 0x61),
        (PacketKind::PlayerPosition, 0x48),
        (PacketKind::SystemChat, 0x79),
        (PacketKind::SetHealth, 0x68),
        (PacketKind::HurtAnimation, 0x2a),
        (PacketKind::PlayerCombatKill, 0x44),
        (PacketKind::Respawn, 0x52),
        (PacketKind::AcceptTeleportation, 0x00),
        (PacketKind::PlayDisconnect, 0x20),
        (PacketKind::KeepAliveRequest, 0x2c),
        (PacketKind::KeepAliveResponse, 0x1c),
        (PacketKind::ClientTickEnd, 0x0d),
        (PacketKind::ClientCommand, 0x0c),
        (PacketKind::MovePlayerPosition, 0x1e),
        (PacketKind::MovePlayerPositionRotation, 0x1f),
        (PacketKind::MovePlayerRotation, 0x20),
        (PacketKind::MovePlayerStatusOnly, 0x21),
        (PacketKind::PlayerAction, 0x29),
        (PacketKind::UseItemOn, 0x42),
        (PacketKind::BlockChangedAck, 0x04),
        (PacketKind::BlockUpdate, 0x08),
        (PacketKind::ForgetLevelChunk, 0x25),
    ] {
        packets.insert(kind, id)?;
    }
    ProtocolProfile::new(VERSION_NAME, PROTOCOL_VERSION, packets)
}

/// The vanilla core pack required to omit inline registry NBT.
#[must_use]
pub fn vanilla_core_pack() -> KnownPack {
    KnownPack::new("minecraft", "core", PROFILE_NAME)
}

/// Packs bundled into a matching vanilla 26.1.2 client.
#[must_use]
pub fn known_packs() -> Vec<KnownPack> {
    vec![vanilla_core_pack()]
}

#[must_use]
pub fn accepts_vanilla_core_pack(accepted: &[KnownPack]) -> bool {
    accepted.contains(&vanilla_core_pack())
}

#[must_use]
pub fn default_features() -> Vec<String> {
    vec![VANILLA_FEATURE.to_owned()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_expected_protocol_identity() {
        let profile = protocol_profile().unwrap();
        assert_eq!(profile.version_name(), VERSION_NAME);
        assert_eq!(profile.protocol_number(), PROTOCOL_VERSION);
        assert!(profile.supports(775));
        assert!(!profile.supports(774));
    }

    #[test]
    fn uses_the_documented_configuration_packet_ids() {
        let profile = protocol_profile().unwrap();
        let packets = profile.packets();
        assert_eq!(
            packets.require(PacketKind::LoginAcknowledged).unwrap(),
            0x03
        );
        assert_eq!(
            packets
                .require(PacketKind::ConfigurationClientInformation)
                .unwrap(),
            0x00
        );
        assert_eq!(
            packets.require(PacketKind::FinishConfiguration).unwrap(),
            0x03
        );
        assert_eq!(packets.require(PacketKind::RegistryData).unwrap(), 0x07);
        assert_eq!(packets.require(PacketKind::FeatureFlags).unwrap(), 0x0c);
        assert_eq!(packets.require(PacketKind::UpdateTags).unwrap(), 0x0d);
        assert_eq!(packets.require(PacketKind::SetHealth).unwrap(), 0x68);
        assert_eq!(packets.require(PacketKind::HurtAnimation).unwrap(), 0x2a);
        assert_eq!(packets.require(PacketKind::PlayerCombatKill).unwrap(), 0x44);
        assert_eq!(packets.require(PacketKind::Respawn).unwrap(), 0x52);
        assert_eq!(packets.require(PacketKind::ClientCommand).unwrap(), 0x0c);
        assert_eq!(packets.require(PacketKind::ChunkBatchStart).unwrap(), 0x0c);
        assert_eq!(
            packets.require(PacketKind::ChunkBatchFinished).unwrap(),
            0x0b
        );
        assert_eq!(
            packets.require(PacketKind::ChunkBatchReceived).unwrap(),
            0x0b
        );
        assert_eq!(
            packets.require(PacketKind::LevelChunkWithLight).unwrap(),
            0x2d
        );
        assert_eq!(
            packets.require(PacketKind::SetChunkCacheCenter).unwrap(),
            0x5e
        );
        assert_eq!(packets.require(PacketKind::SystemChat).unwrap(), 0x79);
        assert_eq!(
            packets
                .require(PacketKind::SelectKnownPacksRequest)
                .unwrap(),
            0x0e
        );
        assert_eq!(
            packets
                .require(PacketKind::SelectKnownPacksResponse)
                .unwrap(),
            0x07
        );
        assert_eq!(packets.require(PacketKind::PlayLogin).unwrap(), 0x31);
        assert_eq!(
            packets.require(PacketKind::DefaultSpawnPosition).unwrap(),
            0x61
        );
        assert_eq!(packets.require(PacketKind::PlayerPosition).unwrap(), 0x48);
        assert_eq!(
            packets.require(PacketKind::AcceptTeleportation).unwrap(),
            0x00
        );
        assert_eq!(packets.require(PacketKind::KeepAliveRequest).unwrap(), 0x2c);
        assert_eq!(
            packets.require(PacketKind::KeepAliveResponse).unwrap(),
            0x1c
        );
        assert_eq!(packets.require(PacketKind::ClientTickEnd).unwrap(), 0x0d);
        assert_eq!(
            packets.require(PacketKind::MovePlayerPosition).unwrap(),
            0x1e
        );
        assert_eq!(
            packets
                .require(PacketKind::MovePlayerPositionRotation)
                .unwrap(),
            0x1f
        );
        assert_eq!(
            packets.require(PacketKind::MovePlayerRotation).unwrap(),
            0x20
        );
        assert_eq!(
            packets.require(PacketKind::MovePlayerStatusOnly).unwrap(),
            0x21
        );
        assert_eq!(packets.require(PacketKind::PlayerAction).unwrap(), 0x29);
        assert_eq!(packets.require(PacketKind::UseItemOn).unwrap(), 0x42);
        assert_eq!(packets.require(PacketKind::BlockChangedAck).unwrap(), 0x04);
        assert_eq!(packets.require(PacketKind::BlockUpdate).unwrap(), 0x08);
        assert_eq!(packets.require(PacketKind::ForgetLevelChunk).unwrap(), 0x25);
    }

    #[test]
    fn advertises_the_vanilla_core_pack() {
        assert_eq!(
            known_packs(),
            vec![KnownPack::new("minecraft", "core", "26.1.2")]
        );
    }

    #[test]
    fn exposes_the_complete_synchronized_registry_manifest() {
        use std::collections::BTreeSet;

        assert_eq!(SYNCHRONIZED_REGISTRIES.len(), REGISTRY_COUNT);
        assert_eq!(
            SYNCHRONIZED_REGISTRIES
                .iter()
                .map(|registry| registry.entries.len())
                .sum::<usize>(),
            REGISTRY_ENTRY_COUNT
        );
        assert_eq!(REGISTRY_COUNT, 28);
        assert_eq!(REGISTRY_ENTRY_COUNT, 382);
        assert_eq!(
            SYNCHRONIZED_REGISTRIES.first().unwrap().id,
            "minecraft:worldgen/biome"
        );
        assert_eq!(
            SYNCHRONIZED_REGISTRIES.last().unwrap().id,
            "minecraft:timeline"
        );

        let registry_ids: BTreeSet<_> = SYNCHRONIZED_REGISTRIES
            .iter()
            .map(|registry| registry.id)
            .collect();
        assert_eq!(registry_ids.len(), REGISTRY_COUNT);
        for registry in SYNCHRONIZED_REGISTRIES {
            let entries: BTreeSet<_> = registry.entries.iter().copied().collect();
            assert_eq!(entries.len(), registry.entries.len(), "{}", registry.id);
        }
    }

    #[test]
    fn configuration_tags_exclude_server_only_worldgen_registries() {
        let registries = configuration_tags();
        assert_eq!(registries.len(), NETWORK_TAG_REGISTRY_COUNT);
        assert_eq!(
            registries
                .iter()
                .map(|registry| registry.tags.len())
                .sum::<usize>(),
            NETWORK_TAG_COUNT
        );
        assert_eq!(
            registries
                .iter()
                .flat_map(|registry| &registry.tags)
                .map(|tag| tag.entries.len())
                .sum::<usize>(),
            NETWORK_TAG_ENTRY_COUNT
        );
        for id in SERVER_ONLY_TAG_REGISTRIES {
            assert!(
                !registries.iter().any(|registry| registry.id == *id),
                "{id}"
            );
        }
        assert!(
            registries
                .iter()
                .any(|registry| registry.id == "minecraft:worldgen/biome")
        );
    }
}
