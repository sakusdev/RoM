//! JAR ingestion and JVM class-file metadata extraction.
//!
//! The importer is deliberately fault tolerant: a malformed or unsupported
//! class is recorded in `JarReport::errors` without aborting the whole scan.

mod bytecode;

use ristretto_classfile::{ClassFile, ConstantPool};
use rom_model::{
    AccessInfo, ClassReport, ClassVersion, EntryError, ErrorStage, JarReport, JarSummary,
    MemberReport, MethodBytecodeReport, REPORT_SCHEMA_VERSION, SourceInfo,
};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};
use thiserror::Error;
use zip::ZipArchive;

const MAX_ARCHIVE_ENTRIES: usize = 250_000;
const MAX_CLASS_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Optional JVM-internal or dotted class prefix.
    pub class_prefix: Option<String>,
    /// Stop after this many matching class entries. `None` means no limit.
    pub class_limit: Option<usize>,
    /// Run structural verification after parsing.
    pub verify: bool,
    /// Inventory method `Code` attributes and classify bytecode difficulty.
    pub bytecode: bool,
    /// Build method-level control-flow graphs. Implies bytecode inventory.
    pub cfg: bool,
    /// Build typed intermediate representation. Implies bytecode inventory and CFG.
    pub ir: bool,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("cannot read metadata for {path}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot open JAR {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid ZIP/JAR {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
    #[error("ZIP/JAR {path} contains {actual} entries, exceeding limit {limit}")]
    EntryLimit {
        path: PathBuf,
        actual: usize,
        limit: usize,
    },
}

pub fn inspect_jar(
    path: impl AsRef<Path>,
    options: &ImportOptions,
) -> Result<JarReport, ImportError> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path).map_err(|source| ImportError::Metadata {
        path: path.to_owned(),
        source,
    })?;
    let file = File::open(path).map_err(|source| ImportError::Open {
        path: path.to_owned(),
        source,
    })?;
    let mut archive = ZipArchive::new(file).map_err(|source| ImportError::Zip {
        path: path.to_owned(),
        source,
    })?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(ImportError::EntryLimit {
            path: path.to_owned(),
            actual: archive.len(),
            limit: MAX_ARCHIVE_ENTRIES,
        });
    }

    let archive_entries = archive.len();
    let manifest = read_manifest(&mut archive);
    let normalized_prefix = options
        .class_prefix
        .as_deref()
        .map(normalize_prefix)
        .filter(|value| !value.is_empty());

    let mut classes = Vec::new();
    let mut errors = Vec::new();
    let mut class_entries_seen = 0usize;

    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(EntryError {
                    archive_path: format!("<entry:{index}>"),
                    stage: ErrorStage::ArchiveRead,
                    message: error.to_string(),
                    classfile_major: None,
                    classfile_minor: None,
                });
                continue;
            }
        };

        let entry_name = entry.name().to_owned();
        if !entry_name.ends_with(".class") || entry_name == "module-info.class" {
            continue;
        }
        if let Some(prefix) = &normalized_prefix {
            if !entry_name.starts_with(prefix) {
                continue;
            }
        }
        if options
            .class_limit
            .is_some_and(|limit| class_entries_seen >= limit)
        {
            break;
        }

        class_entries_seen += 1;
        let expected_size = entry.size();
        let bytes = match read_bounded_bytes(&mut entry, expected_size, MAX_CLASS_BYTES) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                errors.push(EntryError {
                    archive_path: entry_name,
                    stage: ErrorStage::ArchiveRead,
                    message: format!(
                        "class entry size {expected_size} exceeds {MAX_CLASS_BYTES} bytes or changed while reading"
                    ),
                    classfile_major: None,
                    classfile_minor: None,
                });
                continue;
            }
            Err(error) => {
                errors.push(EntryError {
                    archive_path: entry_name,
                    stage: ErrorStage::ArchiveRead,
                    message: error.to_string(),
                    classfile_major: None,
                    classfile_minor: None,
                });
                continue;
            }
        };

        match inspect_class_bytes_with_options(
            &entry_name,
            &bytes,
            options.verify,
            options.bytecode || options.cfg || options.ir,
            options.cfg || options.ir,
            options.ir,
        ) {
            Ok(class) => classes.push(class),
            Err(error) => errors.push(error),
        }
    }

    classes.sort_by(|a, b| a.internal_name.cmp(&b.internal_name));
    errors.sort_by(|a, b| a.archive_path.cmp(&b.archive_path));

    let summary = JarSummary {
        archive_entries,
        class_entries_seen,
        classes_parsed: classes.len(),
        classes_failed: class_entries_seen.saturating_sub(classes.len()),
        fields: classes.iter().map(|class| class.fields.len()).sum(),
        methods: classes.iter().map(|class| class.methods.len()).sum(),
    };

    Ok(JarReport {
        schema_version: REPORT_SCHEMA_VERSION,
        source: SourceInfo {
            path: path.to_string_lossy().into_owned(),
            size_bytes: metadata.len(),
        },
        manifest,
        summary,
        classes,
        errors,
    })
}

pub fn inspect_class_bytes(
    archive_path: &str,
    bytes: &[u8],
    verify: bool,
) -> Result<ClassReport, EntryError> {
    inspect_class_bytes_with_options(archive_path, bytes, verify, false, false, false)
}

fn inspect_class_bytes_with_options(
    archive_path: &str,
    bytes: &[u8],
    verify: bool,
    include_bytecode: bool,
    include_cfg: bool,
    include_ir: bool,
) -> Result<ClassReport, EntryError> {
    let (minor, major) = classfile_header_version(bytes);
    let class_file = ClassFile::from_bytes(bytes).map_err(|error| EntryError {
        archive_path: archive_path.to_owned(),
        stage: ErrorStage::ClassParse,
        message: error.to_string(),
        classfile_major: major,
        classfile_minor: minor,
    })?;

    if verify {
        class_file.verify().map_err(|error| EntryError {
            archive_path: archive_path.to_owned(),
            stage: ErrorStage::ClassVerify,
            message: error.to_string(),
            classfile_major: Some(class_file.version.major()),
            classfile_minor: Some(class_file.version.minor()),
        })?;
    }

    let bytecode_reports = if include_bytecode {
        let reports = bytecode::inspect_method_bytecode(bytes, include_cfg, include_ir).map_err(
            |message| EntryError {
                archive_path: archive_path.to_owned(),
                stage: ErrorStage::BytecodeInventory,
                message,
                classfile_major: Some(class_file.version.major()),
                classfile_minor: Some(class_file.version.minor()),
            },
        )?;
        if reports.len() != class_file.methods.len() {
            return Err(EntryError {
                archive_path: archive_path.to_owned(),
                stage: ErrorStage::BytecodeInventory,
                message: format!(
                    "bytecode parser found {} methods but class parser found {}",
                    reports.len(),
                    class_file.methods.len()
                ),
                classfile_major: Some(class_file.version.major()),
                classfile_minor: Some(class_file.version.minor()),
            });
        }
        Some(reports)
    } else {
        None
    };

    class_report(archive_path, &class_file, bytecode_reports).map_err(|message| EntryError {
        archive_path: archive_path.to_owned(),
        stage: ErrorStage::ConstantPoolResolve,
        message,
        classfile_major: Some(class_file.version.major()),
        classfile_minor: Some(class_file.version.minor()),
    })
}

fn class_report(
    archive_path: &str,
    class_file: &ClassFile<'_>,
    bytecode_reports: Option<Vec<MethodBytecodeReport>>,
) -> Result<ClassReport, String> {
    let pool = &class_file.constant_pool;
    let internal_name = resolve_class(pool, class_file.this_class)?;

    let super_name = if class_file.super_class == 0 {
        None
    } else {
        Some(resolve_class(pool, class_file.super_class)?)
    };

    let interfaces = class_file
        .interfaces
        .iter()
        .map(|index| resolve_class(pool, *index))
        .collect::<Result<Vec<_>, _>>()?;

    let fields = class_file
        .fields
        .iter()
        .map(|field| {
            Ok(MemberReport {
                name: resolve_utf8(pool, field.name_index)?,
                descriptor: resolve_utf8(pool, field.descriptor_index)?,
                access: AccessInfo {
                    bits: field.access_flags.bits(),
                    debug: format!("{:?}", field.access_flags),
                },
                attributes_count: field.attributes.len(),
                bytecode: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let methods = class_file
        .methods
        .iter()
        .enumerate()
        .map(|(index, method)| {
            Ok(MemberReport {
                name: resolve_utf8(pool, method.name_index)?,
                descriptor: resolve_utf8(pool, method.descriptor_index)?,
                access: AccessInfo {
                    bits: method.access_flags.bits(),
                    debug: format!("{:?}", method.access_flags),
                },
                attributes_count: method.attributes.len(),
                bytecode: bytecode_reports
                    .as_ref()
                    .and_then(|reports| reports.get(index))
                    .cloned(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(ClassReport {
        archive_path: archive_path.to_owned(),
        dotted_name: internal_name.replace('/', "."),
        internal_name,
        super_name,
        interfaces,
        version: ClassVersion {
            java: class_file.version.java(),
            major: class_file.version.major(),
            minor: class_file.version.minor(),
            preview: class_file.version.is_preview(),
            display: class_file.version.to_string(),
        },
        access: AccessInfo {
            bits: class_file.access_flags.bits(),
            debug: format!("{:?}", class_file.access_flags),
        },
        constant_pool_entries: class_file.constant_pool.len(),
        attributes_count: class_file.attributes.len(),
        fields,
        methods,
    })
}

fn resolve_utf8(pool: &ConstantPool<'_>, index: u16) -> Result<String, String> {
    pool.try_get_utf8(index)
        .map(|value| value.to_string())
        .map_err(|error| format!("cannot resolve UTF-8 constant #{index}: {error}"))
}

fn resolve_class(pool: &ConstantPool<'_>, index: u16) -> Result<String, String> {
    pool.try_get_class(index)
        .map(|value| value.to_string())
        .map_err(|error| format!("cannot resolve class constant #{index}: {error}"))
}

fn normalize_prefix(prefix: &str) -> String {
    let mut prefix = prefix.trim().replace('.', "/");
    while prefix.starts_with('/') {
        prefix.remove(0);
    }
    prefix
}

fn read_manifest<R: Read + io::Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
    let mut entry = archive.by_name("META-INF/MANIFEST.MF").ok()?;
    let expected_size = entry.size();
    let bytes = read_bounded_bytes(&mut entry, expected_size, MAX_MANIFEST_BYTES).ok()??;
    String::from_utf8(bytes).ok()
}

fn read_bounded_bytes(
    reader: impl Read,
    expected_size: u64,
    limit: u64,
) -> io::Result<Option<Vec<u8>>> {
    if expected_size > limit {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(expected_size as usize);
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != expected_size || bytes.len() as u64 > limit {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn classfile_header_version(bytes: &[u8]) -> (Option<u16>, Option<u16>) {
    if bytes.len() < 8 || bytes[0..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
        return (None, None);
    }
    let minor = u16::from_be_bytes([bytes[4], bytes[5]]);
    let major = u16::from_be_bytes([bytes[6], bytes[7]]);
    (Some(minor), Some(major))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_normalization_accepts_dotted_names() {
        assert_eq!(normalize_prefix("net.minecraft"), "net/minecraft");
        assert_eq!(normalize_prefix("/net/minecraft"), "net/minecraft");
    }

    #[test]
    fn bounded_reader_rejects_oversized_or_mismatched_entries() {
        let exact = read_bounded_bytes(&b"abcd"[..], 4, 4).unwrap();
        assert_eq!(exact.unwrap(), b"abcd");
        assert!(read_bounded_bytes(&b"abcde"[..], 5, 4).unwrap().is_none());
        assert!(read_bounded_bytes(&b"abc"[..], 4, 4).unwrap().is_none());
    }

    #[test]
    fn reads_header_version_without_full_parse() {
        let bytes = [0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x41];
        assert_eq!(classfile_header_version(&bytes), (Some(0), Some(65)));
    }
}
