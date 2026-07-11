use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use rom_bootstrap::{
    DoctorReport, GenerateOptions, InstallLocalOptions, PrepareOptions, SetupOptions,
    doctor_instance, generate_version_pack, install_local_server, prepare_instance, run_instance,
    setup_instance, status_instance,
};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "rom-bootstrap",
    version,
    about = "Prepare and run local RoM server instances from verified official Minecraft source artifacts"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Complete prepare, version-pack generation, native installation, and readiness checks.
    Setup {
        /// Instance directory to create or update.
        #[arg(long, default_value = "rom-instance")]
        instance: PathBuf,

        /// Supported Minecraft Java Edition version.
        #[arg(long, default_value = "26.1.2")]
        version: String,

        /// Confirm that you reviewed and accept the Minecraft EULA.
        #[arg(long)]
        accept_minecraft_eula: bool,

        /// Redownload the official artifact even when the local verified cache exists.
        #[arg(long)]
        force_download: bool,

        /// Regenerate the pack even when the recorded pack is already valid.
        #[arg(long)]
        force_generate: bool,

        /// RoM workspace to build when no server binary is supplied or found beside rom-bootstrap.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,

        /// Existing ferrum-server binary to install. A sibling release binary is detected automatically.
        #[arg(long)]
        server_binary: Option<PathBuf>,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Download and verify the official server JAR, then create a local RoM instance.
    Prepare {
        /// Instance directory to create or update.
        #[arg(long, default_value = "rom-instance")]
        instance: PathBuf,

        /// Supported Minecraft Java Edition version.
        #[arg(long, default_value = "26.1.2")]
        version: String,

        /// Confirm that you reviewed and accept the Minecraft EULA.
        #[arg(long)]
        accept_minecraft_eula: bool,

        /// Redownload the official artifact even when the local verified cache exists.
        #[arg(long)]
        force_download: bool,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Generate and verify a deterministic local .rompack from the prepared official source.
    Generate {
        /// Prepared RoM instance directory.
        #[arg(long, default_value = "rom-instance")]
        instance: PathBuf,

        /// Regenerate the pack even when the recorded pack is already valid.
        #[arg(long)]
        force: bool,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Build ferrum-server from a local RoM checkout or install an existing native binary.
    InstallLocal {
        /// Prepared RoM instance directory.
        #[arg(long, default_value = "rom-instance")]
        instance: PathBuf,

        /// RoM workspace to build when --server-binary is not supplied.
        #[arg(long, default_value = ".")]
        workspace: PathBuf,

        /// Existing ferrum-server binary to install instead of building the workspace.
        #[arg(long)]
        server_binary: Option<PathBuf>,
    },

    /// Verify the instance metadata, EULA marker, official source cache, and native binary.
    Status {
        /// RoM instance directory.
        #[arg(long, default_value = "rom-instance")]
        instance: PathBuf,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Diagnose whether an instance is ready to run and report every missing component.
    Doctor {
        /// RoM instance directory.
        #[arg(long, default_value = "rom-instance")]
        instance: PathBuf,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Run the native RoM server from a prepared instance.
    Run {
        /// RoM instance directory.
        #[arg(long, default_value = "rom-instance")]
        instance: PathBuf,

        /// Additional arguments passed to ferrum-server after `--`.
        #[arg(last = true)]
        server_args: Vec<OsString>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Setup {
            instance,
            version,
            accept_minecraft_eula,
            force_download,
            force_generate,
            workspace,
            server_binary,
            json,
        } => {
            let report = setup_instance(&SetupOptions {
                instance,
                version,
                accept_minecraft_eula,
                force_download,
                force_generate,
                workspace,
                server_binary,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("RoM instance is ready: {}", report.doctor.instance.display());
                println!(
                    "Minecraft: {} / protocol {}",
                    report.prepare.minecraft_version, report.prepare.protocol
                );
                println!("Version pack: {}", report.generate.version_pack.display());
                println!("Native server: {}", report.installed_server.display());
                println!(
                    "Run: rom-bootstrap run --instance {}",
                    report.doctor.instance.display()
                );
            }
        }
        Command::Prepare {
            instance,
            version,
            accept_minecraft_eula,
            force_download,
            json,
        } => {
            let report = prepare_instance(&PrepareOptions {
                instance,
                version,
                accept_minecraft_eula,
                force_download,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Prepared RoM instance: {}", report.instance.display());
                println!(
                    "Minecraft: {} / protocol {}",
                    report.minecraft_version, report.protocol
                );
                println!("Official source: {}", report.official_server_jar.display());
                println!("Verified SHA-1: {}", report.official_sha1);
                println!(
                    "Cache: {}",
                    if report.reused_cached_jar {
                        "reused"
                    } else {
                        "downloaded"
                    }
                );
                println!(
                    "Next: rom-bootstrap generate --instance {}",
                    report.instance.display()
                );
            }
        }
        Command::Generate {
            instance,
            force,
            json,
        } => {
            let report = generate_version_pack(&GenerateOptions { instance, force })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Generated RoM version pack: {}",
                    report.version_pack.display()
                );
                println!("Pack SHA-256: {}", report.version_pack_sha256);
                println!("Game JAR: {}", report.game_jar_path);
                println!("Game JAR SHA-256: {}", report.game_jar_sha256);
                println!(
                    "Packets: {} / data version: {} / sections: {}+{} / registries: {} / entries: {} / source resources: {}",
                    report.packet_count,
                    report.world_data_version,
                    report.overworld_min_section_y,
                    report.overworld_section_count,
                    report.registry_count,
                    report.registry_entry_count,
                    report.resource_count
                );
                println!(
                    "Play world: {} / dimension type {} / sea {} / floor {} / spawn {},{}",
                    report.world_dimension,
                    report.dimension_type_id,
                    report.sea_level,
                    report.floor_y,
                    report.spawn_x,
                    report.spawn_z
                );
                println!(
                    "Cache: {}",
                    if report.reused_existing_pack {
                        "reused"
                    } else {
                        "generated"
                    }
                );
            }
        }
        Command::InstallLocal {
            instance,
            workspace,
            server_binary,
        } => {
            let installed = install_local_server(&InstallLocalOptions {
                instance,
                workspace,
                server_binary,
            })?;
            println!("Installed native RoM server: {}", installed.display());
        }
        Command::Status { instance, json } => {
            let report = status_instance(instance)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Instance: {}", report.instance.display());
                println!("Prepared: {}", report.prepared);
                println!(
                    "Minecraft EULA accepted: {}",
                    report.minecraft_eula_accepted
                );
                println!(
                    "Minecraft version: {}",
                    report.minecraft_version.as_deref().unwrap_or("unknown")
                );
                println!(
                    "Protocol: {}",
                    report
                        .protocol
                        .map_or_else(|| "unknown".to_owned(), |value| value.to_string())
                );
                println!(
                    "Official source verified: {}",
                    report.official_source_verified
                );
                println!("Version pack verified: {}", report.version_pack_verified);
                if let Some(path) = &report.version_pack_path {
                    println!("Version pack: {}", path.display());
                }
                println!(
                    "Native server installed: {}",
                    report.native_server_installed
                );
            }
        }
        Command::Doctor { instance, json } => {
            let report = doctor_instance(instance)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_doctor(&report);
            }
            if !report.ready_to_run {
                bail!("RoM instance is not ready to run");
            }
        }
        Command::Run {
            instance,
            server_args,
        } => {
            let status = run_instance(instance, &server_args)?;
            if !status.success() {
                bail!("native RoM server exited with status {status}");
            }
        }
    }
    Ok(())
}

fn print_doctor(report: &DoctorReport) {
    println!("Instance: {}", report.instance.display());
    println!("Ready to run: {}", report.ready_to_run);
    println!("Prepared: {}", report.status.prepared);
    println!(
        "Minecraft EULA accepted: {}",
        report.status.minecraft_eula_accepted
    );
    println!(
        "Official source verified: {}",
        report.status.official_source_verified
    );
    println!(
        "Version pack verified: {}",
        report.status.version_pack_verified
    );
    println!(
        "Native server installed: {}",
        report.status.native_server_installed
    );
    println!("Server config present: {}", report.server_config_present);
    if !report.problems.is_empty() {
        println!("Problems:");
        for problem in &report.problems {
            println!("- {problem}");
        }
    }
}
