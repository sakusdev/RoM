from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-rompack/src/lib.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    "use ferrum_protocol::{PacketKind, PacketTable};",
    "use ferrum_protocol::{PacketCatalog, PacketDescriptor, PacketKind, PacketTable};",
    "packet catalog imports",
)
text = replace_once(
    text,
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 4;",
    "pub const ROMPACK_SCHEMA_VERSION: u32 = 5;",
    "schema version",
)
text = replace_once(
    text,
    '''    pub metadata: RomPackMetadata,
    pub packets: Vec<RomPackPacket>,
    pub world: RomPackWorld,''',
    '''    pub metadata: RomPackMetadata,
    /// Typed packet IDs understood by the current native runtime.
    pub packets: Vec<RomPackPacket>,
    /// Complete generated packet inventory, including packets not implemented yet.
    pub packet_catalog: Vec<PacketDescriptor>,
    pub world: RomPackWorld,''',
    "rompack catalog field",
)
text = replace_once(
    text,
    '''    pub packet_count: usize,
    pub registry_count: usize,''',
    '''    pub packet_count: usize,
    pub packet_catalog_count: usize,
    pub registry_count: usize,''',
    "summary catalog count",
)
text = replace_once(
    text,
    '''    if pack.packets.is_empty() {
        bail!("version pack does not contain a packet table");
    }
    if pack.packets.len() > limits.max_packets {
        bail!("version pack contains too many packet records");
    }
    let mut previous_packet = None;
    let mut packet_table = PacketTable::new();
    for packet in &pack.packets {
        if previous_packet.is_some_and(|previous| previous >= packet.kind) {
            bail!("version-pack packets must be strictly sorted and unique");
        }
        previous_packet = Some(packet.kind);
        packet_table
            .insert(packet.kind, packet.id)
            .with_context(|| format!("invalid packet record {:?}", packet.kind))?;
    }''',
    '''    if pack.packets.is_empty() {
        bail!("version pack does not contain a typed packet table");
    }
    if pack.packets.len() > limits.max_packets {
        bail!("version pack contains too many typed packet records");
    }
    if pack.packet_catalog.is_empty() {
        bail!("version pack does not contain a generated packet catalog");
    }
    if pack.packet_catalog.len() > limits.max_packets {
        bail!("version pack contains too many packet catalog records");
    }
    let catalog = PacketCatalog::new(pack.packet_catalog.clone())
        .context("invalid generated packet catalog")?;
    if catalog.entries() != pack.packet_catalog.as_slice() {
        bail!("version-pack packet catalog must be canonically sorted");
    }
    let catalog_table = catalog
        .typed_table()
        .context("cannot derive typed packet table from generated catalog")?;

    let mut previous_packet = None;
    let mut packet_table = PacketTable::new();
    for packet in &pack.packets {
        if previous_packet.is_some_and(|previous| previous >= packet.kind) {
            bail!("version-pack packets must be strictly sorted and unique");
        }
        previous_packet = Some(packet.kind);
        packet_table
            .insert(packet.kind, packet.id)
            .with_context(|| format!("invalid packet record {:?}", packet.kind))?;
        if catalog_table.id(packet.kind) != Some(packet.id) {
            bail!(
                "typed packet {:?} ID {} does not match the generated packet catalog",
                packet.kind,
                packet.id
            );
        }
    }''',
    "catalog validation",
)
text = replace_once(
    text,
    '''        packet_count: pack.packets.len(),
        registry_count: pack.registries.len(),''',
    '''        packet_count: pack.packets.len(),
        packet_catalog_count: pack.packet_catalog.len(),
        registry_count: pack.registries.len(),''',
    "summary catalog count value",
)
text = replace_once(
    text,
    '''            packets: vec![
                RomPackPacket {
                    kind: PacketKind::Handshake,
                    id: 0,
                },
                RomPackPacket {
                    kind: PacketKind::StatusRequest,
                    id: 0,
                },
                RomPackPacket {
                    kind: PacketKind::StatusResponse,
                    id: 0,
                },
            ],''',
    '''            packets: vec![
                RomPackPacket {
                    kind: PacketKind::Handshake,
                    id: 0,
                },
                RomPackPacket {
                    kind: PacketKind::StatusRequest,
                    id: 0,
                },
                RomPackPacket {
                    kind: PacketKind::StatusResponse,
                    id: 0,
                },
            ],
            packet_catalog: vec![
                PacketDescriptor::new(
                    ferrum_protocol::ProtocolPhase::Handshake,
                    ferrum_protocol::PacketDirection::Serverbound,
                    "minecraft:intention",
                    0,
                )
                .unwrap(),
                PacketDescriptor::new(
                    ferrum_protocol::ProtocolPhase::Status,
                    ferrum_protocol::PacketDirection::Serverbound,
                    "minecraft:status_request",
                    0,
                )
                .unwrap(),
                PacketDescriptor::new(
                    ferrum_protocol::ProtocolPhase::Status,
                    ferrum_protocol::PacketDirection::Clientbound,
                    "minecraft:status_response",
                    0,
                )
                .unwrap(),
            ],''',
    "sample catalog",
)
text = replace_once(
    text,
    '''        assert_eq!(written.packet_count, 3);
        assert_eq!(written.registry_count, 2);''',
    '''        assert_eq!(written.packet_count, 3);
        assert_eq!(written.packet_catalog_count, 3);
        assert_eq!(written.registry_count, 2);''',
    "catalog summary test",
)
text = replace_once(
    text,
    '''        let mut pack = sample_pack();
        pack.packets.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();''',
    '''        let mut pack = sample_pack();
        pack.packets.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();
        pack.packet_catalog.reverse();
        assert!(validate_rompack(&pack, RomPackLimits::default()).is_err());

        let mut pack = sample_pack();''',
    "unsorted catalog test",
)
path.write_text(text, encoding="utf-8")
