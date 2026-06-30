from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_between(path: str, start: str, end: str, replacement: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    start_index = text.index(start)
    end_index = text.index(end, start_index)
    file.write_text(text[:start_index] + replacement + text[end_index:], encoding="utf-8")


# Fix the staged extractor's read-after-move binding.
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        if file.size() > MAX_RESOURCE_BYTES {\n"
    "            bail!(\"registry resource {name} exceeds the per-resource limit\");\n"
    "        }\n"
    "        total_resource_bytes = total_resource_bytes\n"
    "            .checked_add(file.size())\n",
    "        let expected_size = file.size();\n"
    "        if expected_size > MAX_RESOURCE_BYTES {\n"
    "            bail!(\"registry resource {name} exceeds the per-resource limit\");\n"
    "        }\n"
    "        total_resource_bytes = total_resource_bytes\n"
    "            .checked_add(expected_size)\n",
)
replace_once(
    "crates/rom-bootstrap/src/extract.rs",
    "        if data.len() as u64 != file.size() {\n",
    "        if data.len() as u64 != expected_size {\n",
)

# Integrate extraction, pack provenance, status verification, and runtime use.
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    "use anyhow::{Context, Result, bail};\n",
    "mod extract;\n\n"
    "pub use extract::{GenerateOptions, GenerateReport, generate_version_pack};\n\n"
    "use anyhow::{Context, Result, bail};\n",
)
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    "    pub official_source_verified: bool,\n"
    "    pub native_server_binary: Option<PathBuf>,\n",
    "    pub official_source_verified: bool,\n"
    "    pub version_pack_path: Option<PathBuf>,\n"
    "    pub version_pack_sha256: Option<String>,\n"
    "    pub version_pack_verified: bool,\n"
    "    pub native_server_binary: Option<PathBuf>,\n",
)
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    "    stage: BootstrapStage,\n"
    "    source: SourceRecord,\n"
    "}\n",
    "    stage: BootstrapStage,\n"
    "    source: SourceRecord,\n"
    "    #[serde(default)]\n"
    "    pack: Option<extract::PackRecord>,\n"
    "}\n",
)
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    "enum BootstrapStage {\n    OfficialSourceVerified,\n}\n",
    "enum BootstrapStage {\n"
    "    OfficialSourceVerified,\n"
    "    VersionPackGenerated,\n"
    "}\n",
)

new_status = '''pub fn status_instance(instance: impl AsRef<Path>) -> Result<StatusReport> {
    let instance = absolute_path(instance.as_ref())?;
    let manifest_path = instance.join("rom-bootstrap.json");
    let manifest = if manifest_path.is_file() {
        let bytes = fs::read(&manifest_path)
            .with_context(|| format!("cannot read {}", manifest_path.display()))?;
        Some(
            serde_json::from_slice::<BootstrapManifest>(&bytes)
                .with_context(|| format!("cannot parse {}", manifest_path.display()))?,
        )
    } else {
        None
    };

    let (version, protocol, jar_path, verified) = if let Some(manifest) = &manifest {
        let jar_path = instance.join(&manifest.source.local_path);
        let verified = manifest.schema_version == BOOTSTRAP_SCHEMA_VERSION
            && matches!(
                manifest.stage,
                BootstrapStage::OfficialSourceVerified | BootstrapStage::VersionPackGenerated
            )
            && jar_path.is_file()
            && verify_file(&jar_path, &manifest.source.sha1, manifest.source.size)?;
        (
            Some(manifest.minecraft_version.clone()),
            Some(manifest.protocol),
            Some(jar_path),
            verified,
        )
    } else {
        (None, None, None, false)
    };
    let pack_status = if let Some(manifest) = &manifest {
        extract::verify_version_pack_record(&instance, manifest)?
    } else {
        extract::VersionPackStatus::default()
    };

    let native_binary = instance.join("bin").join(native_server_file_name());
    let installed = native_binary.is_file();

    Ok(StatusReport {
        instance,
        prepared: manifest.is_some(),
        minecraft_eula_accepted: eula_is_accepted(&manifest_path.with_file_name("eula.txt"))?,
        minecraft_version: version,
        protocol,
        official_server_jar: jar_path,
        official_source_verified: verified,
        version_pack_path: pack_status.path,
        version_pack_sha256: pack_status.sha256,
        version_pack_verified: pack_status.verified,
        native_server_binary: installed.then_some(native_binary),
        native_server_installed: installed,
    })
}

'''
replace_between(
    "crates/rom-bootstrap/src/lib.rs",
    "pub fn status_instance",
    "pub fn run_instance",
    new_status,
)

new_run = '''pub fn run_instance(
    instance: impl AsRef<Path>,
    server_args: &[impl AsRef<OsStr>],
) -> Result<ExitStatus> {
    let instance = absolute_path(instance.as_ref())?;
    let status = status_instance(&instance)?;
    if !status.minecraft_eula_accepted {
        bail!(
            "Minecraft EULA acceptance is missing; run rom-bootstrap prepare with explicit acceptance"
        );
    }
    if !status.official_source_verified {
        bail!("official Minecraft source artifact is missing or failed integrity verification");
    }
    if !status.version_pack_verified {
        bail!("local version pack is missing or invalid; run rom-bootstrap generate first");
    }
    let version_pack = status
        .version_pack_path
        .context("verified version pack path is missing")?;
    let binary = status
        .native_server_binary
        .context("native RoM server is not installed; run rom-bootstrap install-local")?;
    let config = instance.join("server.toml");
    if !config.is_file() {
        bail!("server configuration is missing: {}", config.display());
    }

    Command::new(binary)
        .arg("--config")
        .arg(&config)
        .arg("--version-pack")
        .arg(&version_pack)
        .args(server_args.iter().map(AsRef::as_ref))
        .current_dir(&instance)
        .status()
        .context("failed to start the native RoM server")
}

'''
replace_between(
    "crates/rom-bootstrap/src/lib.rs",
    "pub fn run_instance",
    "fn build_http_client",
    new_run,
)
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    "            local_path: relative_jar.to_owned(),\n"
    "        },\n"
    "    };\n"
    "    write_json(instance.join(\"rom-bootstrap.json\"), &manifest)?;\n"
    "    write_json(\n"
    "        instance.join(\"versions\").join(version).join(\"rompack.json\"),\n"
    "        &manifest,\n"
    "    )?;\n",
    "            local_path: relative_jar.to_owned(),\n"
    "        },\n"
    "        pack: None,\n"
    "    };\n"
    "    write_json(instance.join(\"rom-bootstrap.json\"), &manifest)?;\n",
)
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    "        assert!(!status.native_server_installed);\n",
    "        assert!(!status.version_pack_verified);\n"
    "        assert!(status.version_pack_path.is_none());\n"
    "        assert!(!status.native_server_installed);\n",
)

# CLI generate command and status output.
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    "    InstallLocalOptions, PrepareOptions, install_local_server, prepare_instance, run_instance,\n"
    "    status_instance,\n",
    "    GenerateOptions, InstallLocalOptions, PrepareOptions, generate_version_pack,\n"
    "    install_local_server, prepare_instance, run_instance, status_instance,\n",
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    "    /// Build ferrum-server from a local RoM checkout or install an existing native binary.\n"
    "    InstallLocal {\n",
    "    /// Generate and verify a deterministic local .rompack from the prepared official source.\n"
    "    Generate {\n"
    "        /// Prepared RoM instance directory.\n"
    "        #[arg(long, default_value = \"rom-instance\")]\n"
    "        instance: PathBuf,\n\n"
    "        /// Regenerate the pack even when the recorded pack is already valid.\n"
    "        #[arg(long)]\n"
    "        force: bool,\n\n"
    "        /// Print the result as JSON.\n"
    "        #[arg(long)]\n"
    "        json: bool,\n"
    "    },\n\n"
    "    /// Build ferrum-server from a local RoM checkout or install an existing native binary.\n"
    "    InstallLocal {\n",
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    "                    \"Next: rom-bootstrap install-local --instance {}\",\n"
    "                    report.instance.display()\n"
    "                );\n"
    "            }\n"
    "        }\n"
    "        Command::InstallLocal {\n",
    "                    \"Next: rom-bootstrap generate --instance {}\",\n"
    "                    report.instance.display()\n"
    "                );\n"
    "            }\n"
    "        }\n"
    "        Command::Generate {\n"
    "            instance,\n"
    "            force,\n"
    "            json,\n"
    "        } => {\n"
    "            let report = generate_version_pack(&GenerateOptions { instance, force })?;\n"
    "            if json {\n"
    "                println!(\"{}\", serde_json::to_string_pretty(&report)?);\n"
    "            } else {\n"
    "                println!(\"Generated RoM version pack: {}\", report.version_pack.display());\n"
    "                println!(\"Pack SHA-256: {}\", report.version_pack_sha256);\n"
    "                println!(\"Game JAR: {}\", report.game_jar_path);\n"
    "                println!(\"Game JAR SHA-256: {}\", report.game_jar_sha256);\n"
    "                println!(\n"
    "                    \"Registries: {} / entries: {} / source resources: {}\",\n"
    "                    report.registry_count,\n"
    "                    report.registry_entry_count,\n"
    "                    report.resource_count\n"
    "                );\n"
    "                println!(\n"
    "                    \"Cache: {}\",\n"
    "                    if report.reused_existing_pack { \"reused\" } else { \"generated\" }\n"
    "                );\n"
    "            }\n"
    "        }\n"
    "        Command::InstallLocal {\n",
)
replace_once(
    "crates/rom-bootstrap/src/main.rs",
    "                println!(\"Native server installed: {}\", report.native_server_installed);\n",
    "                println!(\"Version pack verified: {}\", report.version_pack_verified);\n"
    "                if let Some(path) = &report.version_pack_path {\n"
    "                    println!(\"Version pack: {}\", path.display());\n"
    "                }\n"
    "                println!(\"Native server installed: {}\", report.native_server_installed);\n",
)

# Server-side pack validation and explicit CLI plumbing.
replace_once(
    "crates/ferrum-server/src/main.rs",
    "use ferrum_protocol::{HandshakeIntent, PacketKind, PacketTable, ProtocolProfile, ProtocolSession};\n",
    "use ferrum_protocol::{HandshakeIntent, PacketKind, PacketTable, ProtocolProfile, ProtocolSession};\n"
    "use ferrum_rompack::{RomPack, read_rompack};\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    collections::BTreeSet,\n",
    "    collections::{BTreeMap, BTreeSet},\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    path::PathBuf,\n",
    "    path::{Path, PathBuf},\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "struct Cli {\n"
    "    /// Path to the server configuration file.\n"
    "    #[arg(long, value_name = \"PATH\")]\n"
    "    config: PathBuf,\n"
    "}\n",
    "struct Cli {\n"
    "    /// Path to the server configuration file.\n"
    "    #[arg(long, value_name = \"PATH\")]\n"
    "    config: PathBuf,\n\n"
    "    /// Locally generated and integrity-verified RoM version pack.\n"
    "    #[arg(long, value_name = \"PATH\")]\n"
    "    version_pack: Option<PathBuf>,\n"
    "}\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    config\n"
    "        .protocol_profile()\n"
    "        .context(\"cannot build configured protocol profile\")?;\n"
    "    let state = Arc::new(ServerState::new(config.online_players));\n",
    "    config\n"
    "        .protocol_profile()\n"
    "        .context(\"cannot build configured protocol profile\")?;\n"
    "    if let Some(version_pack) = &cli.version_pack {\n"
    "        validate_version_pack(version_pack)?;\n"
    "    }\n"
    "    let state = Arc::new(ServerState::new(config.online_players));\n",
)
server_validator = '''fn validate_version_pack(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("version pack {} does not exist or is not a file", path.display());
    }
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", path.display()))?;
    let (pack, summary) = read_rompack(&canonical)
        .with_context(|| format!("cannot load {}", canonical.display()))?;
    validate_builtin_26_1_2_pack(&pack)?;
    println!(
        "loaded RoM version pack {} (SHA-256 {}, {} registries / {} entries)",
        canonical.display(),
        summary.sha256,
        summary.registry_count,
        summary.registry_entry_count
    );
    Ok(())
}

fn validate_builtin_26_1_2_pack(pack: &RomPack) -> Result<()> {
    if pack.metadata.minecraft_version != version_26_1_2::PROFILE_NAME
        || pack.metadata.protocol != version_26_1_2::PROTOCOL_VERSION
    {
        bail!("version pack does not match the built-in Minecraft 26.1.2 profile");
    }
    if !pack
        .metadata
        .source
        .official_server_sha1
        .eq_ignore_ascii_case(version_26_1_2::OFFICIAL_SERVER_SHA1)
    {
        bail!("version pack official-source SHA-1 does not match the built-in profile");
    }

    let expected: BTreeMap<_, _> = version_26_1_2::SYNCHRONIZED_REGISTRIES
        .iter()
        .map(|registry| (registry.id, registry.entries.to_vec()))
        .collect();
    let actual: BTreeMap<_, _> = pack
        .registries
        .iter()
        .map(|registry| {
            (
                registry.id.as_str(),
                registry.entries.iter().map(String::as_str).collect::<Vec<_>>(),
            )
        })
        .collect();
    if actual != expected {
        bail!("version pack synchronized registries do not match the built-in profile");
    }
    Ok(())
}

'''
replace_once(
    "crates/ferrum-server/src/main.rs",
    "impl Default for ServerConfig {\n",
    server_validator + "impl Default for ServerConfig {\n",
)

# Documentation moves the bootstrap stage from source verification to generated packs.
replace_once(
    "README.md",
    "6. RoM records the source hash and patch-set identity in local bootstrap metadata.\n"
    "7. The native `rom-server` executable is built or installed and remains the runtime.\n\n"
    "The current bootstrap implementation stops at the **verified official source** stage. It does not decompile, translate, execute, patch, or redistribute the official server JAR. Deterministic local version-pack generation is planned as a later stage.\n",
    "6. RoM scans only synchronized-registry JSON resources from the locally obtained game JAR.\n"
    "7. A deterministic, integrity-protected `.rompack` records registry IDs, source-resource hashes, source hashes, and patch-set identity.\n"
    "8. The native `rom-server` validates that pack against its built-in 26.1.2 profile before starting.\n\n"
    "The current bootstrap implementation supports the **version pack generated** stage. It does not decompile, translate, execute, bytecode-patch, or redistribute the official server JAR. The generated pack contains derived registry identifiers and source-resource hashes, not copied JSON payloads.\n",
)
replace_once(
    "README.md",
    "- Deterministic local extraction and completed `.rompack` generation\n",
    "- Runtime replacement of the remaining built-in packet/profile constants with generated pack data\n",
)
replace_once(
    "README.md",
    "### 3. Install the native server\n\n"
    "```bash\n"
    "./target/release/rom-bootstrap install-local \\\n"
    "  --instance ./rom-instance \\\n"
    "  --workspace .\n"
    "```\n\n"
    "### 4. Inspect and run\n",
    "### 3. Generate the local version pack\n\n"
    "```bash\n"
    "./target/release/rom-bootstrap generate \\\n"
    "  --instance ./rom-instance\n"
    "```\n\n"
    "The extractor opens the verified local JAR, resolves the bundled game JAR when present, validates all selected JSON resources, derives the exact synchronized-registry identifiers, compares them with the built-in 26.1.2 manifest, and writes an integrity-protected `.rompack`.\n\n"
    "### 4. Install the native server\n\n"
    "```bash\n"
    "./target/release/rom-bootstrap install-local \\\n"
    "  --instance ./rom-instance \\\n"
    "  --workspace .\n"
    "```\n\n"
    "### 5. Inspect and run\n",
)
replace_once(
    "README.md",
    "│   └── 26.1.2/\n"
    "│       └── rompack.json\n",
    "│   └── 26.1.2/\n"
    "│       ├── 26.1.2.rompack\n"
    "│       └── rompack.json\n",
)
replace_once(
    "README.md",
    "- `rom-bootstrap` — official-source verification and local instance management\n",
    "- `rom-bootstrap` — official-source verification, bounded local extraction, and instance management\n"
    "- `ferrum-rompack` — deterministic pack encoding, integrity validation, and bounded decoding\n",
)
replace_once(
    "README.md",
    "1. Package `rom-bootstrap` alongside `rom-server` in native release archives\n"
    "2. Define a deterministic local `.rompack` format\n"
    "3. Extract only required version data into locally generated packs\n"
    "4. Wire dedicated network workers into the authoritative 20 TPS runtime\n"
    "5. Add full block interaction and inventory validation\n"
    "6. Add entities and entity tracking\n"
    "7. Add persistent Anvil region loading and saving\n"
    "8. Add Microsoft account authentication and encrypted online mode\n"
    "9. Add additional Minecraft version profiles\n",
    "1. Package `rom-bootstrap` alongside `rom-server` in native release archives\n"
    "2. Move more version-specific runtime metadata from built-in Rust constants into generated packs\n"
    "3. Wire dedicated network workers into the authoritative 20 TPS runtime\n"
    "4. Add full block interaction and inventory validation\n"
    "5. Add entities and entity tracking\n"
    "6. Add persistent Anvil region loading and saving\n"
    "7. Add Microsoft account authentication and encrypted online mode\n"
    "8. Add additional Minecraft version profiles\n",
)

replace_between(
    "docs/BOOTSTRAP.md",
    "## Current bootstrap stage",
    "## Build the tools",
    '''## Current bootstrap stage

The implementation supports `official_source_verified` and `version_pack_generated`:

1. Resolve Minecraft Java Edition 26.1.2 from the official version manifest.
2. Verify the version metadata SHA-1.
3. Download and verify the official server JAR from an approved HTTPS host.
4. Resolve the bundled game JAR without executing Java or bytecode.
5. Scan only the 28 synchronized-registry resource directories.
6. Parse every selected resource as bounded JSON and derive its resource identifier.
7. Record each selected source path, size, and SHA-256 without copying the JSON payload into the pack.
8. Compare the resulting 28 registries and 382 identifiers with the built-in 26.1.2 manifest.
9. Write a deterministic `.rompack` with a container SHA-256 trailer and provenance metadata.
10. Revalidate the pack before `rom-bootstrap run`, then pass it to the native server for a second profile check.

The extractor does **not** decompile, translate, execute, or bytecode-patch the official server JAR. The generated pack is local-only provenance and derived runtime metadata.

''',
)
replace_once(
    "docs/BOOTSTRAP.md",
    "## Install the local native server\n",
    "## Generate the local version pack\n\n"
    "```bash\n"
    "./target/release/rom-bootstrap generate \\\n"
    "  --instance ./rom-instance\n"
    "```\n\n"
    "Use `--force` to regenerate an already valid pack. Generation is deterministic for the same verified source JAR and extractor version.\n\n"
    "## Install the local native server\n",
)
replace_once(
    "docs/BOOTSTRAP.md",
    "│   └── 26.1.2/\n"
    "│       └── rompack.json\n",
    "│   └── 26.1.2/\n"
    "│       ├── 26.1.2.rompack\n"
    "│       └── rompack.json\n",
)
replace_between(
    "docs/BOOTSTRAP.md",
    "## Planned next stages",
    "",
    "## Planned next stages\n\n"
    "1. Package `rom-bootstrap` alongside `rom-server` in native release archives.\n"
    "2. Move packet tables and additional version-specific runtime metadata into generated packs.\n"
    "3. Add more independently testable extractors only when the server consumes their output.\n"
    "4. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.\n",
)
