#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text()
    if new in text:
        return
    if old not in text:
        raise SystemExit(f"missing {label} anchor in {path}")
    file.write_text(text.replace(old, new, 1))


# Admin GUI: constrain secrets, reject DNS rebinding on unauthenticated loopback,
# classify internal errors as 500, and add a real socket-level smoke test.
path = Path("crates/ferrum-server/src/admin_gui.rs")
text = path.read_text()
text = text.replace(
    "const MAX_COMMAND_BYTES: usize = 1_024;\n",
    "const MAX_COMMAND_BYTES: usize = 1_024;\nconst MAX_TOKEN_BYTES: usize = 256;\nconst MIN_REMOTE_TOKEN_BYTES: usize = 16;\n",
    1,
)
old_validate = '''        if self
            .token
            .as_deref()
            .is_some_and(|token| token.trim().is_empty())
        {
            bail!("admin GUI token cannot be empty");
        }
        if !self.bind.ip().is_loopback() && self.token.is_none() {
            bail!(
                "admin GUI bind {} is not loopback; provide --admin-token before exposing it",
                self.bind
            );
        }
'''
new_validate = '''        if let Some(token) = self.token.as_deref() {
            if token.is_empty() || token.len() > MAX_TOKEN_BYTES {
                bail!("admin GUI token must contain 1..={MAX_TOKEN_BYTES} bytes");
            }
            if !token.bytes().all(|byte| byte.is_ascii_graphic()) {
                bail!("admin GUI token must contain visible ASCII characters only");
            }
            if !self.bind.ip().is_loopback() && token.len() < MIN_REMOTE_TOKEN_BYTES {
                bail!(
                    "a non-loopback admin GUI bind requires a token of at least {MIN_REMOTE_TOKEN_BYTES} bytes"
                );
            }
        } else if !self.bind.ip().is_loopback() {
            bail!(
                "admin GUI bind {} is not loopback; provide --admin-token before exposing it",
                self.bind
            );
        }
'''
if new_validate not in text:
    if old_validate not in text:
        raise SystemExit("missing admin token validation anchor")
    text = text.replace(old_validate, new_validate, 1)
text = text.replace(
    "pub fn spawn_admin_gui(config: AdminGuiConfig, state: Arc<ServerState>) -> Result<AdminGuiHandle> {",
    "pub fn spawn_admin_gui(mut config: AdminGuiConfig, state: Arc<ServerState>) -> Result<AdminGuiHandle> {",
    1,
)
old_addr = '''    let local_addr = listener
        .local_addr()
        .context("cannot read admin GUI listener address")?;
    let join = thread::Builder::new()
'''
new_addr = '''    let local_addr = listener
        .local_addr()
        .context("cannot read admin GUI listener address")?;
    config.bind = local_addr;
    let join = thread::Builder::new()
'''
if new_addr not in text:
    if old_addr not in text:
        raise SystemExit("missing admin local address anchor")
    text = text.replace(old_addr, new_addr, 1)
old_path = '''    let request = read_request(stream)?;
    let path = request.path.split('?').next().unwrap_or(&request.path);

    if request.method == "GET" && path == "/" {
'''
new_path = '''    let request = read_request(stream)?;
    let path = request.path.split('?').next().unwrap_or(&request.path);

    if config.token.is_none() && !is_allowed_loopback_host(&request, config.bind) {
        return write_json_error(
            stream,
            421,
            "Host does not match the local admin GUI listener",
        );
    }

    if request.method == "GET" && path == "/" {
'''
if new_path not in text:
    if old_path not in text:
        raise SystemExit("missing admin Host validation anchor")
    text = text.replace(old_path, new_path, 1)
old_status = '''        ("GET", "/api/status") => {
            let body = metrics.status_json(config, state)?;
            write_json(stream, 200, &body)
        }
'''
new_status = '''        ("GET", "/api/status") => match metrics.status_json(config, state) {
            Ok(body) => write_json(stream, 200, &body),
            Err(error) => write_json_error(
                stream,
                500,
                &format!("cannot collect admin status: {error:#}"),
            ),
        },
'''
if new_status not in text:
    if old_status not in text:
        raise SystemExit("missing admin status route anchor")
    text = text.replace(old_status, new_status, 1)
old_header_control = r'''        if value
            .bytes()
            .any(|byte| matches!(byte, b'\r' | b'\n' | b'\0'))
        {
            bail!("HTTP header value contains invalid characters");
        }
'''
new_header_control = r'''        if value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            bail!("HTTP header value contains invalid control characters");
        }
'''
if new_header_control not in text:
    if old_header_control not in text:
        raise SystemExit("missing HTTP header control anchor")
    text = text.replace(old_header_control, new_header_control, 1)
text = text.replace("        method: method.to_owned(),\n        path: path.to_owned(),", "        method,\n        path,", 1)
host_helper = '''fn is_allowed_loopback_host(request: &HttpRequest, bind: SocketAddr) -> bool {
    let Some(host) = request.headers.get("host") else {
        return false;
    };
    let host = host.trim();
    let port = bind.port();
    for name in ["localhost", "127.0.0.1", "[::1]"] {
        if host.eq_ignore_ascii_case(name)
            || host.eq_ignore_ascii_case(&format!("{name}:{port}"))
        {
            return true;
        }
    }
    let exact = if bind.ip().is_ipv6() {
        format!("[{}]:{port}", bind.ip())
    } else {
        format!("{}:{port}", bind.ip())
    };
    host.eq_ignore_ascii_case(&exact)
}

'''
if host_helper not in text:
    marker = "fn is_authorized(request: &HttpRequest, token: Option<&str>) -> bool {\n"
    if marker not in text:
        raise SystemExit("missing authorization helper anchor")
    text = text.replace(marker, host_helper + marker, 1)
text = text.replace('        415 => "Unsupported Media Type",\n        500 => "Internal Server Error",', '        415 => "Unsupported Media Type",\n        421 => "Misdirected Request",\n        500 => "Internal Server Error",', 1)
old_config_test = '''        let secure = AdminGuiConfig {
            token: Some("secret".to_owned()),
            ..config
        };
        assert!(secure.validate().is_ok());
'''
new_config_test = '''        let short = AdminGuiConfig {
            token: Some("short".to_owned()),
            ..config.clone()
        };
        assert!(short.validate().is_err());
        let secure = AdminGuiConfig {
            token: Some("correct-horse-42".to_owned()),
            ..config
        };
        assert!(secure.validate().is_ok());
'''
if new_config_test not in text:
    if old_config_test not in text:
        raise SystemExit("missing admin config test anchor")
    text = text.replace(old_config_test, new_config_test, 1)
extra_tests = '''
    #[test]
    fn token_and_host_validation_reject_ambiguous_inputs() {
        let loopback = AdminGuiConfig {
            bind: "127.0.0.1:25575".parse().unwrap(),
            token: Some("contains space".to_owned()),
            disk_path: PathBuf::from("."),
            version_name: "test".to_owned(),
            protocol: 1,
            max_players: 20,
        };
        assert!(loopback.validate().is_err());

        let local = request([("host", "127.0.0.1:25575")]);
        assert!(is_allowed_loopback_host(
            &local,
            "127.0.0.1:25575".parse().unwrap()
        ));
        let localhost = request([("host", "localhost:25575")]);
        assert!(is_allowed_loopback_host(
            &localhost,
            "127.0.0.1:25575".parse().unwrap()
        ));
        let rebound = request([("host", "attacker.example:25575")]);
        assert!(!is_allowed_loopback_host(
            &rebound,
            "127.0.0.1:25575".parse().unwrap()
        ));
    }

    #[test]
    fn live_dashboard_serves_status_and_authoritative_commands() {
        use std::net::Shutdown;

        fn exchange(address: SocketAddr, request: &str) -> String {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            stream.shutdown(Shutdown::Write).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        }

        let state = Arc::new(ServerState::new(&crate::ServerConfig::default()));
        let handle = spawn_admin_gui(
            AdminGuiConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                token: None,
                disk_path: PathBuf::from("."),
                version_name: "test".to_owned(),
                protocol: 1,
                max_players: 20,
            },
            Arc::clone(&state),
        )
        .unwrap();
        let address = handle.local_addr();
        let status = exchange(
            address,
            &format!(
                "GET /api/status HTTP/1.1\\r\\nHost: {address}\\r\\nConnection: close\\r\\n\\r\\n"
            ),
        );
        assert!(status.starts_with("HTTP/1.1 200 OK"));
        assert!(status.contains("\\\"cpu_percent\\\""));

        let body = r#"{"command":"help"}"#;
        let command = exchange(
            address,
            &format!(
                "POST /api/command HTTP/1.1\\r\\nHost: {address}\\r\\nContent-Type: application/json\\r\\nContent-Length: {}\\r\\nConnection: close\\r\\n\\r\\n{body}",
                body.len()
            ),
        );
        assert!(command.starts_with("HTTP/1.1 200 OK"));
        assert!(command.contains("\\\"ok\\\":true"));

        state.shutdown_signal().store(true, Ordering::Release);
        handle.join().unwrap();
        let state = match Arc::try_unwrap(state) {
            Ok(state) => state,
            Err(_) => panic!("admin GUI retained the server state after shutdown"),
        };
        state.shutdown().unwrap();
    }
'''
if "fn live_dashboard_serves_status_and_authoritative_commands" not in text:
    marker = "\n    #[test]\n    fn embedded_page_contains_metrics_and_console() {"
    if marker not in text:
        raise SystemExit("missing admin final test anchor")
    text = text.replace(marker, extra_tests + marker, 1)
path.write_text(text)

# World snapshot: fluid counts must not exceed the actual non-air block count.
replace_once(
    "crates/ferrum-world/src/persistence.rs",
    "                if usize::from(section.fluid_count) > BLOCKS_PER_SECTION {",
    "                if section.fluid_count > non_empty_block_count {",
    "world fluid count validation",
)
path = Path("crates/ferrum-world/src/persistence.rs")
text = path.read_text()
fluid_test = '''
    #[test]
    fn rejects_fluid_count_greater_than_non_air_blocks() {
        let mut snapshot = store().snapshot();
        let section = &mut snapshot.chunks[0].sections[0];
        section.blocks.fill(section.air);
        section.fluid_count = 1;
        assert!(matches!(
            snapshot.restore(),
            Err(WorldPersistenceError::InvalidFluidCount { .. })
        ));
    }
'''
if "fn rejects_fluid_count_greater_than_non_air_blocks" not in text:
    marker = "\n    #[test]\n    fn rejects_duplicate_chunk_positions() {"
    if marker not in text:
        raise SystemExit("missing world persistence test anchor")
    text = text.replace(marker, fluid_test + marker, 1)
path.write_text(text)

# Bootstrap: avoid a 64 KiB stack allocation on constrained Android shells.
replace_once(
    "crates/rom-bootstrap/src/lib.rs",
    "    let mut buffer = [0_u8; 64 * 1024];",
    "    let mut buffer = vec![0_u8; 64 * 1024];",
    "bootstrap SHA-1 buffer",
)

# Importer: bound archive entry counts, class decompression, and manifest reads.
path = Path("crates/ferrum-importer/src/lib.rs")
text = path.read_text()
text = text.replace(
    "use zip::ZipArchive;\n",
    "use zip::ZipArchive;\n\nconst MAX_ARCHIVE_ENTRIES: usize = 250_000;\nconst MAX_CLASS_BYTES: u64 = 16 * 1024 * 1024;\nconst MAX_MANIFEST_BYTES: u64 = 1024 * 1024;\n",
    1,
)
old_error_tail = '''    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
}
'''
new_error_tail = '''    Zip {
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
'''
if new_error_tail not in text:
    if old_error_tail not in text:
        raise SystemExit("missing importer error anchor")
    text = text.replace(old_error_tail, new_error_tail, 1)
old_archive = '''    let mut archive = ZipArchive::new(file).map_err(|source| ImportError::Zip {
        path: path.to_owned(),
        source,
    })?;

    let archive_entries = archive.len();
'''
new_archive = '''    let mut archive = ZipArchive::new(file).map_err(|source| ImportError::Zip {
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
'''
if new_archive not in text:
    if old_archive not in text:
        raise SystemExit("missing importer archive anchor")
    text = text.replace(old_archive, new_archive, 1)
old_class_read = '''        class_entries_seen += 1;
        let mut bytes = Vec::with_capacity(entry.size().min(16 * 1024 * 1024) as usize);
        if let Err(error) = entry.read_to_end(&mut bytes) {
            errors.push(EntryError {
                archive_path: entry_name,
                stage: ErrorStage::ArchiveRead,
                message: error.to_string(),
                classfile_major: None,
                classfile_minor: None,
            });
            continue;
        }

        match inspect_class_bytes_with_options(
'''
new_class_read = '''        class_entries_seen += 1;
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
'''
if new_class_read not in text:
    if old_class_read not in text:
        raise SystemExit("missing importer class read anchor")
    text = text.replace(old_class_read, new_class_read, 1)
old_manifest = '''fn read_manifest<R: Read + io::Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
    let mut entry = archive.by_name("META-INF/MANIFEST.MF").ok()?;
    let mut manifest = String::new();
    entry.read_to_string(&mut manifest).ok()?;
    Some(manifest)
}
'''
new_manifest = '''fn read_manifest<R: Read + io::Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
    let mut entry = archive.by_name("META-INF/MANIFEST.MF").ok()?;
    let expected_size = entry.size();
    let bytes = read_bounded_bytes(&mut entry, expected_size, MAX_MANIFEST_BYTES)
        .ok()??;
    String::from_utf8(bytes).ok()
}

fn read_bounded_bytes(
    mut reader: impl Read,
    expected_size: u64,
    limit: u64,
) -> io::Result<Option<Vec<u8>>> {
    if expected_size > limit {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(expected_size as usize);
    reader.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != expected_size || bytes.len() as u64 > limit {
        return Ok(None);
    }
    Ok(Some(bytes))
}
'''
if new_manifest not in text:
    if old_manifest not in text:
        raise SystemExit("missing importer manifest anchor")
    text = text.replace(old_manifest, new_manifest, 1)
import_test = '''
    #[test]
    fn bounded_reader_rejects_oversized_or_mismatched_entries() {
        let exact = read_bounded_bytes(&b"abcd"[..], 4, 4).unwrap();
        assert_eq!(exact.unwrap(), b"abcd");
        assert!(read_bounded_bytes(&b"abcde"[..], 5, 4).unwrap().is_none());
        assert!(read_bounded_bytes(&b"abc"[..], 4, 4).unwrap().is_none());
    }
'''
if "fn bounded_reader_rejects_oversized_or_mismatched_entries" not in text:
    marker = "\n    #[test]\n    fn reads_header_version_without_full_parse() {"
    if marker not in text:
        raise SystemExit("missing importer test anchor")
    text = text.replace(marker, import_test + marker, 1)
path.write_text(text)

# Fabric analysis: bound archive entry count and all JSON metadata reads.
path = Path("crates/ferrum-cli/src/fabric.rs")
text = path.read_text()
text = text.replace(
    "use zip::ZipArchive;\n",
    "use zip::ZipArchive;\n\nconst MAX_ARCHIVE_ENTRIES: usize = 250_000;\nconst MAX_JSON_ENTRY_BYTES: u64 = 4 * 1024 * 1024;\n",
    1,
)
old_fabric_archive = '''    let mut archive =
        ZipArchive::new(file).with_context(|| format!("cannot read JAR {}", path.display()))?;
    let mut warnings = Vec::new();
'''
new_fabric_archive = '''    let mut archive =
        ZipArchive::new(file).with_context(|| format!("cannot read JAR {}", path.display()))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        anyhow::bail!(
            "JAR {} contains {} entries, exceeding limit {MAX_ARCHIVE_ENTRIES}",
            path.display(),
            archive.len()
        );
    }
    let mut warnings = Vec::new();
'''
if new_fabric_archive not in text:
    if old_fabric_archive not in text:
        raise SystemExit("missing Fabric archive anchor")
    text = text.replace(old_fabric_archive, new_fabric_archive, 1)
old_json = '''    let Ok(mut entry) = archive.by_name(name) else {
        return Ok(None);
    };
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .with_context(|| format!("cannot read {name}"))?;
    serde_json::from_str(&text)
'''
new_json = '''    let Ok(mut entry) = archive.by_name(name) else {
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
'''
if new_json not in text:
    if old_json not in text:
        raise SystemExit("missing Fabric JSON anchor")
    text = text.replace(old_json, new_json, 1)
path.write_text(text)

# Document transport expectations for remote access.
path = Path("README.md")
text = path.read_text()
old_doc = '''A non-loopback bind is rejected unless `--admin-token` is supplied. On Termux
or Pixel Terminal, leave the loopback default and open
`http://127.0.0.1:25575` in the Android browser.
'''
new_doc = '''A non-loopback bind is rejected unless `--admin-token` is supplied; remote
binds require at least 16 visible ASCII characters. The built-in endpoint is
plain HTTP, so expose it only on a trusted LAN or place it behind a TLS reverse
proxy. On Termux or Pixel Terminal, leave the loopback default and open
`http://127.0.0.1:25575` in the Android browser.
'''
if new_doc not in text:
    if old_doc not in text:
        raise SystemExit("missing README admin security anchor")
    text = text.replace(old_doc, new_doc, 1)
path.write_text(text)
