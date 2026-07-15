use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
};
use zip::ZipArchive;

const MAX_ARCHIVE_ENTRIES: usize = 250_000;
const MAX_JSON_ENTRY_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct FabricReport {
    pub schema_version: u32,
    pub source_jar: String,
    pub metadata: Option<FabricMetadataReport>,
    pub access_wideners: Vec<String>,
    pub mixin_configs: Vec<MixinConfigReport>,
    pub nested_jars: Vec<String>,
    pub warnings: Vec<FabricWarning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FabricMetadataReport {
    pub mod_id: Option<String>,
    pub version: Option<String>,
    pub environment: Option<String>,
    pub entrypoints: Vec<String>,
    pub dependencies: Vec<String>,
    pub mixins: Vec<String>,
    pub jars: Vec<String>,
    pub access_widener: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MixinConfigReport {
    pub path: String,
    pub package: Option<String>,
    pub compatibility_level: Option<String>,
    pub mixins: Vec<MixinReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MixinReport {
    pub class: String,
    pub target_class: Option<String>,
    pub classification: MixinClassification,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MixinClassification {
    Supported,
    PartiallySupported,
    SemanticRewriteRequired,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct FabricWarning {
    pub code: &'static str,
    pub message: String,
}

pub fn inspect_fabric_jar(path: &Path) -> Result<FabricReport> {
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut archive =
        ZipArchive::new(file).with_context(|| format!("cannot read JAR {}", path.display()))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        anyhow::bail!(
            "JAR {} contains {} entries, exceeding limit {MAX_ARCHIVE_ENTRIES}",
            path.display(),
            archive.len()
        );
    }
    let mut warnings = Vec::new();
    let metadata = read_fabric_metadata(&mut archive, &mut warnings)?;
    let mut mixin_paths = metadata
        .as_ref()
        .map(|metadata| metadata.mixins.clone())
        .unwrap_or_default();
    let mut access_wideners = metadata
        .as_ref()
        .and_then(|metadata| metadata.access_widener.clone())
        .into_iter()
        .collect::<Vec<_>>();
    let nested_jars = metadata
        .as_ref()
        .map(|metadata| metadata.jars.clone())
        .unwrap_or_default();

    discover_fabric_entries(
        &mut archive,
        &mut mixin_paths,
        &mut access_wideners,
        &mut warnings,
    );
    mixin_paths.sort();
    mixin_paths.dedup();
    access_wideners.sort();
    access_wideners.dedup();

    let mut mixin_configs = Vec::new();
    for mixin_path in mixin_paths {
        match read_json_entry(&mut archive, &mixin_path) {
            Ok(Some(value)) => mixin_configs.push(parse_mixin_config(&mixin_path, &value)),
            Ok(None) => warnings.push(FabricWarning {
                code: "mixin_config_missing",
                message: format!("mixin config {mixin_path} was declared but not found"),
            }),
            Err(error) => warnings.push(FabricWarning {
                code: "mixin_config_parse_failed",
                message: format!("cannot parse {mixin_path}: {error}"),
            }),
        }
    }

    Ok(FabricReport {
        schema_version: 1,
        source_jar: path.to_string_lossy().into_owned(),
        metadata,
        access_wideners,
        mixin_configs,
        nested_jars,
        warnings,
    })
}

fn read_fabric_metadata<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    warnings: &mut Vec<FabricWarning>,
) -> Result<Option<FabricMetadataReport>> {
    let Some(value) = read_json_entry(archive, "fabric.mod.json")? else {
        return Ok(None);
    };
    let metadata = FabricMetadataReport {
        mod_id: string_field(&value, "id"),
        version: string_field(&value, "version"),
        environment: string_field(&value, "environment"),
        entrypoints: object_keys(value.get("entrypoints")),
        dependencies: object_keys(value.get("depends")),
        mixins: string_or_array_field(&value, "mixins", warnings),
        jars: jars_field(&value),
        access_widener: string_field(&value, "accessWidener"),
    };
    Ok(Some(metadata))
}

fn discover_fabric_entries<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    mixin_paths: &mut Vec<String>,
    access_wideners: &mut Vec<String>,
    warnings: &mut Vec<FabricWarning>,
) {
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            warnings.push(FabricWarning {
                code: "archive_entry_unreadable",
                message: format!("cannot inspect archive entry #{index}"),
            });
            continue;
        };
        let name = entry.name().to_owned();
        if name.ends_with(".mixins.json") || name.ends_with(".mixin.json") {
            mixin_paths.push(name.clone());
        }
        if name.ends_with(".accesswidener") {
            access_wideners.push(name);
        }
    }
}

fn read_json_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Option<Value>> {
    let Ok(entry) = archive.by_name(name) else {
        return Ok(None);
    };
    let expected_size = entry.size();
    if expected_size > MAX_JSON_ENTRY_BYTES {
        anyhow::bail!("{name} exceeds the {MAX_JSON_ENTRY_BYTES}-byte JSON limit");
    }
    let mut bytes = Vec::with_capacity(expected_size as usize);
    entry
        .take(MAX_JSON_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("cannot read {name}"))?;
    if bytes.len() as u64 != expected_size || bytes.len() as u64 > MAX_JSON_ENTRY_BYTES {
        anyhow::bail!("{name} changed size while reading or exceeds the JSON limit");
    }
    let text = String::from_utf8(bytes).with_context(|| format!("{name} is not UTF-8"))?;
    serde_json::from_str(&text)
        .map(Some)
        .with_context(|| format!("{name} is not valid JSON"))
}

fn parse_mixin_config(path: &str, value: &Value) -> MixinConfigReport {
    let package = string_field(value, "package");
    let compatibility_level = string_field(value, "compatibilityLevel");
    let mut mixins = Vec::new();
    for field in ["mixins", "client", "server"] {
        if let Some(values) = value.get(field).and_then(Value::as_array) {
            for mixin in values {
                if let Some(class) = mixin.as_str() {
                    let qualified = match package.as_deref() {
                        Some(package) if !class.contains('.') => format!("{package}.{class}"),
                        _ => class.to_owned(),
                    };
                    mixins.push(classify_mixin(&qualified, value));
                }
            }
        }
    }
    mixins.sort_by(|left, right| left.class.cmp(&right.class));
    MixinConfigReport {
        path: path.to_owned(),
        package,
        compatibility_level,
        mixins,
    }
}

fn classify_mixin(class: &str, config: &Value) -> MixinReport {
    let mut reasons = Vec::new();
    let target_class = config
        .get("target")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let lower = class.to_ascii_lowercase();
    let classification = if lower.contains("redirect")
        || lower.contains("modifyvariable")
        || lower.contains("modifyconstant")
    {
        reasons
            .push("redirect or expression/local mutation needs bytecode-level rewrite".to_owned());
        MixinClassification::Unsupported
    } else if lower.contains("head") || lower.contains("return") {
        reasons.push(
            "HEAD/RETURN style hook can become an explicit event after target review".to_owned(),
        );
        MixinClassification::Supported
    } else if lower.contains("accessor") || lower.contains("invoker") {
        reasons.push("accessor/invoker can become a generated facade".to_owned());
        MixinClassification::PartiallySupported
    } else if lower.contains("hook") || lower.contains("inject") {
        reasons.push(
            "simple hook-like mixin can map to explicit server events after review".to_owned(),
        );
        MixinClassification::SemanticRewriteRequired
    } else {
        reasons.push(
            "mixin requires review; no injection metadata is available in config alone".to_owned(),
        );
        MixinClassification::SemanticRewriteRequired
    };
    MixinReport {
        class: class.to_owned(),
        target_class,
        classification,
        reasons,
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn object_keys(value: Option<&Value>) -> Vec<String> {
    let mut keys = value
        .and_then(Value::as_object)
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    keys
}

fn string_or_array_field(
    value: &Value,
    field: &str,
    warnings: &mut Vec<FabricWarning>,
) -> Vec<String> {
    let Some(value) = value.get(field) else {
        return Vec::new();
    };
    let mut values = match value {
        Value::String(value) => vec![value.clone()],
        Value::Array(values) => values
            .iter()
            .filter_map(|value| match value {
                Value::String(value) => Some(value.clone()),
                Value::Object(object) => object
                    .get("config")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                _ => None,
            })
            .collect(),
        _ => {
            warnings.push(FabricWarning {
                code: "fabric_metadata_field_ignored",
                message: format!("fabric.mod.json field {field} is not a string or array"),
            });
            Vec::new()
        }
    };
    values.sort();
    values
}

fn jars_field(value: &Value) -> Vec<String> {
    let mut jars = value
        .get("jars")
        .and_then(Value::as_array)
        .map(|jars| {
            jars.iter()
                .filter_map(|jar| {
                    jar.get("file")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    jars.sort();
    jars
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_fabric_metadata_fields() {
        let mut warnings = Vec::new();
        let mixins = string_or_array_field(
            &json!({
                "mixins": [
                    "a.mixins.json",
                    { "config": "b.mixins.json" }
                ]
            }),
            "mixins",
            &mut warnings,
        );
        assert_eq!(mixins, vec!["a.mixins.json", "b.mixins.json"]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn classifies_accessor_and_redirect_mixins() {
        let accessor = classify_mixin("com.example.ExampleAccessor", &json!({}));
        let redirect = classify_mixin("com.example.ExampleRedirect", &json!({}));
        assert_eq!(
            accessor.classification,
            MixinClassification::PartiallySupported
        );
        assert_eq!(redirect.classification, MixinClassification::Unsupported);
    }
}
