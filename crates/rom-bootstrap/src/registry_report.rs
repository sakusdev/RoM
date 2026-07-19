use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{Context, Result, bail};
use ferrum_rompack::{
    RomPackDataComponent, RomPackEntityType, RomPackItem, RomPackProtocolEntry,
    RomPackProtocolRegistry,
};
use serde_json::Value;

const MAX_REGISTRY_REPORT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryProtocolReport {
    pub items: Vec<RomPackItem>,
    pub entity_types: Vec<RomPackEntityType>,
    pub data_components: Vec<RomPackDataComponent>,
    pub protocol_registries: Vec<RomPackProtocolRegistry>,
}

pub fn read_registry_protocol_report(path: &Path) -> Result<RegistryProtocolReport> {
    if !path.is_file() {
        bail!(
            "registry report {} does not exist or is not a file",
            path.display()
        );
    }
    let metadata = fs::metadata(path)
        .with_context(|| format!("cannot stat registry report {}", path.display()))?;
    if metadata.len() == 0 || metadata.len() > MAX_REGISTRY_REPORT_BYTES {
        bail!(
            "registry report {} size {} is outside the supported range",
            path.display(),
            metadata.len()
        );
    }
    let bytes = fs::read(path)
        .with_context(|| format!("cannot read registry report {}", path.display()))?;
    parse_registry_protocol_report(&bytes)
        .with_context(|| format!("cannot parse registry report {}", path.display()))
}

#[allow(dead_code)]
pub fn read_item_registry_report(path: &Path) -> Result<Vec<RomPackItem>> {
    Ok(read_registry_protocol_report(path)?.items)
}

pub fn parse_registry_protocol_report(bytes: &[u8]) -> Result<RegistryProtocolReport> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_REGISTRY_REPORT_BYTES {
        bail!("registry report size is outside the supported range");
    }
    let root: Value = serde_json::from_slice(bytes).context("registry report is not valid JSON")?;
    let items = parse_registry(&root, "minecraft:item")?
        .into_iter()
        .map(|(item, protocol_id)| RomPackItem { item, protocol_id })
        .collect();
    let entity_types = parse_registry(&root, "minecraft:entity_type")?
        .into_iter()
        .map(|(entity_type, protocol_id)| RomPackEntityType {
            entity_type,
            protocol_id,
        })
        .collect();
    let data_components = parse_registry(&root, "minecraft:data_component_type")?
        .into_iter()
        .map(|(component, protocol_id)| RomPackDataComponent {
            component,
            protocol_id,
        })
        .collect();
    let protocol_registries = parse_protocol_registries(&root)?;
    Ok(RegistryProtocolReport {
        items,
        entity_types,
        data_components,
        protocol_registries,
    })
}

#[allow(dead_code)]
pub fn parse_item_registry_report(bytes: &[u8]) -> Result<Vec<RomPackItem>> {
    Ok(parse_registry_protocol_report(bytes)?.items)
}

fn parse_registry(root: &Value, registry_id: &str) -> Result<Vec<(String, i32)>> {
    let entries = root
        .get(registry_id)
        .and_then(|registry| registry.get("entries"))
        .and_then(Value::as_object)
        .with_context(|| format!("registry report is missing {registry_id}.entries"))?;
    if entries.is_empty() {
        bail!("{registry_id} registry is empty");
    }
    let mut protocol_ids = BTreeSet::new();
    let mut values = Vec::with_capacity(entries.len());
    for (name, record) in entries {
        let protocol_id = record
            .get("protocol_id")
            .or_else(|| record.get("protocolId"))
            .and_then(Value::as_i64)
            .with_context(|| {
                format!("{registry_id} entry {name} is missing an integer protocol_id")
            })?;
        let protocol_id = i32::try_from(protocol_id)
            .with_context(|| format!("{registry_id} entry {name} protocol_id exceeds i32"))?;
        if protocol_id < 0 {
            bail!("{registry_id} entry {name} protocol_id cannot be negative");
        }
        if !protocol_ids.insert(protocol_id) {
            bail!("duplicate {registry_id} protocol_id {protocol_id}");
        }
        values.push((name.clone(), protocol_id));
    }
    values.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(values)
}

fn parse_protocol_registries(root: &Value) -> Result<Vec<RomPackProtocolRegistry>> {
    let registries = root
        .as_object()
        .context("registry report root must be an object")?;
    let mut result = Vec::with_capacity(registries.len());
    for registry_id in registries.keys() {
        let entries = parse_registry(root, registry_id)?
            .into_iter()
            .map(|(id, protocol_id)| RomPackProtocolEntry { id, protocol_id })
            .collect();
        result.push(RomPackProtocolRegistry {
            id: registry_id.clone(),
            entries,
        });
    }
    result.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &[u8] = br#"{
        "minecraft:item": {
            "entries": {
                "minecraft:stone": { "protocol_id": 1 },
                "minecraft:air": { "protocol_id": 0 }
            }
        },
        "minecraft:entity_type": {
            "entries": {
                "minecraft:item": { "protocol_id": 2 },
                "minecraft:player": { "protocol_id": 147 }
            }
        },
        "minecraft:data_component_type": {
            "entries": {
                "minecraft:custom_name": { "protocol_id": 7 },
                "minecraft:damage": { "protocol_id": 3 }
            }
        }
    }"#;

    #[test]
    fn parses_item_and_component_protocol_ids() {
        let report = parse_registry_protocol_report(REPORT).unwrap();
        assert_eq!(report.items[0].item, "minecraft:air");
        assert_eq!(report.items[1].protocol_id, 1);
        assert_eq!(report.entity_types[0].entity_type, "minecraft:item");
        assert_eq!(report.entity_types[1].protocol_id, 147);
        assert_eq!(report.data_components[0].component, "minecraft:custom_name");
        assert_eq!(report.data_components[1].protocol_id, 3);
        assert_eq!(report.protocol_registries.len(), 3);
        assert_eq!(
            report.protocol_registries[2].id,
            "minecraft:item"
        );
        assert_eq!(report.protocol_registries[2].entries[0].protocol_id, 0);
    }

    #[test]
    fn legacy_item_parser_uses_combined_report() {
        assert_eq!(parse_item_registry_report(REPORT).unwrap().len(), 2);
    }
}
