from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"replacement target not found: {label}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    '''        "loaded RoM version pack {} (SHA-256 {}, {} packets, data version {}, {} sections, {} registries / {} entries)",
        canonical.display(),
        summary.sha256,
        summary.packet_count,
        pack.world.data_version,''',
    '''        "loaded RoM version pack {} (SHA-256 {}, {} typed packets / {} catalog entries, data version {}, {} sections, {} registries / {} entries)",
        canonical.display(),
        summary.sha256,
        summary.packet_count,
        summary.packet_catalog_count,
        pack.world.data_version,''',
    "version pack catalog log",
)
text = replace_once(
    text,
    '''    let expected: BTreeSet<_> = PacketKind::ALL.iter().copied().collect();
    let actual: BTreeSet<_> = packets.iter().map(|packet| packet.kind).collect();
    if actual != expected || packets.len() != expected.len() {
        bail!(
            "version pack packet kinds do not match the runtime: expected {}, got {}",
            expected.len(),
            packets.len()
        );
    }

    let mut table = PacketTable::new();''',
    '''    let actual: BTreeSet<_> = packets.iter().map(|packet| packet.kind).collect();
    let missing = PacketKind::CORE
        .iter()
        .copied()
        .filter(|kind| !actual.contains(kind))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!("version pack is missing required core packet kinds: {missing:?}");
    }
    if actual.len() != packets.len() {
        bail!("version pack typed packet kinds are not unique");
    }

    let mut table = PacketTable::new();''',
    "core packet subset validation",
)
path.write_text(text, encoding="utf-8")
