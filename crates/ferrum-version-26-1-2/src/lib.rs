//! Built-in protocol metadata for Minecraft Java Edition 26.1.2.
//!
//! This crate contains public wire metadata, the vanilla known-pack declaration,
//! and the generated synchronized-registry manifest required during Configuration.
//! Gameplay state remains separate from version data.

mod registries;

pub use registries::{
    OFFICIAL_SERVER_SHA1, REGISTRY_COUNT, REGISTRY_ENTRY_COUNT, REGISTRY_MANIFEST_SHA256,
    RegistryManifest, SYNCHRONIZED_REGISTRIES, configuration_registries,
};

use ferrum_configuration::KnownPack;
use ferrum_protocol::{PacketKind, PacketTable, ProfileError, ProtocolProfile};

pub const PROFILE_NAME: &str = "26.1.2";
pub const VERSION_NAME: &str = "Minecraft Java Edition 26.1.2";
pub const PROTOCOL_VERSION: i32 = 775;
pub const WORLD_VERSION: i32 = 4_790;
pub const VANILLA_FEATURE: &str = "minecraft:vanilla";

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
        (PacketKind::ConfigurationAcknowledged, 0x03),
        (PacketKind::ConfigurationDisconnect, 0x02),
        (PacketKind::FinishConfiguration, 0x03),
        (PacketKind::RegistryData, 0x07),
        (PacketKind::FeatureFlags, 0x0c),
        (PacketKind::UpdateTags, 0x0d),
        (PacketKind::SelectKnownPacksRequest, 0x0e),
        (PacketKind::SelectKnownPacksResponse, 0x07),
        (PacketKind::PlayLogin, 0x31),
        (PacketKind::DefaultSpawnPosition, 0x61),
        (PacketKind::PlayerPosition, 0x48),
        (PacketKind::AcceptTeleportation, 0x00),
        (PacketKind::PlayDisconnect, 0x20),
        (PacketKind::KeepAliveRequest, 0x2c),
        (PacketKind::KeepAliveResponse, 0x1c),
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
            packets.require(PacketKind::FinishConfiguration).unwrap(),
            0x03
        );
        assert_eq!(packets.require(PacketKind::RegistryData).unwrap(), 0x07);
        assert_eq!(packets.require(PacketKind::FeatureFlags).unwrap(), 0x0c);
        assert_eq!(packets.require(PacketKind::UpdateTags).unwrap(), 0x0d);
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
    fn registry_packets_reference_the_accepted_core_pack() {
        let registries = configuration_registries();
        assert_eq!(registries.len(), REGISTRY_COUNT);
        assert!(
            registries
                .iter()
                .all(|registry| { registry.entries.iter().all(|entry| entry.value.is_none()) })
        );
        assert!(accepts_vanilla_core_pack(&known_packs()));
        assert!(!accepts_vanilla_core_pack(&[]));
    }
}
