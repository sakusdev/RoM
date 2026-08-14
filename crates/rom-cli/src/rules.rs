use anyhow::{Context, Result};
use rom_model::{ClassReport, JarReport, MemberReport};
use serde::Serialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, Serialize)]
pub struct MappingReport {
    pub schema_version: u32,
    pub source_jar: String,
    pub mappings: Option<MappingSummary>,
    pub rewrites: RewriteSummary,
    pub minecraft: MinecraftSummary,
    pub classes: Vec<MappedClassReport>,
    pub warnings: Vec<MappingWarning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappingSummary {
    pub path: String,
    pub format: MappingFormat,
    pub namespaces: Vec<String>,
    pub classes: usize,
    pub fields: usize,
    pub methods: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingFormat {
    TinyV2,
    TinyV1,
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RewriteSummary {
    pub files_loaded: usize,
    pub java_types: BTreeMap<String, String>,
    pub java_methods: BTreeMap<String, String>,
    pub java_fields: BTreeMap<String, String>,
    pub manual_overrides: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinecraftSummary {
    pub special_cases: Vec<MinecraftSpecialCaseReport>,
    pub pilot_ports: Vec<PilotPortReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinecraftSpecialCaseReport {
    pub java_type: String,
    pub handler: String,
    pub present: bool,
    pub references: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PilotPortReport {
    pub java_type: String,
    pub status: PilotPortStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PilotPortStatus {
    ReadyForManualPort,
    MappingMissing,
    NotPresent,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappedClassReport {
    pub internal_name: String,
    pub selected_name: String,
    pub fields: Vec<MappedMemberReport>,
    pub methods: Vec<MappedMemberReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappedMemberReport {
    pub name: String,
    pub descriptor: String,
    pub selected_name: Option<String>,
    pub rewrite: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappingWarning {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
struct MappingDatabase {
    format: MappingFormatInternal,
    namespaces: Vec<String>,
    classes: BTreeMap<String, SymbolNameSet>,
    fields: BTreeMap<String, SymbolNameSet>,
    methods: BTreeMap<String, SymbolNameSet>,
}

#[derive(Debug, Clone, Copy, Default)]
enum MappingFormatInternal {
    TinyV2,
    TinyV1,
    #[default]
    Unknown,
}

#[derive(Debug, Clone)]
struct SymbolNameSet {
    selected: String,
}

pub fn build_mapping_report(
    jar: &Path,
    report: &JarReport,
    mappings: Option<&Path>,
    rewrite_dir: Option<&Path>,
) -> Result<MappingReport> {
    let mut warnings = Vec::new();
    let mapping_db = match mappings {
        Some(path) => Some(load_mappings(path, &mut warnings)?),
        None => None,
    };
    let rewrites = match rewrite_dir {
        Some(path) => load_rewrites(path, &mut warnings)?,
        None => RewriteSummary::default(),
    };
    let classes = report
        .classes
        .iter()
        .map(|class| map_class(class, mapping_db.as_ref(), &rewrites))
        .collect();
    let minecraft = minecraft_summary(report, &rewrites);

    Ok(MappingReport {
        schema_version: 1,
        source_jar: jar.to_string_lossy().into_owned(),
        mappings: mapping_db.as_ref().map(|db| MappingSummary {
            path: mappings
                .expect("mapping path is present when mapping database exists")
                .to_string_lossy()
                .into_owned(),
            format: match db.format {
                MappingFormatInternal::TinyV2 => MappingFormat::TinyV2,
                MappingFormatInternal::TinyV1 => MappingFormat::TinyV1,
                MappingFormatInternal::Unknown => MappingFormat::Unknown,
            },
            namespaces: db.namespaces.clone(),
            classes: db.classes.len(),
            fields: db.fields.len(),
            methods: db.methods.len(),
        }),
        rewrites,
        minecraft,
        classes,
        warnings,
    })
}

fn load_mappings(path: &Path, warnings: &mut Vec<MappingWarning>) -> Result<MappingDatabase> {
    let text =
        fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut db = MappingDatabase::default();
    let mut current_class: Option<String> = None;
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if line_index == 0 && columns.first() == Some(&"tiny") {
            if columns.get(1) == Some(&"2") {
                db.format = MappingFormatInternal::TinyV2;
                db.namespaces = columns
                    .iter()
                    .skip(3)
                    .map(|value| (*value).to_owned())
                    .collect();
            } else {
                db.format = MappingFormatInternal::TinyV1;
                db.namespaces = columns
                    .iter()
                    .skip(1)
                    .map(|value| (*value).to_owned())
                    .collect();
            }
            continue;
        }
        match columns.as_slice() {
            ["c", source, rest @ ..] | ["CLASS", source, rest @ ..] => {
                current_class = Some((*source).to_owned());
                db.classes.insert(
                    (*source).to_owned(),
                    SymbolNameSet {
                        selected: rest.last().unwrap_or(source).to_string(),
                    },
                );
            }
            ["m", values @ ..] => {
                if matches!(db.format, MappingFormatInternal::TinyV2) && values.len() >= 3 {
                    let descriptor = values[0];
                    let name = values[1];
                    let rest = &values[2..];
                    if let Some(owner) = &current_class {
                        db.methods.insert(
                            format!("{owner}.{name}{descriptor}"),
                            SymbolNameSet {
                                selected: rest.last().unwrap_or(&name).to_string(),
                            },
                        );
                    } else {
                        warnings.push(MappingWarning {
                            code: "mapping_line_ignored",
                            message: format!(
                                "ignored method mapping line {} without current class: {}",
                                line_index + 1,
                                line
                            ),
                        });
                    }
                } else if values.len() >= 4 {
                    let owner = values[0];
                    let descriptor = values[1];
                    let name = values[2];
                    let rest = &values[3..];
                    db.methods.insert(
                        format!("{owner}.{name}{descriptor}"),
                        SymbolNameSet {
                            selected: rest.last().unwrap_or(&name).to_string(),
                        },
                    );
                } else {
                    warnings.push(MappingWarning {
                        code: "mapping_line_ignored",
                        message: format!(
                            "ignored method mapping line {} without current class: {}",
                            line_index + 1,
                            line
                        ),
                    });
                }
            }
            ["METHOD", owner, descriptor, name, rest @ ..] => {
                db.methods.insert(
                    format!("{owner}.{name}{descriptor}"),
                    SymbolNameSet {
                        selected: rest.last().unwrap_or(name).to_string(),
                    },
                );
            }
            ["f", values @ ..] => {
                if matches!(db.format, MappingFormatInternal::TinyV2) && values.len() >= 3 {
                    let descriptor = values[0];
                    let name = values[1];
                    let rest = &values[2..];
                    if let Some(owner) = &current_class {
                        db.fields.insert(
                            format!("{owner}.{name}{descriptor}"),
                            SymbolNameSet {
                                selected: rest.last().unwrap_or(&name).to_string(),
                            },
                        );
                    } else {
                        warnings.push(MappingWarning {
                            code: "mapping_line_ignored",
                            message: format!(
                                "ignored field mapping line {} without current class: {}",
                                line_index + 1,
                                line
                            ),
                        });
                    }
                } else if values.len() >= 4 {
                    let owner = values[0];
                    let descriptor = values[1];
                    let name = values[2];
                    let rest = &values[3..];
                    db.fields.insert(
                        format!("{owner}.{name}{descriptor}"),
                        SymbolNameSet {
                            selected: rest.last().unwrap_or(&name).to_string(),
                        },
                    );
                } else {
                    warnings.push(MappingWarning {
                        code: "mapping_line_ignored",
                        message: format!(
                            "ignored field mapping line {} without current class: {}",
                            line_index + 1,
                            line
                        ),
                    });
                }
            }
            ["FIELD", owner, descriptor, name, rest @ ..] => {
                db.fields.insert(
                    format!("{owner}.{name}{descriptor}"),
                    SymbolNameSet {
                        selected: rest.last().unwrap_or(name).to_string(),
                    },
                );
            }
            _ => warnings.push(MappingWarning {
                code: "mapping_line_ignored",
                message: format!(
                    "ignored unsupported mapping line {}: {}",
                    line_index + 1,
                    line
                ),
            }),
        }
    }
    Ok(db)
}

fn load_rewrites(path: &Path, warnings: &mut Vec<MappingWarning>) -> Result<RewriteSummary> {
    let mut summary = RewriteSummary::default();
    for entry in fs::read_dir(path).with_context(|| format!("cannot read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        summary.files_loaded += 1;
        parse_rewrite_toml(&text, &mut summary, warnings);
    }
    Ok(summary)
}

fn parse_rewrite_toml(
    text: &str,
    summary: &mut RewriteSummary,
    warnings: &mut Vec<MappingWarning>,
) {
    let mut section = String::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            warnings.push(MappingWarning {
                code: "rewrite_line_ignored",
                message: format!("ignored rewrite line {}: {}", line_index + 1, raw_line),
            });
            continue;
        };
        let key = unquote(key.trim());
        let value = unquote(value.trim());
        match section.as_str() {
            "java_types" => {
                summary.java_types.insert(key, value);
            }
            "java_methods" => {
                summary.java_methods.insert(key, value);
            }
            "java_fields" => {
                summary.java_fields.insert(key, value);
            }
            "manual_overrides" => {
                summary.manual_overrides.insert(key, value);
            }
            _ => warnings.push(MappingWarning {
                code: "rewrite_section_ignored",
                message: format!("ignored unsupported rewrite section [{section}]"),
            }),
        }
    }
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn map_class(
    class: &ClassReport,
    mapping_db: Option<&MappingDatabase>,
    rewrites: &RewriteSummary,
) -> MappedClassReport {
    let selected_name = rewrites
        .java_types
        .get(&class.internal_name)
        .cloned()
        .or_else(|| {
            mapping_db
                .and_then(|db| db.classes.get(&class.internal_name))
                .map(|symbol| symbol.selected.clone())
        })
        .unwrap_or_else(|| class.internal_name.clone());
    MappedClassReport {
        internal_name: class.internal_name.clone(),
        selected_name,
        fields: class
            .fields
            .iter()
            .map(|field| {
                map_member(
                    &class.internal_name,
                    field,
                    mapping_db,
                    &rewrites.java_fields,
                )
            })
            .collect(),
        methods: class
            .methods
            .iter()
            .map(|method| {
                map_member(
                    &class.internal_name,
                    method,
                    mapping_db,
                    &rewrites.java_methods,
                )
            })
            .collect(),
    }
}

fn map_member(
    owner: &str,
    member: &MemberReport,
    mapping_db: Option<&MappingDatabase>,
    rewrites: &BTreeMap<String, String>,
) -> MappedMemberReport {
    let key = format!("{owner}.{}{}", member.name, member.descriptor);
    MappedMemberReport {
        name: member.name.clone(),
        descriptor: member.descriptor.clone(),
        selected_name: mapping_db.and_then(|db| {
            db.methods
                .get(&key)
                .or_else(|| db.fields.get(&key))
                .map(|symbol| symbol.selected.clone())
        }),
        rewrite: rewrites.get(&key).cloned(),
    }
}

fn minecraft_summary(report: &JarReport, rewrites: &RewriteSummary) -> MinecraftSummary {
    let mut reference_counts = BTreeMap::<String, usize>::new();
    for class in &report.classes {
        *reference_counts
            .entry(class.internal_name.clone())
            .or_default() += 1;
        for method in &class.methods {
            let Some(bytecode) = method.bytecode.as_ref() else {
                continue;
            };
            for ty in &bytecode.referenced_types {
                *reference_counts.entry(ty.clone()).or_default() += 1;
            }
        }
    }

    let special_cases = minecraft_special_cases()
        .iter()
        .map(|(java_type, handler)| MinecraftSpecialCaseReport {
            java_type: (*java_type).to_owned(),
            handler: (*handler).to_owned(),
            present: reference_counts.contains_key(*java_type),
            references: reference_counts
                .get(*java_type)
                .copied()
                .unwrap_or_default(),
        })
        .collect();

    let pilot_ports = pilot_types()
        .iter()
        .map(|java_type| {
            let present = reference_counts.contains_key(*java_type);
            let mapped = rewrites.java_types.contains_key(*java_type);
            let (status, reason) = if present && mapped {
                (
                    PilotPortStatus::ReadyForManualPort,
                    "type is present and has a configured Rust rewrite".to_owned(),
                )
            } else if present {
                (
                    PilotPortStatus::MappingMissing,
                    "type is present but has no Rust rewrite yet".to_owned(),
                )
            } else {
                (
                    PilotPortStatus::NotPresent,
                    "type was not found in this scan".to_owned(),
                )
            };
            PilotPortReport {
                java_type: (*java_type).to_owned(),
                status,
                reason,
            }
        })
        .collect();

    MinecraftSummary {
        special_cases,
        pilot_ports,
    }
}

fn minecraft_special_cases() -> &'static [(&'static str, &'static str)] {
    &[
        ("net/minecraft/util/Identifier", "identifier"),
        ("net/minecraft/registry/RegistryKey", "registry_key"),
        ("net/minecraft/registry/Registry", "registry"),
        ("com/mojang/serialization/Codec", "codec"),
        ("net/minecraft/network/codec/PacketCodec", "packet_codec"),
        ("net/minecraft/network/PacketByteBuf", "packet_byte_buf"),
        ("net/minecraft/util/math/BlockPos", "block_pos"),
        ("net/minecraft/util/math/ChunkPos", "chunk_pos"),
        ("net/minecraft/util/math/Vec3d", "vec3d"),
        ("net/minecraft/block/BlockState", "block_state"),
        ("net/minecraft/item/ItemStack", "item_stack"),
        ("net/minecraft/nbt/NbtElement", "nbt"),
    ]
}

fn pilot_types() -> &'static [&'static str] {
    &[
        "net/minecraft/util/math/Direction",
        "net/minecraft/util/math/BlockPos",
        "net/minecraft/util/math/ChunkPos",
        "net/minecraft/util/math/Vec3d",
        "net/minecraft/util/Identifier",
        "net/minecraft/nbt/NbtElement",
        "net/minecraft/network/PacketByteBuf",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rewrite_toml_sections() {
        let mut summary = RewriteSummary::default();
        let mut warnings = Vec::new();
        parse_rewrite_toml(
            r#"
            [java_types]
            "java/lang/String" = "String"
            [java_methods]
            "java/lang/Math.floor(D)D" = "f64::floor"
            "#,
            &mut summary,
            &mut warnings,
        );

        assert_eq!(
            summary.java_types.get("java/lang/String"),
            Some(&"String".to_owned())
        );
        assert_eq!(
            summary.java_methods.get("java/lang/Math.floor(D)D"),
            Some(&"f64::floor".to_owned())
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_tiny_v2_class_member_mappings() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m5-rules/example.tiny");
        let mut warnings = Vec::new();
        let db = load_mappings(&path, &mut warnings).expect("tiny fixture should parse");

        assert_eq!(db.classes.len(), 1);
        assert_eq!(db.methods.len(), 1);
        assert_eq!(db.fields.len(), 1);
        assert!(warnings.is_empty());
        assert_eq!(
            db.methods
                .get("m1/BytecodeFeatures.arithmetic(II)I")
                .map(|symbol| symbol.selected.as_str()),
            Some("arithmetic")
        );
    }

    #[test]
    fn recognizes_minecraft_pilot_mapping_status() {
        let mut report = JarReport {
            schema_version: 1,
            source: rom_model::SourceInfo {
                path: "fixture.jar".to_owned(),
                size_bytes: 0,
            },
            manifest: None,
            summary: rom_model::JarSummary::default(),
            classes: Vec::new(),
            errors: Vec::new(),
        };
        report.classes.push(ClassReport {
            archive_path: "net/minecraft/util/Identifier.class".to_owned(),
            internal_name: "net/minecraft/util/Identifier".to_owned(),
            dotted_name: "net.minecraft.util.Identifier".to_owned(),
            super_name: Some("java/lang/Object".to_owned()),
            interfaces: Vec::new(),
            version: rom_model::ClassVersion {
                java: 21,
                major: 65,
                minor: 0,
                preview: false,
                display: "Java 21".to_owned(),
            },
            access: rom_model::AccessInfo {
                bits: 0,
                debug: String::new(),
            },
            constant_pool_entries: 0,
            attributes_count: 0,
            fields: Vec::new(),
            methods: Vec::new(),
        });
        let mut rewrites = RewriteSummary::default();
        rewrites.java_types.insert(
            "net/minecraft/util/Identifier".to_owned(),
            "rom_types::Identifier".to_owned(),
        );

        let summary = minecraft_summary(&report, &rewrites);
        assert!(summary.pilot_ports.iter().any(|port| {
            port.java_type == "net/minecraft/util/Identifier"
                && matches!(port.status, PilotPortStatus::ReadyForManualPort)
        }));
    }
}
