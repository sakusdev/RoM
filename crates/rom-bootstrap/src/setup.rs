use crate::{
    GenerateOptions, GenerateReport, InstallLocalOptions, PrepareOptions, PrepareReport,
    StatusReport, generate_version_pack, install_local_server, prepare_instance, status_instance,
};
use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    env,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct SetupOptions {
    pub instance: PathBuf,
    pub version: String,
    pub accept_minecraft_eula: bool,
    pub force_download: bool,
    pub force_generate: bool,
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
    let prepare = prepare_instance(&PrepareOptions {
        instance: options.instance.clone(),
        version: options.version.clone(),
        accept_minecraft_eula: options.accept_minecraft_eula,
        force_download: options.force_download,
    })?;
    let generate = generate_version_pack(&GenerateOptions {
        instance: options.instance.clone(),
        force: options.force_generate,
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
}
