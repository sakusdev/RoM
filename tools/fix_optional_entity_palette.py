from pathlib import Path


def one(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected 1, found {count}")
    return text.replace(old, new, 1)


path = Path("crates/rom-bootstrap/src/extract.rs")
text = path.read_text()
text = one(
    text,
    """    Ok(RegistryProtocolReport {
        items: Vec::new(),
        data_components: Vec::new(),
    })
""",
    """    Ok(RegistryProtocolReport {
        items: Vec::new(),
        entity_types: Vec::new(),
        data_components: Vec::new(),
    })
""",
    "registry fallback entity palette",
)
path.write_text(text)

path = Path("crates/ferrum-rompack/src/lib.rs")
text = path.read_text()
text = one(
    text,
    """    if pack.entity_types.is_empty() {
        bail!("version pack does not contain an entity-type registry");
    }
""",
    "",
    "optional entity palette",
)
text = one(
    text,
    """    if !pack
        .entity_types
        .iter()
        .any(|entity_type| entity_type.entity_type == "minecraft:player")
    {
        bail!("version pack entity-type registry is missing minecraft:player");
    }
""",
    """    if !pack.entity_types.is_empty()
        && !pack
            .entity_types
            .iter()
            .any(|entity_type| entity_type.entity_type == "minecraft:player")
    {
        bail!("non-empty version-pack entity-type registry is missing minecraft:player");
    }
""",
    "conditional player entity validation",
)
path.write_text(text)

path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text()
text = one(
    text,
    """    if entity_protocol_ids
        .protocol_id("minecraft:player")
        .is_none()
    {
        bail!("version pack entity protocol registry is missing minecraft:player");
    }
""",
    """    if !pack.entity_types.is_empty()
        && entity_protocol_ids
            .protocol_id("minecraft:player")
            .is_none()
    {
        bail!("non-empty version pack entity protocol registry is missing minecraft:player");
    }
""",
    "conditional server player entity validation",
)
path.write_text(text)
