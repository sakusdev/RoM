//! Built-in protocol metadata for Minecraft Java Edition 26.1.2.
//!
//! This crate contains only public wire metadata and the vanilla known-pack
//! declaration required during Configuration. World registries and gameplay
//! data remain separate so they can be generated and tested independently.

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
    ] {
        packets.insert(kind, id)?;
    }
    ProtocolProfile::new(VERSION_NAME, PROTOCOL_VERSION, packets)
}

/// Packs bundled into a matching vanilla 26.1.2 client.
#[must_use]
pub fn known_packs() -> Vec<KnownPack> {
    vec![KnownPack::new("minecraft", "core", PROFILE_NAME)]
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
    }

    #[test]
    fn advertises_the_vanilla_core_pack() {
        assert_eq!(
            known_packs(),
            vec![KnownPack::new("minecraft", "core", "26.1.2")]
        );
    }
}
