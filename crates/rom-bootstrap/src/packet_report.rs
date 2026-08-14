use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use rom_protocol::{
    PacketCatalog, PacketDescriptor, PacketDirection, ProtocolPhase, normalize_packet_name,
};
use serde_json::{Map, Value};

const MAX_PACKET_REPORT_BYTES: u64 = 32 * 1024 * 1024;

pub fn read_packet_report(path: &Path) -> Result<PacketCatalog> {
    if !path.is_file() {
        bail!(
            "packet report {} does not exist or is not a file",
            path.display()
        );
    }
    let metadata = fs::metadata(path)
        .with_context(|| format!("cannot stat packet report {}", path.display()))?;
    if metadata.len() == 0 || metadata.len() > MAX_PACKET_REPORT_BYTES {
        bail!(
            "packet report {} size {} is outside the supported range",
            path.display(),
            metadata.len()
        );
    }
    let bytes =
        fs::read(path).with_context(|| format!("cannot read packet report {}", path.display()))?;
    parse_packet_report(&bytes)
        .with_context(|| format!("cannot parse packet report {}", path.display()))
}

pub fn parse_packet_report(bytes: &[u8]) -> Result<PacketCatalog> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_PACKET_REPORT_BYTES {
        bail!("packet report size is outside the supported range");
    }
    let root: Value = serde_json::from_slice(bytes).context("packet report is not valid JSON")?;
    let root = root
        .as_object()
        .context("packet report root must be an object")?;
    let mut entries = Vec::new();
    for (phase, aliases) in phase_aliases() {
        let Some(phase_value) = find_object_value(root, aliases) else {
            continue;
        };
        let phase_object = phase_value
            .as_object()
            .with_context(|| format!("{phase:?} packet report section must be an object"))?;
        for (direction, direction_aliases) in direction_aliases() {
            let Some(direction_value) = find_object_value(phase_object, direction_aliases) else {
                continue;
            };
            let packets = packet_map(direction_value).with_context(|| {
                format!("{phase:?}/{direction:?} packet report section is invalid")
            })?;
            for (name, value) in packets {
                let id = packet_id(value).with_context(|| {
                    format!("packet {name} in {phase:?}/{direction:?} has no protocol_id")
                })?;
                entries.push(PacketDescriptor::new(
                    phase,
                    direction,
                    normalize_packet_name(name)?,
                    id,
                )?);
            }
        }
    }
    if entries.is_empty() {
        bail!("packet report did not contain any recognized protocol states");
    }
    PacketCatalog::new(entries).context("packet report contains conflicting packet records")
}

fn phase_aliases() -> [(ProtocolPhase, &'static [&'static str]); 5] {
    [
        (ProtocolPhase::Handshake, &["handshake", "handshaking"]),
        (ProtocolPhase::Status, &["status"]),
        (ProtocolPhase::Login, &["login"]),
        (ProtocolPhase::Configuration, &["configuration", "config"]),
        (ProtocolPhase::Play, &["play", "game"]),
    ]
}

fn direction_aliases() -> [(PacketDirection, &'static [&'static str]); 2] {
    [
        (
            PacketDirection::Serverbound,
            &["serverbound", "to_server", "toServer"],
        ),
        (
            PacketDirection::Clientbound,
            &["clientbound", "to_client", "toClient"],
        ),
    ]
}

fn find_object_value<'a>(object: &'a Map<String, Value>, aliases: &[&str]) -> Option<&'a Value> {
    aliases.iter().find_map(|alias| object.get(*alias))
}

fn packet_map(value: &Value) -> Result<&Map<String, Value>> {
    let object = value
        .as_object()
        .context("packet direction must be an object")?;
    if let Some(packets) = object.get("packets") {
        return packets
            .as_object()
            .context("packet direction packets field must be an object");
    }
    Ok(object)
}

fn packet_id(value: &Value) -> Result<i32> {
    let object = value
        .as_object()
        .context("packet record must be an object")?;
    let value = ["protocol_id", "protocolId", "id"]
        .iter()
        .find_map(|field| object.get(*field))
        .context("missing packet protocol ID")?;
    let id = value
        .as_i64()
        .context("packet protocol ID must be an integer")?;
    i32::try_from(id).context("packet protocol ID exceeds i32")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rom_protocol::PacketKind;

    #[test]
    fn parses_modern_mojang_packet_report_shape() {
        let report = br#"{
            "configuration": {
                "clientbound": {
                    "minecraft:registry_data": { "protocol_id": 7 }
                },
                "serverbound": {
                    "minecraft:client_information": { "protocol_id": 0 }
                }
            },
            "play": {
                "clientbound": {
                    "minecraft:system_chat": { "protocol_id": 121 },
                    "minecraft:container_set_content": { "protocol_id": 18 }
                },
                "serverbound": {
                    "minecraft:chat_command": { "protocol_id": 6 },
                    "minecraft:set_carried_item": { "protocol_id": 52 }
                }
            }
        }"#;
        let catalog = parse_packet_report(report).unwrap();
        assert_eq!(catalog.len(), 6);
        let typed = catalog.typed_table().unwrap();
        assert_eq!(typed.id(PacketKind::SystemChat), Some(121));
        assert_eq!(typed.id(PacketKind::ChatCommand), Some(6));
        assert_eq!(typed.id(PacketKind::SetCarriedItem), Some(52));
        assert_eq!(typed.id(PacketKind::SetContainerContent), Some(18));
    }

    #[test]
    fn supports_nested_packets_and_direction_aliases() {
        let report = br#"{
            "play": {
                "toServer": {
                    "packets": {
                        "chat_command": { "id": 4 }
                    }
                }
            }
        }"#;
        let catalog = parse_packet_report(report).unwrap();
        assert_eq!(catalog.entries()[0].name, "minecraft:chat_command");
        assert_eq!(catalog.entries()[0].id, 4);
    }

    #[test]
    fn rejects_duplicate_packet_ids() {
        let report = br#"{
            "play": {
                "clientbound": {
                    "minecraft:system_chat": { "protocol_id": 1 },
                    "minecraft:disconnect": { "protocol_id": 1 }
                }
            }
        }"#;
        assert!(parse_packet_report(report).is_err());
    }
}
