use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{Context, Result, bail};
use ferrum_rompack::RomPackItem;
use serde_json::Value;

const MAX_REGISTRY_REPORT_BYTES: u64 = 64 * 1024 * 1024;

pub fn read_item_registry_report(path: &Path) -> Result<Vec<RomPackItem>> {
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
    parse_item_registry_report(&bytes)
        .with_context(|| format!("cannot parse registry report {}", path.display()))
}

pub fn parse_item_registry_report(bytes: &[u8]) -> Result<Vec<RomPackItem>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_REGISTRY_REPORT_BYTES {
        bail!("registry report size is outside the supported range");
    }
    let root: Value = serde_json::from_slice(bytes).context("registry report is not valid JSON")?;
    let entries = root
        .get("minecraft:item")
        .and_then(|registry| registry.get("entries"))
        .and_then(Value::as_object)
        .context("registry report is missing minecraft:item.entries")?;
    if entries.is_empty() {
        bail!("minecraft:item registry is empty");
    }

    let mut protocol_ids = BTreeSet::new();
    let mut items = Vec::with_capacity(entries.len());
    for (item, record) in entries {
        let protocol_id = record
            .get("protocol_id")
            .or_else(|| record.get("protocolId"))
            .and_then(Value::as_i64)
            .with_context(|| format!("item {item} is missing an integer protocol_id"))?;
        let protocol_id = i32::try_from(protocol_id)
            .with_context(|| format!("item {item} protocol_id exceeds i32"))?;
        if protocol_id < 0 {
            bail!("item {item} protocol_id cannot be negative");
        }
        if !protocol_ids.insert(protocol_id) {
            bail!("duplicate item protocol_id {protocol_id}");
        }
        items.push(RomPackItem {
            item: item.clone(),
            protocol_id,
        });
    }
    items.sort_by(|left, right| left.item.cmp(&right.item));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_item_protocol_ids() {
        let report = br#"{
            "minecraft:item": {
                "entries": {
                    "minecraft:stone": { "protocol_id": 1 },
                    "minecraft:air": { "protocol_id": 0 }
                }
            }
        }"#;
        let items = parse_item_registry_report(report).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].item, "minecraft:air");
        assert_eq!(items[0].protocol_id, 0);
        assert_eq!(items[1].item, "minecraft:stone");
        assert_eq!(items[1].protocol_id, 1);
    }

    #[test]
    fn rejects_duplicate_item_protocol_ids() {
        let report = br#"{
            "minecraft:item": {
                "entries": {
                    "minecraft:air": { "protocol_id": 0 },
                    "minecraft:stone": { "protocol_id": 0 }
                }
            }
        }"#;
        assert!(parse_item_registry_report(report).is_err());
    }
}
