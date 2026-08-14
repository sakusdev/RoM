mod codegen;
mod diff;
mod fabric;
mod rules;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use rom_importer::{ImportOptions, inspect_jar};
use rom_model::JarReport;
use serde::Serialize;
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::PathBuf,
};

#[derive(Debug, Parser)]
#[command(name = "rom-cli", version, about = "JVM JAR to Rust porting toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect classes, fields, methods, and descriptors in a JAR.
    Inspect {
        /// Input JAR, for example server.jar or a Fabric mod JAR.
        jar: PathBuf,

        /// Write JSON to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Only inspect classes under this dotted or JVM-internal prefix.
        #[arg(long)]
        prefix: Option<String>,

        /// Stop after this many matching class files.
        #[arg(long)]
        limit: Option<usize>,

        /// Run class-file structural verification after parsing.
        #[arg(long)]
        verify: bool,

        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,

        /// Exit with failure when any class could not be parsed or verified.
        #[arg(long)]
        fail_on_class_error: bool,
    },
    /// Inspect method bytecode, references, difficult features, and porting classifications.
    Bytecode {
        /// Input JAR, for example server.jar or a Fabric mod JAR.
        jar: PathBuf,

        /// Write JSON to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Only inspect classes under this dotted or JVM-internal prefix.
        #[arg(long)]
        prefix: Option<String>,

        /// Stop after this many matching class files.
        #[arg(long)]
        limit: Option<usize>,

        /// Run class-file structural verification after parsing.
        #[arg(long)]
        verify: bool,

        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,

        /// Exit with failure when any class could not be parsed or verified.
        #[arg(long)]
        fail_on_class_error: bool,
    },
    /// Build method-level control-flow graphs from bytecode.
    Cfg {
        /// Input JAR, for example server.jar or a Fabric mod JAR.
        jar: PathBuf,

        /// Write JSON to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Only inspect classes under this dotted or JVM-internal prefix.
        #[arg(long)]
        prefix: Option<String>,

        /// Stop after this many matching class files.
        #[arg(long)]
        limit: Option<usize>,

        /// Run class-file structural verification after parsing.
        #[arg(long)]
        verify: bool,

        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,

        /// Exit with failure when any class could not be parsed or verified.
        #[arg(long)]
        fail_on_class_error: bool,

        /// Keep only this exact dotted or JVM-internal class in the output.
        #[arg(long = "class")]
        class_name: Option<String>,

        /// Keep only methods with this JVM method name in the output.
        #[arg(long)]
        method: Option<String>,

        /// Keep only methods with this exact JVM descriptor in the output.
        #[arg(long)]
        descriptor: Option<String>,

        /// Write Graphviz DOT for a single selected method to this file.
        #[arg(long)]
        dot_output: Option<PathBuf>,
    },
    /// Build typed intermediate representation from bytecode and CFG.
    Ir {
        /// Input JAR, for example server.jar or a Fabric mod JAR.
        jar: PathBuf,

        /// Write JSON to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Only inspect classes under this dotted or JVM-internal prefix.
        #[arg(long)]
        prefix: Option<String>,

        /// Stop after this many matching class files.
        #[arg(long)]
        limit: Option<usize>,

        /// Run class-file structural verification after parsing.
        #[arg(long)]
        verify: bool,

        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,

        /// Exit with failure when any class could not be parsed or verified.
        #[arg(long)]
        fail_on_class_error: bool,

        /// Keep only this exact dotted or JVM-internal class in the output.
        #[arg(long = "class")]
        class_name: Option<String>,

        /// Keep only methods with this JVM method name in the output.
        #[arg(long)]
        method: Option<String>,

        /// Keep only methods with this exact JVM descriptor in the output.
        #[arg(long)]
        descriptor: Option<String>,
    },
    /// Generate deterministic Rust skeletons with todo! method bodies.
    Generate {
        /// Input JAR, for example server.jar or a Fabric mod JAR.
        jar: PathBuf,

        /// Exact dotted or JVM-internal class to generate.
        #[arg(long = "class")]
        class_name: String,

        /// Output directory for the generated Rust package.
        #[arg(short, long)]
        output: PathBuf,

        /// Run class-file structural verification after parsing.
        #[arg(long)]
        verify: bool,

        /// Exit with failure when any class could not be parsed or verified.
        #[arg(long)]
        fail_on_class_error: bool,
    },
    /// Apply mappings and Minecraft rewrite rules to an inventory report.
    Map {
        /// Input JAR, for example server.jar.
        jar: PathBuf,

        /// Tiny mapping file.
        #[arg(long)]
        mappings: Option<PathBuf>,

        /// Directory containing rewrite TOML files.
        #[arg(long)]
        rewrites: Option<PathBuf>,

        /// Write JSON to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Only inspect classes under this dotted or JVM-internal prefix.
        #[arg(long)]
        prefix: Option<String>,

        /// Stop after this many matching class files.
        #[arg(long)]
        limit: Option<usize>,

        /// Run class-file structural verification after parsing.
        #[arg(long)]
        verify: bool,

        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,

        /// Exit with failure when any class could not be parsed or verified.
        #[arg(long)]
        fail_on_class_error: bool,
    },
    /// Inspect Fabric metadata and Mixin compatibility.
    Fabric {
        #[command(subcommand)]
        command: FabricCommand,
    },
    /// Run deterministic differential replay comparisons.
    Diff {
        #[command(subcommand)]
        command: DiffCommand,
    },
}

#[derive(Debug, Subcommand)]
enum FabricCommand {
    /// Inspect fabric.mod.json, access wideners, nested JARs, and Mixin configs.
    Inspect {
        /// Input Fabric mod JAR.
        jar: PathBuf,

        /// Write JSON to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DiffCommand {
    /// Compare expected and actual outcomes in a replay file.
    Run {
        /// Replay JSON file.
        replay: PathBuf,

        /// Write JSON to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect {
            jar,
            output,
            prefix,
            limit,
            verify,
            compact,
            fail_on_class_error,
        } => {
            let options = ImportOptions {
                class_prefix: prefix,
                class_limit: limit,
                verify,
                bytecode: false,
                cfg: false,
                ir: false,
            };
            let report = inspect_jar(&jar, &options)
                .with_context(|| format!("failed to inspect {}", jar.display()))?;

            match output {
                Some(path) => {
                    let file = File::create(&path)
                        .with_context(|| format!("cannot create {}", path.display()))?;
                    write_report(BufWriter::new(file), &report, compact)?;
                    eprintln!(
                        "wrote {} classes and {} errors to {}",
                        report.summary.classes_parsed,
                        report.errors.len(),
                        path.display()
                    );
                }
                None => write_report(BufWriter::new(io::stdout().lock()), &report, compact)?,
            }

            if fail_on_class_error && !report.errors.is_empty() {
                bail!("{} archive/class entries failed", report.errors.len());
            }
        }
        Command::Bytecode {
            jar,
            output,
            prefix,
            limit,
            verify,
            compact,
            fail_on_class_error,
        } => {
            let options = ImportOptions {
                class_prefix: prefix,
                class_limit: limit,
                verify,
                bytecode: true,
                cfg: false,
                ir: false,
            };
            let report = inspect_jar(&jar, &options)
                .with_context(|| format!("failed to inspect bytecode in {}", jar.display()))?;

            match output {
                Some(path) => {
                    let file = File::create(&path)
                        .with_context(|| format!("cannot create {}", path.display()))?;
                    write_report(BufWriter::new(file), &report, compact)?;
                    eprintln!(
                        "wrote bytecode for {} classes and {} errors to {}",
                        report.summary.classes_parsed,
                        report.errors.len(),
                        path.display()
                    );
                }
                None => write_report(BufWriter::new(io::stdout().lock()), &report, compact)?,
            }

            if fail_on_class_error && !report.errors.is_empty() {
                bail!("{} archive/class entries failed", report.errors.len());
            }
        }
        Command::Cfg {
            jar,
            output,
            prefix,
            limit,
            verify,
            compact,
            fail_on_class_error,
            class_name,
            method,
            descriptor,
            dot_output,
        } => {
            let options = ImportOptions {
                class_prefix: class_name.clone().or(prefix),
                class_limit: limit,
                verify,
                bytecode: true,
                cfg: true,
                ir: false,
            };
            let mut report = inspect_jar(&jar, &options)
                .with_context(|| format!("failed to build CFGs in {}", jar.display()))?;
            filter_cfg_report(
                &mut report,
                class_name.as_deref(),
                method.as_deref(),
                descriptor.as_deref(),
            );

            if let Some(path) = dot_output {
                let dot = single_cfg_dot(&report)
                    .context("--dot-output requires exactly one method with a CFG after filters")?;
                let mut file = File::create(&path)
                    .with_context(|| format!("cannot create {}", path.display()))?;
                file.write_all(dot.as_bytes())?;
                file.flush()?;
            }

            match output {
                Some(path) => {
                    let file = File::create(&path)
                        .with_context(|| format!("cannot create {}", path.display()))?;
                    write_report(BufWriter::new(file), &report, compact)?;
                    eprintln!(
                        "wrote CFGs for {} classes and {} errors to {}",
                        report.summary.classes_parsed,
                        report.errors.len(),
                        path.display()
                    );
                }
                None => write_report(BufWriter::new(io::stdout().lock()), &report, compact)?,
            }

            if fail_on_class_error && !report.errors.is_empty() {
                bail!("{} archive/class entries failed", report.errors.len());
            }
        }
        Command::Ir {
            jar,
            output,
            prefix,
            limit,
            verify,
            compact,
            fail_on_class_error,
            class_name,
            method,
            descriptor,
        } => {
            let options = ImportOptions {
                class_prefix: class_name.clone().or(prefix),
                class_limit: limit,
                verify,
                bytecode: true,
                cfg: true,
                ir: true,
            };
            let mut report = inspect_jar(&jar, &options)
                .with_context(|| format!("failed to build IR in {}", jar.display()))?;
            filter_cfg_report(
                &mut report,
                class_name.as_deref(),
                method.as_deref(),
                descriptor.as_deref(),
            );

            match output {
                Some(path) => {
                    let file = File::create(&path)
                        .with_context(|| format!("cannot create {}", path.display()))?;
                    write_report(BufWriter::new(file), &report, compact)?;
                    eprintln!(
                        "wrote IR for {} classes and {} errors to {}",
                        report.summary.classes_parsed,
                        report.errors.len(),
                        path.display()
                    );
                }
                None => write_report(BufWriter::new(io::stdout().lock()), &report, compact)?,
            }

            if fail_on_class_error && !report.errors.is_empty() {
                bail!("{} archive/class entries failed", report.errors.len());
            }
        }
        Command::Generate {
            jar,
            class_name,
            output,
            verify,
            fail_on_class_error,
        } => {
            let options = ImportOptions {
                class_prefix: Some(class_name.clone()),
                class_limit: None,
                verify,
                bytecode: false,
                cfg: false,
                ir: false,
            };
            let mut report = inspect_jar(&jar, &options)
                .with_context(|| format!("failed to inspect {}", jar.display()))?;
            filter_cfg_report(&mut report, Some(&class_name), None, None);
            if report.classes.len() != 1 {
                bail!(
                    "--class {} matched {} classes; expected exactly one",
                    class_name,
                    report.classes.len()
                );
            }
            let generation =
                codegen::generate_rust_skeleton(&report, &output).with_context(|| {
                    format!("cannot generate Rust skeleton in {}", output.display())
                })?;
            eprintln!(
                "generated {} Rust files and {} warnings in {}",
                generation.generated_files.len(),
                generation.warnings.len(),
                output.display()
            );

            if fail_on_class_error && !report.errors.is_empty() {
                bail!("{} archive/class entries failed", report.errors.len());
            }
        }
        Command::Map {
            jar,
            mappings,
            rewrites,
            output,
            prefix,
            limit,
            verify,
            compact,
            fail_on_class_error,
        } => {
            let options = ImportOptions {
                class_prefix: prefix,
                class_limit: limit,
                verify,
                bytecode: true,
                cfg: false,
                ir: false,
            };
            let report = inspect_jar(&jar, &options)
                .with_context(|| format!("failed to inspect {}", jar.display()))?;
            let mapping_report = rules::build_mapping_report(
                &jar,
                &report,
                mappings.as_deref(),
                rewrites.as_deref(),
            )?;
            write_json_output(output, &mapping_report, compact)?;

            if fail_on_class_error && !report.errors.is_empty() {
                bail!("{} archive/class entries failed", report.errors.len());
            }
        }
        Command::Fabric { command } => match command {
            FabricCommand::Inspect {
                jar,
                output,
                compact,
            } => {
                let report = fabric::inspect_fabric_jar(&jar)
                    .with_context(|| format!("failed to inspect Fabric mod {}", jar.display()))?;
                write_json_output(output, &report, compact)?;
            }
        },
        Command::Diff { command } => match command {
            DiffCommand::Run {
                replay,
                output,
                compact,
            } => {
                let report = diff::run_replay(&replay)
                    .with_context(|| format!("failed to run replay {}", replay.display()))?;
                write_json_output(output, &report, compact)?;
            }
        },
    }
    Ok(())
}

fn filter_cfg_report(
    report: &mut JarReport,
    class_name: Option<&str>,
    method: Option<&str>,
    descriptor: Option<&str>,
) {
    if let Some(class_name) = class_name {
        let normalized = normalize_class_name(class_name);
        report
            .classes
            .retain(|class| class.internal_name == normalized);
    }

    if method.is_some() || descriptor.is_some() {
        for class in &mut report.classes {
            class.methods.retain(|member| {
                method.is_none_or(|expected| member.name == expected)
                    && descriptor.is_none_or(|expected| member.descriptor == expected)
            });
        }
    }

    report.summary.classes_parsed = report.classes.len();
    report.summary.fields = report.classes.iter().map(|class| class.fields.len()).sum();
    report.summary.methods = report.classes.iter().map(|class| class.methods.len()).sum();
}

fn single_cfg_dot(report: &JarReport) -> Option<&str> {
    let mut cfgs = report
        .classes
        .iter()
        .flat_map(|class| class.methods.iter())
        .filter_map(|method| method.bytecode.as_ref())
        .filter_map(|bytecode| bytecode.cfg.as_ref());
    let cfg = cfgs.next()?;
    if cfgs.next().is_some() {
        return None;
    }
    Some(&cfg.dot)
}

fn normalize_class_name(name: &str) -> String {
    name.trim()
        .replace('.', "/")
        .trim_start_matches('/')
        .to_owned()
}

fn write_report(
    mut writer: impl Write,
    report: &rom_model::JarReport,
    compact: bool,
) -> Result<()> {
    if compact {
        serde_json::to_writer(&mut writer, report)?;
    } else {
        serde_json::to_writer_pretty(&mut writer, report)?;
    }
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn write_json_output<T: Serialize>(
    output: Option<PathBuf>,
    value: &T,
    compact: bool,
) -> Result<()> {
    match output {
        Some(path) => {
            let file =
                File::create(&path).with_context(|| format!("cannot create {}", path.display()))?;
            write_json(BufWriter::new(file), value, compact)
        }
        None => write_json(BufWriter::new(io::stdout().lock()), value, compact),
    }
}

fn write_json<T: Serialize>(mut writer: impl Write, value: &T, compact: bool) -> Result<()> {
    if compact {
        serde_json::to_writer(&mut writer, value)?;
    } else {
        serde_json::to_writer_pretty(&mut writer, value)?;
    }
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
