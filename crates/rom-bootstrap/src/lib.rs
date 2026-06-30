mod extract;

pub use extract::{GenerateOptions, GenerateReport, generate_version_pack};

use anyhow::{Context, Result, bail};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::Duration,
};

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MINECRAFT_EULA_URL: &str = "https://aka.ms/MinecraftEULA";
const MAX_METADATA_BYTES: u64 = 8 * 1024 * 1024;
const MAX_OFFICIAL_SERVER_BYTES: u64 = 1024 * 1024 * 1024;
const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;

const INSTANCE_NOTICE: &str = r#"NOT AN OFFICIAL MINECRAFT PRODUCT.
NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

RoM is an independently written Minecraft Java Edition-compatible server.
The RoM project does not redistribute the official Minecraft client, server
software, textures, sounds, or other proprietary game assets.

The official server JAR stored under cache/official is downloaded directly by
this user from an official Mojang/Microsoft endpoint, verified against official
metadata, and retained only as a local source artifact. Do not redistribute it
or generated data derived from it unless you independently have permission.

The MIT License applies only to original RoM source code and documentation.
Minecraft, Mojang, Microsoft, and related names and assets belong to their
respective owners.
"#;

const DEFAULT_SERVER_TOML: &str = r#"[server]
profile = "26.1.2"
bind = "127.0.0.1:25565"
motd = "RoM native Rust server"
max_players = 20
online_players = 0
allow_offline_login = true
online_mode = false
hide_online_players = false
enforces_secure_chat = false
previews_chat = false

[configuration]
enabled = true
features = "minecraft:vanilla"
"#;

#[derive(Debug, Clone)]
pub struct PrepareOptions {
    pub instance: PathBuf,
    pub version: String,
    pub accept_minecraft_eula: bool,
    pub force_download: bool,
}

#[derive(Debug, Clone)]
pub struct InstallLocalOptions {
    pub instance: PathBuf,
    pub workspace: PathBuf,
    pub server_binary: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrepareReport {
    pub instance: PathBuf,
    pub minecraft_version: String,
    pub protocol: i32,
    pub official_server_jar: PathBuf,
    pub official_sha1: String,
    pub reused_cached_jar: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusReport {
    pub instance: PathBuf,
    pub prepared: bool,
    pub minecraft_eula_accepted: bool,
    pub minecraft_version: Option<String>,
    pub protocol: Option<i32>,
    pub official_server_jar: Option<PathBuf>,
    pub official_source_verified: bool,
    pub version_pack_path: Option<PathBuf>,
    pub version_pack_sha256: Option<String>,
    pub version_pack_verified: bool,
    pub native_server_binary: Option<PathBuf>,
    pub native_server_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootstrapManifest {
    schema_version: u32,
    minecraft_version: String,
    protocol: i32,
    patch_set: String,
    stage: BootstrapStage,
    source: SourceRecord,
    #[serde(default)]
    pack: Option<extract::PackRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum BootstrapStage {
    OfficialSourceVerified,
    VersionPackGenerated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceRecord {
    kind: String,
    url: String,
    sha1: String,
    size: u64,
    local_path: String,
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionIndexEntry>,
}

#[derive(Debug, Deserialize)]
struct VersionIndexEntry {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Debug, Deserialize)]
struct VersionMetadata {
    id: String,
    downloads: VersionDownloads,
}

#[derive(Debug, Deserialize)]
struct VersionDownloads {
    server: DownloadArtifact,
}

#[derive(Debug, Clone, Deserialize)]
struct DownloadArtifact {
    sha1: String,
    size: u64,
    url: String,
}

pub fn prepare_instance(options: &PrepareOptions) -> Result<PrepareReport> {
    validate_version_component(&options.version)?;
    if !options.accept_minecraft_eula {
        bail!(
            "preparing official Minecraft files requires --accept-minecraft-eula; review {MINECRAFT_EULA_URL} first"
        );
    }
    let protocol = supported_protocol(&options.version)?;
    let client = build_http_client()?;
    let artifact = resolve_official_server_artifact(&client, &options.version)?;

    if artifact.size == 0 || artifact.size > MAX_OFFICIAL_SERVER_BYTES {
        bail!(
            "official server artifact size {} is outside the allowed range",
            artifact.size
        );
    }

    let instance = absolute_path(&options.instance)?;
    let relative_jar = format!("cache/official/{}/server.jar", options.version);
    let jar_path = instance.join(&relative_jar);
    let reused_cached_jar = if !options.force_download
        && jar_path.is_file()
        && verify_file(&jar_path, &artifact.sha1, artifact.size)?
    {
        true
    } else {
        download_verified_artifact(&client, &artifact, &jar_path)?;
        false
    };

    write_instance_files(
        &instance,
        &options.version,
        protocol,
        &artifact,
        &relative_jar,
    )?;

    Ok(PrepareReport {
        instance,
        minecraft_version: options.version.clone(),
        protocol,
        official_server_jar: jar_path,
        official_sha1: artifact.sha1,
        reused_cached_jar,
    })
}

pub fn install_local_server(options: &InstallLocalOptions) -> Result<PathBuf> {
    let instance = absolute_path(&options.instance)?;
    let status = status_instance(&instance)?;
    if !status.prepared || !status.minecraft_eula_accepted || !status.official_source_verified {
        bail!("instance is not prepared; run rom-bootstrap prepare first");
    }

    let source = if let Some(binary) = &options.server_binary {
        absolute_existing_file(binary)?
    } else {
        let workspace = absolute_path(&options.workspace)?;
        let cargo_toml = workspace.join("Cargo.toml");
        if !cargo_toml.is_file() {
            bail!(
                "workspace does not contain Cargo.toml: {}",
                workspace.display()
            );
        }
        let status = Command::new("cargo")
            .args(["build", "--locked", "--release", "-p", "ferrum-server"])
            .current_dir(&workspace)
            .status()
            .context("failed to start Cargo while building ferrum-server")?;
        if !status.success() {
            bail!("ferrum-server build failed with status {status}");
        }
        let binary = workspace
            .join("target")
            .join("release")
            .join(native_server_file_name());
        absolute_existing_file(&binary)?
    };

    let destination_dir = instance.join("bin");
    fs::create_dir_all(&destination_dir)
        .with_context(|| format!("cannot create {}", destination_dir.display()))?;
    let destination = destination_dir.join(native_server_file_name());
    copy_executable(&source, &destination)?;
    Ok(destination)
}

pub fn status_instance(instance: impl AsRef<Path>) -> Result<StatusReport> {
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

pub fn run_instance(
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

fn build_http_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("RoM-Bootstrap/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .redirect(Policy::none())
        .build()
        .context("cannot build HTTPS client")
}

fn resolve_official_server_artifact(client: &Client, version: &str) -> Result<DownloadArtifact> {
    let manifest_url = ensure_official_url(VERSION_MANIFEST_URL)?;
    let manifest_bytes = download_metadata(client, manifest_url)?;
    let manifest: VersionManifest = serde_json::from_slice(&manifest_bytes)
        .context("cannot parse official version manifest")?;
    let entry = manifest
        .versions
        .into_iter()
        .find(|entry| entry.id == version)
        .with_context(|| {
            format!("Minecraft version {version} was not found in the official manifest")
        })?;

    let metadata_url = ensure_official_url(&entry.url)?;
    let metadata_bytes = download_metadata(client, metadata_url)?;
    let metadata_sha1 = sha1_bytes(&metadata_bytes);
    if !metadata_sha1.eq_ignore_ascii_case(&entry.sha1) {
        bail!(
            "official version metadata SHA-1 mismatch: expected {}, got {}",
            entry.sha1,
            metadata_sha1
        );
    }
    let metadata: VersionMetadata = serde_json::from_slice(&metadata_bytes)
        .with_context(|| format!("cannot parse official metadata for {version}"))?;
    if metadata.id != version {
        bail!(
            "official metadata returned version {}, expected {version}",
            metadata.id
        );
    }
    ensure_official_url(&metadata.downloads.server.url)?;
    Ok(metadata.downloads.server)
}

fn download_metadata(client: &Client, url: Url) -> Result<Vec<u8>> {
    let response = client
        .get(url.clone())
        .send()
        .with_context(|| format!("cannot download {url}"))?
        .error_for_status()
        .with_context(|| format!("official endpoint returned an error for {url}"))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_METADATA_BYTES)
    {
        bail!("metadata response exceeds the allowed size");
    }
    let bytes = response.bytes().context("cannot read metadata response")?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        bail!("metadata response exceeds the allowed size");
    }
    Ok(bytes.to_vec())
}

fn download_verified_artifact(
    client: &Client,
    artifact: &DownloadArtifact,
    path: &Path,
) -> Result<()> {
    let url = ensure_official_url(&artifact.url)?;
    let parent = path
        .parent()
        .with_context(|| format!("artifact path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    let temporary = path.with_extension("jar.part");
    let _ = fs::remove_file(&temporary);

    let response = client
        .get(url.clone())
        .send()
        .with_context(|| format!("cannot download {url}"))?
        .error_for_status()
        .with_context(|| format!("official endpoint returned an error for {url}"))?;
    if response
        .content_length()
        .is_some_and(|length| length != artifact.size)
    {
        bail!("official server Content-Length does not match metadata");
    }

    let mut file = File::create(&temporary)
        .with_context(|| format!("cannot create {}", temporary.display()))?;
    let copied = io::copy(&mut response.take(MAX_OFFICIAL_SERVER_BYTES + 1), &mut file)
        .context("cannot write official server artifact")?;
    file.sync_all()
        .with_context(|| format!("cannot sync {}", temporary.display()))?;
    if copied > MAX_OFFICIAL_SERVER_BYTES {
        let _ = fs::remove_file(&temporary);
        bail!("official server artifact exceeds the allowed size");
    }
    if copied != artifact.size || !verify_file(&temporary, &artifact.sha1, artifact.size)? {
        let actual = sha1_file(&temporary).unwrap_or_else(|_| "unavailable".to_owned());
        let _ = fs::remove_file(&temporary);
        bail!(
            "official server artifact failed verification: expected size {} and SHA-1 {}, got size {} and SHA-1 {}",
            artifact.size,
            artifact.sha1,
            copied,
            actual
        );
    }
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("cannot replace {}", path.display()))?;
    }
    fs::rename(&temporary, path)
        .with_context(|| format!("cannot move artifact into {}", path.display()))?;
    Ok(())
}

fn write_instance_files(
    instance: &Path,
    version: &str,
    protocol: i32,
    artifact: &DownloadArtifact,
    relative_jar: &str,
) -> Result<()> {
    fs::create_dir_all(instance)
        .with_context(|| format!("cannot create {}", instance.display()))?;
    fs::create_dir_all(instance.join("versions").join(version))
        .with_context(|| format!("cannot create version directory for {version}"))?;

    let eula = format!(
        "# Written by RoM Bootstrap after explicit user acceptance.\n# Review: {MINECRAFT_EULA_URL}\neula=true\n"
    );
    write_text(instance.join("eula.txt"), &eula)?;
    write_text(instance.join("NOTICE.txt"), INSTANCE_NOTICE)?;
    write_if_missing(instance.join("server.toml"), DEFAULT_SERVER_TOML)?;

    let manifest = BootstrapManifest {
        schema_version: BOOTSTRAP_SCHEMA_VERSION,
        minecraft_version: version.to_owned(),
        protocol,
        patch_set: format!("builtin:{version}"),
        stage: BootstrapStage::OfficialSourceVerified,
        source: SourceRecord {
            kind: "official_server_jar".to_owned(),
            url: artifact.url.clone(),
            sha1: artifact.sha1.clone(),
            size: artifact.size,
            local_path: relative_jar.to_owned(),
        },
        pack: None,
    };
    write_json(instance.join("rom-bootstrap.json"), &manifest)?;
    Ok(())
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context("cannot serialize bootstrap metadata")?;
    let mut bytes_with_newline = bytes;
    bytes_with_newline.push(b'\n');
    write_bytes(path, &bytes_with_newline)
}

fn write_text(path: PathBuf, value: &str) -> Result<()> {
    write_bytes(path, value.as_bytes())
}

fn write_if_missing(path: PathBuf, value: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    write_text(path, value)
}

fn write_bytes(path: PathBuf, value: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let temporary = path.with_extension("tmp");
    let _ = fs::remove_file(&temporary);
    let mut file = File::create(&temporary)
        .with_context(|| format!("cannot create {}", temporary.display()))?;
    file.write_all(value)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    file.sync_all()
        .with_context(|| format!("cannot sync {}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("cannot replace {}", path.display()))?;
    }
    fs::rename(&temporary, &path)
        .with_context(|| format!("cannot move file into {}", path.display()))?;
    Ok(())
}

fn verify_file(path: &Path, expected_sha1: &str, expected_size: u64) -> Result<bool> {
    let metadata = fs::metadata(path).with_context(|| format!("cannot stat {}", path.display()))?;
    if metadata.len() != expected_size {
        return Ok(false);
    }
    Ok(sha1_file(path)?.eq_ignore_ascii_case(expected_sha1))
}

fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut hasher = Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("cannot read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha1_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_official_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).with_context(|| format!("invalid URL: {value}"))?;
    if url.scheme() != "https" || url.port_or_known_default() != Some(443) {
        bail!("official Minecraft files must use HTTPS on the standard port");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("official Minecraft URLs must not include credentials");
    }
    let host = url.host_str().context("URL does not include a host")?;
    let official = host == "mojang.com"
        || host.ends_with(".mojang.com")
        || host == "minecraft.net"
        || host.ends_with(".minecraft.net")
        || host == "aka.ms";
    if !official {
        bail!("refusing non-official Minecraft download host: {host}");
    }
    Ok(url)
}

fn supported_protocol(version: &str) -> Result<i32> {
    match version {
        "26.1.2" => Ok(775),
        _ => bail!("RoM does not contain a built-in protocol profile for Minecraft {version}"),
    }
}

fn validate_version_component(version: &str) -> Result<()> {
    if version.is_empty()
        || version.len() > 64
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        bail!("unsafe or invalid Minecraft version identifier: {version}");
    }
    Ok(())
}

fn eula_is_accepted(path: &Path) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(text.lines().any(|line| {
        let line = line.trim();
        !line.starts_with('#') && line.eq_ignore_ascii_case("eula=true")
    }))
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("cannot determine current directory")?
            .join(path))
    }
}

fn absolute_existing_file(path: &Path) -> Result<PathBuf> {
    let path = absolute_path(path)?;
    if !path.is_file() {
        bail!("file does not exist: {}", path.display());
    }
    Ok(path)
}

fn copy_executable(source: &Path, destination: &Path) -> Result<()> {
    let temporary = destination.with_extension("new");
    let _ = fs::remove_file(&temporary);
    fs::copy(source, &temporary).with_context(|| {
        format!(
            "cannot copy native server from {} to {}",
            source.display(),
            temporary.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755)).with_context(|| {
            format!(
                "cannot set executable permissions on {}",
                temporary.display()
            )
        })?;
    }
    if destination.exists() {
        fs::remove_file(destination)
            .with_context(|| format!("cannot replace {}", destination.display()))?;
    }
    fs::rename(&temporary, destination)
        .with_context(|| format!("cannot install {}", destination.display()))?;
    Ok(())
}

fn native_server_file_name() -> &'static str {
    if cfg!(windows) {
        "rom-server.exe"
    } else {
        "rom-server"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sha1_matches_known_vector() {
        assert_eq!(
            sha1_bytes(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn official_url_filter_rejects_lookalike_hosts_and_plain_http() {
        assert!(ensure_official_url(VERSION_MANIFEST_URL).is_ok());
        assert!(ensure_official_url("https://piston-data.mojang.com/file").is_ok());
        assert!(ensure_official_url("https://evilmojang.com/file").is_err());
        assert!(ensure_official_url("http://piston-data.mojang.com/file").is_err());
    }

    #[test]
    fn version_identifier_cannot_escape_instance_directories() {
        assert!(validate_version_component("26.1.2").is_ok());
        assert!(validate_version_component("26w10a").is_ok());
        assert!(validate_version_component("../server").is_err());
        assert!(validate_version_component("26/1/2").is_err());
    }

    #[test]
    fn parses_official_manifest_shapes() {
        let index: VersionManifest = serde_json::from_str(
            r#"{"versions":[{"id":"26.1.2","url":"https://piston-meta.mojang.com/version.json","sha1":"abc"}]}"#,
        )
        .unwrap();
        assert_eq!(index.versions[0].id, "26.1.2");

        let metadata: VersionMetadata = serde_json::from_str(
            r#"{"id":"26.1.2","downloads":{"server":{"sha1":"deadbeef","size":4,"url":"https://piston-data.mojang.com/server.jar"}}}"#,
        )
        .unwrap();
        assert_eq!(metadata.downloads.server.size, 4);
    }

    #[test]
    fn instance_status_verifies_the_cached_source() {
        let directory = tempdir().unwrap();
        let instance = directory.path();
        let relative = "cache/official/26.1.2/server.jar";
        let jar = instance.join(relative);
        fs::create_dir_all(jar.parent().unwrap()).unwrap();
        fs::write(&jar, b"test").unwrap();
        let artifact = DownloadArtifact {
            sha1: sha1_bytes(b"test"),
            size: 4,
            url: "https://piston-data.mojang.com/server.jar".to_owned(),
        };
        write_instance_files(instance, "26.1.2", 775, &artifact, relative).unwrap();

        let status = status_instance(instance).unwrap();
        assert!(status.prepared);
        assert!(status.minecraft_eula_accepted);
        assert!(status.official_source_verified);
        assert!(!status.version_pack_verified);
        assert!(status.version_pack_path.is_none());
        assert!(!status.native_server_installed);
    }

    #[test]
    fn existing_server_configuration_is_preserved() {
        let directory = tempdir().unwrap();
        let instance = directory.path();
        fs::write(instance.join("server.toml"), "custom = true\n").unwrap();
        let artifact = DownloadArtifact {
            sha1: "00".repeat(20),
            size: 1,
            url: "https://piston-data.mojang.com/server.jar".to_owned(),
        };
        write_instance_files(instance, "26.1.2", 775, &artifact, "cache/server.jar").unwrap();
        assert_eq!(
            fs::read_to_string(instance.join("server.toml")).unwrap(),
            "custom = true\n"
        );
    }
}
