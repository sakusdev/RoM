use crate::{
    BootstrapManifest, BootstrapStage, GenerateOptions, GenerateReport, InstallLocalOptions,
    PrepareOptions, PrepareReport, StatusReport, absolute_path, generate_version_pack,
    install_local_server, prepare_instance, status_instance, write_json,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct SetupOptions {
    pub instance: PathBuf,
    pub version: String,
    pub accept_minecraft_eula: bool,
    pub force_download: bool,
    pub force_generate: bool,
    pub packet_report: Option<PathBuf>,
    pub registry_report: Option<PathBuf>,
    pub workspace: PathBuf,
    pub server_binary: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupReport {
    pub prepare: PrepareReport,
    pub generate: GenerateReport,
    pub installed_server: PathBuf,
    pub doctor: DoctorReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub instance: PathBuf,
    pub status: StatusReport,
    pub server_config: PathBuf,
    pub server_config_present: bool,
    pub ready_to_run: bool,
    pub problems: Vec<String>,
}

pub fn setup_instance(options: &SetupOptions) -> Result<SetupReport> {
    let existing_manifest = read_existing_manifest(&options.instance)?;
    let prepare = prepare_instance(&PrepareOptions {
        instance: options.instance.clone(),
        version: options.version.clone(),
        accept_minecraft_eula: options.accept_minecraft_eula,
        force_download: options.force_download,
    })?;
    restore_compatible_pack_record(&prepare.instance, existing_manifest)?;

    let generate = generate_version_pack(&GenerateOptions {
        instance: options.instance.clone(),
        force: options.force_generate,
        packet_report: options.packet_report.clone(),
        registry_report: options.registry_report.clone(),
    })?;
    let installed_server = install_local_server(&InstallLocalOptions {
        instance: options.instance.clone(),
        workspace: options.workspace.clone(),
        server_binary: options
            .server_binary
            .clone()
            .or_else(adjacent_native_server_binary),
    })?;
    let doctor = doctor_instance(&options.instance)?;
    if !doctor.ready_to_run {
        bail!(
            "Bootstrap setup completed but the instance is not ready: {}",
            doctor.problems.join("; ")
        );
    }
    Ok(SetupReport {
        prepare,
        generate,
        installed_server,
        doctor,
    })
}

pub fn doctor_instance(instance: impl AsRef<Path>) -> Result<DoctorReport> {
    let status = status_instance(instance.as_ref())?;
    let server_config = status.instance.join("server.toml");
    let server_config_present = server_config.is_file();
    let mut problems = Vec::new();
    if !status.prepared {
        problems.push("bootstrap manifest is missing".to_owned());
    }
    if !status.minecraft_eula_accepted {
        problems.push("Minecraft EULA acceptance is missing".to_owned());
    }
    if !status.official_source_verified {
        problems.push("official Minecraft source is missing or invalid".to_owned());
    }
    if !status.version_pack_verified {
        problems.push("local version pack is missing or invalid".to_owned());
    }
    if !status.native_server_installed {
        problems.push("native ferrum-server is not installed".to_owned());
    }
    if !server_config_present {
        problems.push("server.toml is missing".to_owned());
    }
    Ok(DoctorReport {
        instance: status.instance.clone(),
        status,
        server_config,
        server_config_present,
        ready_to_run: problems.is_empty(),
        problems,
    })
}

fn read_existing_manifest(instance: &Path) -> Result<Option<BootstrapManifest>> {
    let manifest_path = absolute_path(instance)?.join("rom-bootstrap.json");
    let bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot read {}", manifest_path.display()));
        }
    };
    Ok(serde_json::from_slice(&bytes).ok())
}

fn restore_compatible_pack_record(
    instance: &Path,
    existing: Option<BootstrapManifest>,
) -> Result<()> {
    let Some(mut existing) = existing else {
        return Ok(());
    };
    let Some(pack) = existing.pack.take() else {
        return Ok(());
    };

    let manifest_path = instance.join("rom-bootstrap.json");
    let bytes = fs::read(&manifest_path)
        .with_context(|| format!("cannot read {}", manifest_path.display()))?;
    let mut prepared: BootstrapManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("cannot parse {}", manifest_path.display()))?;
    if !manifests_share_source(&existing, &prepared) {
        return Ok(());
    }

    prepared.stage = BootstrapStage::VersionPackGenerated;
    prepared.pack = Some(pack);
    write_json(manifest_path, &prepared)
}

fn manifests_share_source(left: &BootstrapManifest, right: &BootstrapManifest) -> bool {
    left.schema_version == right.schema_version
        && left.minecraft_version == right.minecraft_version
        && left.protocol == right.protocol
        && left.patch_set == right.patch_set
        && left.source.kind == right.source.kind
        && left.source.sha1.eq_ignore_ascii_case(&right.source.sha1)
        && left.source.size == right.source.size
        && left.source.local_path == right.source.local_path
}

fn adjacent_native_server_binary() -> Option<PathBuf> {
    let directory = env::current_exe().ok()?.parent()?.to_path_buf();
    let candidate = directory.join(if cfg!(windows) {
        "ferrum-server.exe"
    } else {
        "ferrum-server"
    });
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SourceRecord, extract::PackRecord};
    use tempfile::tempdir;

    #[test]
    fn doctor_reports_every_missing_runtime_component() {
        let directory = tempdir().unwrap();
        let report = doctor_instance(directory.path()).unwrap();
        assert!(!report.ready_to_run);
        assert_eq!(report.problems.len(), 6);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("manifest"))
        );
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("server.toml"))
        );
    }

    #[test]
    fn compatible_prepare_manifest_keeps_generated_pack_provenance() {
        let directory = tempdir().unwrap();
        let instance = directory.path();
        let existing = manifest(Some(PackRecord {
            local_path: "versions/26.1.2/26.1.2.rompack".to_owned(),
            sha256: "00".repeat(32),
            size: 1,
            packet_count: 1,
            packet_catalog_count: 1,
            item_count: 1,
            protocol_registry_count: 0,
            protocol_registry_entry_count: 0,
            registry_count: 1,
            registry_entry_count: 1,
            resource_count: 1,
        }));
        write_json(instance.join("rom-bootstrap.json"), &manifest(None)).unwrap();

        restore_compatible_pack_record(instance, Some(existing)).unwrap();

        let restored: BootstrapManifest =
            serde_json::from_slice(&fs::read(instance.join("rom-bootstrap.json")).unwrap())
                .unwrap();
        assert_eq!(restored.stage, BootstrapStage::VersionPackGenerated);
        assert_eq!(
            restored.pack.unwrap().local_path,
            "versions/26.1.2/26.1.2.rompack"
        );
    }

    #[test]
    fn changed_source_does_not_restore_generated_pack_provenance() {
        let directory = tempdir().unwrap();
        let instance = directory.path();
        let mut existing = manifest(Some(PackRecord {
            local_path: "versions/26.1.2/26.1.2.rompack".to_owned(),
            sha256: "00".repeat(32),
            size: 1,
            packet_count: 1,
            packet_catalog_count: 1,
            item_count: 1,
            protocol_registry_count: 0,
            protocol_registry_entry_count: 0,
            registry_count: 1,
            registry_entry_count: 1,
            resource_count: 1,
        }));
        existing.source.sha1 = "11".repeat(20);
        write_json(instance.join("rom-bootstrap.json"), &manifest(None)).unwrap();

        restore_compatible_pack_record(instance, Some(existing)).unwrap();

        let restored: BootstrapManifest =
            serde_json::from_slice(&fs::read(instance.join("rom-bootstrap.json")).unwrap())
                .unwrap();
        assert_eq!(restored.stage, BootstrapStage::OfficialSourceVerified);
        assert!(restored.pack.is_none());
    }

    fn manifest(pack: Option<PackRecord>) -> BootstrapManifest {
        BootstrapManifest {
            schema_version: 1,
            minecraft_version: "26.1.2".to_owned(),
            protocol: 775,
            patch_set: "builtin:26.1.2".to_owned(),
            stage: if pack.is_some() {
                BootstrapStage::VersionPackGenerated
            } else {
                BootstrapStage::OfficialSourceVerified
            },
            source: SourceRecord {
                kind: "official_server_jar".to_owned(),
                url: "https://piston-data.mojang.com/server.jar".to_owned(),
                sha1: "00".repeat(20),
                size: 4,
                local_path: "cache/official/26.1.2/server.jar".to_owned(),
            },
            pack,
        }
    }
}
