use std::{
    collections::BTreeMap,
    io::{self, ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use ferrum_game::CommandSource;
use serde_json::{Value, json};
use sysinfo::{Disks, System};

use super::ServerState;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 4 * 1024;
const MAX_COMMAND_BYTES: usize = 1_024;
const MAX_TOKEN_BYTES: usize = 256;
const MIN_REMOTE_TOKEN_BYTES: usize = 16;
const ACCEPT_POLL_MILLIS: u64 = 50;
const STREAM_TIMEOUT_SECONDS: u64 = 2;

#[derive(Debug, Clone)]
pub struct AdminGuiConfig {
    pub bind: SocketAddr,
    pub token: Option<String>,
    pub disk_path: PathBuf,
    pub version_name: String,
    pub protocol: i32,
    pub max_players: i32,
}

impl AdminGuiConfig {
    pub fn validate(&self) -> Result<()> {
        if let Some(token) = self.token.as_deref() {
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
        if self.max_players < 0 {
            bail!("admin GUI maximum player count cannot be negative");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct AdminGuiHandle {
    local_addr: SocketAddr,
    join: JoinHandle<()>,
}

impl AdminGuiHandle {
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn join(self) -> Result<()> {
        self.join
            .join()
            .map_err(|_| anyhow::anyhow!("admin GUI worker panicked"))
    }
}

pub fn spawn_admin_gui(
    mut config: AdminGuiConfig,
    state: Arc<ServerState>,
) -> Result<AdminGuiHandle> {
    config.validate()?;
    let listener = TcpListener::bind(config.bind)
        .with_context(|| format!("cannot bind admin GUI on {}", config.bind))?;
    listener
        .set_nonblocking(true)
        .context("cannot configure non-blocking admin GUI listener")?;
    let local_addr = listener
        .local_addr()
        .context("cannot read admin GUI listener address")?;
    config.bind = local_addr;
    let join = thread::Builder::new()
        .name("rom-admin-gui".to_owned())
        .spawn(move || run_admin_gui(listener, config, state))
        .context("cannot spawn admin GUI worker")?;
    Ok(AdminGuiHandle { local_addr, join })
}

fn run_admin_gui(listener: TcpListener, config: AdminGuiConfig, state: Arc<ServerState>) {
    let shutdown = state.shutdown_signal();
    let mut metrics = MetricsCollector::new(config.disk_path.clone());
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _address)) => {
                if let Err(error) = configure_admin_stream(&stream) {
                    eprintln!("cannot configure accepted admin GUI socket: {error}");
                    continue;
                }
                if let Err(error) = handle_connection(&mut stream, &config, &state, &mut metrics) {
                    let _ = write_json_error(&mut stream, 400, &error.to_string());
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(ACCEPT_POLL_MILLIS));
            }
            Err(error) => {
                eprintln!("admin GUI connection failed: {error}");
                thread::sleep(Duration::from_millis(ACCEPT_POLL_MILLIS));
            }
        }
    }
}

fn configure_admin_stream(stream: &TcpStream) -> io::Result<()> {
    // Windows can inherit non-blocking mode from a non-blocking listener. The
    // bounded HTTP parser intentionally uses blocking reads with timeouts.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(STREAM_TIMEOUT_SECONDS)))?;
    stream.set_write_timeout(Some(Duration::from_secs(STREAM_TIMEOUT_SECONDS)))?;
    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    config: &AdminGuiConfig,
    state: &ServerState,
    metrics: &mut MetricsCollector,
) -> Result<()> {
    let request = read_request(stream)?;
    let path = request.path.split('?').next().unwrap_or(&request.path);

    if config.token.is_none() && !is_allowed_loopback_host(&request, config.bind) {
        return write_json_error(
            stream,
            421,
            "Host does not match the local admin GUI listener",
        );
    }

    if request.method == "GET" && path == "/" {
        return write_response(
            stream,
            200,
            "text/html; charset=utf-8",
            ADMIN_HTML.as_bytes(),
        );
    }
    if request.method == "GET" && path == "/favicon.ico" {
        return write_response(stream, 204, "image/x-icon", &[]);
    }
    if !is_authorized(&request, config.token.as_deref()) {
        return write_json_error(stream, 401, "admin token is missing or invalid");
    }

    match (request.method.as_str(), path) {
        ("GET", "/api/status") => match metrics.status_json(config, state) {
            Ok(body) => write_json(stream, 200, &body),
            Err(error) => write_json_error(
                stream,
                500,
                &format!("cannot collect admin status: {error:#}"),
            ),
        },
        ("POST", "/api/command") => handle_command(stream, &request, state),
        _ => write_json_error(stream, 404, "route not found"),
    }
}

fn handle_command(
    stream: &mut TcpStream,
    request: &HttpRequest,
    state: &ServerState,
) -> Result<()> {
    if !request.has_json_content_type() {
        return write_json_error(stream, 415, "Content-Type must be application/json");
    }
    let value: Value = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(error) => {
            return write_json_error(
                stream,
                400,
                &format!("command body is invalid JSON: {error}"),
            );
        }
    };
    let command = match value.get("command").and_then(Value::as_str) {
        Some(command) => command.trim(),
        None => {
            return write_json_error(
                stream,
                400,
                "command body must contain a string field named command",
            );
        }
    };
    if command.is_empty() {
        return write_json_error(stream, 400, "command cannot be empty");
    }
    if command.len() > MAX_COMMAND_BYTES {
        return write_json_error(stream, 413, "command exceeds the 1024-byte limit");
    }

    let outcome = match state
        .game_runtime
        .execute_command(&CommandSource::console(), command)
    {
        Ok(outcome) => outcome,
        Err(error) => return write_json_error(stream, 400, &format!("command failed: {error}")),
    };
    let mut feedback = vec![outcome.feedback.clone()];

    if outcome.save_requested {
        let game_report = match state.game_control().save_now() {
            Ok(report) => report,
            Err(error) => {
                return write_json_error(stream, 500, &format!("game save failed: {error:#}"));
            }
        };
        feedback.push(format!(
            "Saved gameplay state to {} ({} bytes, tick {}, {} players, {} entities)",
            game_report.path.display(),
            game_report.bytes,
            game_report.game_time,
            game_report.players,
            game_report.entities
        ));
        let world_report = match state.world_control().save_now() {
            Ok(report) => report,
            Err(error) => {
                return write_json_error(stream, 500, &format!("world save failed: {error:#}"));
            }
        };
        feedback.push(format!(
            "Saved world state to {} ({} bytes, {} chunks)",
            world_report.path.display(),
            world_report.bytes,
            world_report.chunks
        ));
    }
    if outcome.shutdown_requested {
        state.shutdown_signal().store(true, Ordering::Release);
    }

    write_json(
        stream,
        200,
        &json!({
            "ok": true,
            "command": command,
            "feedback": feedback,
            "save_requested": outcome.save_requested,
            "shutdown_requested": outcome.shutdown_requested,
        }),
    )
}

#[derive(Debug)]
struct MetricsCollector {
    system: System,
    disks: Disks,
    disk_path: PathBuf,
    started: Instant,
}

impl MetricsCollector {
    fn new(disk_path: PathBuf) -> Self {
        let mut system = System::new_all();
        system.refresh_memory();
        system.refresh_cpu_usage();
        Self {
            system,
            disks: Disks::new_with_refreshed_list(),
            disk_path,
            started: Instant::now(),
        }
    }

    fn status_json(&mut self, config: &AdminGuiConfig, state: &ServerState) -> Result<Value> {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.disks.refresh(true);

        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory().min(total_memory);
        let disk = select_disk(&self.disks, &self.disk_path);
        let (disk_total, disk_available, disk_name, disk_mount) = disk.map_or_else(
            || (0, 0, String::new(), String::new()),
            |disk| {
                (
                    disk.total_space(),
                    disk.available_space(),
                    disk.name().to_string_lossy().into_owned(),
                    disk.mount_point().display().to_string(),
                )
            },
        );
        let disk_used = disk_total.saturating_sub(disk_available);
        let (game_time, day_time, authoritative_players, entities) = state
            .game_runtime
            .with_state(|game| {
                let time = game.time();
                (
                    time.game_time,
                    time.day_time,
                    game.online_player_count(),
                    game.entities().len(),
                )
            })
            .context("cannot read game state for admin status")?;

        Ok(json!({
            "server": {
                "version": config.version_name,
                "protocol": config.protocol,
                "uptime_seconds": self.started.elapsed().as_secs(),
                "online_players": state.online_players(),
                "max_players": config.max_players,
                "authoritative_players": authoritative_players,
                "entities": entities,
                "game_time": game_time,
                "day_time": day_time,
                "shutdown_requested": state.shutdown_requested(),
            },
            "system": {
                "supported": sysinfo::IS_SUPPORTED_SYSTEM,
                "cpu_percent": finite_percent(self.system.global_cpu_usage()),
                "cpu_count": self.system.cpus().len(),
                "memory_used_bytes": used_memory,
                "memory_total_bytes": total_memory,
                "memory_percent": percent(used_memory, total_memory),
                "disk_used_bytes": disk_used,
                "disk_total_bytes": disk_total,
                "disk_percent": percent(disk_used, disk_total),
                "disk_name": disk_name,
                "disk_mount": disk_mount,
            }
        }))
    }
}

fn select_disk<'a>(disks: &'a Disks, path: &Path) -> Option<&'a sysinfo::Disk> {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    disks
        .iter()
        .filter(|disk| resolved.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count())
        .or_else(|| disks.iter().max_by_key(|disk| disk.total_space()))
}

fn finite_percent(value: f32) -> f64 {
    if value.is_finite() {
        f64::from(value.clamp(0.0, 100.0))
    } else {
        0.0
    }
}

fn percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((used as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
}

#[derive(Debug, PartialEq, Eq)]
struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn has_json_content_type(&self) -> bool {
        self.headers.get("content-type").is_some_and(|value| {
            value.split(';').next().is_some_and(|media_type| {
                media_type.trim().eq_ignore_ascii_case("application/json")
            })
        })
    }
}

fn read_request<R: Read>(reader: &mut R) -> Result<HttpRequest> {
    let mut buffer = Vec::with_capacity(2_048);
    let header_end = loop {
        if buffer.len() >= MAX_HEADER_BYTES {
            bail!("HTTP request headers exceed the 16 KiB limit");
        }
        let mut chunk = [0_u8; 1_024];
        let count = reader
            .read(&mut chunk)
            .context("cannot read HTTP request")?;
        if count == 0 {
            bail!("HTTP request ended before headers were complete");
        }
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.len() > MAX_HEADER_BYTES {
            bail!("HTTP request headers exceed the 16 KiB limit");
        }
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };

    let header_text = std::str::from_utf8(&buffer[..header_end])
        .context("HTTP request headers are not valid UTF-8")?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().context("HTTP request line is missing")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .context("HTTP method is missing")?
        .to_owned();
    let path = request_parts
        .next()
        .context("HTTP path is missing")?
        .to_owned();
    let version = request_parts.next().context("HTTP version is missing")?;
    if request_parts.next().is_some() || version != "HTTP/1.1" {
        bail!("only well-formed HTTP/1.1 requests are supported");
    }
    if !matches!(method.as_str(), "GET" | "POST") {
        bail!("HTTP method {method} is not supported");
    }
    if !path.starts_with('/') {
        bail!("HTTP path must use origin-form");
    }
    if path
        .bytes()
        .any(|byte| matches!(byte, b'\r' | b'\n' | b'\0'))
    {
        bail!("HTTP path contains invalid characters");
    }

    let mut headers = BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            bail!("folded HTTP headers are not supported");
        }
        let (name, value) = line
            .split_once(':')
            .context("HTTP header is missing a colon")?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() || !name.bytes().all(is_http_token_byte) {
            bail!("HTTP header name is invalid");
        }
        if value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            bail!("HTTP header value contains invalid control characters");
        }
        if headers
            .insert(name.clone(), value.trim().to_owned())
            .is_some()
        {
            bail!("duplicate HTTP header {name}");
        }
    }
    if !headers.contains_key("host") {
        bail!("HTTP/1.1 Host header is required");
    }
    if headers.contains_key("transfer-encoding") {
        bail!("HTTP Transfer-Encoding is not supported");
    }
    let content_length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>())
        .transpose()
        .context("HTTP Content-Length is invalid")?
        .unwrap_or(0);
    if method == "POST" && !headers.contains_key("content-length") {
        bail!("HTTP POST requires Content-Length");
    }
    if content_length > MAX_BODY_BYTES {
        bail!("HTTP request body exceeds the 4 KiB limit");
    }

    let body_start = header_end + 4;
    while buffer.len().saturating_sub(body_start) < content_length {
        let mut chunk = [0_u8; 1_024];
        let count = reader
            .read(&mut chunk)
            .context("cannot read HTTP request body")?;
        if count == 0 {
            bail!("HTTP request body ended early");
        }
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.len().saturating_sub(body_start) > MAX_BODY_BYTES {
            bail!("HTTP request body exceeds the 4 KiB limit");
        }
    }
    let body = buffer[body_start..body_start + content_length].to_vec();
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn is_http_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn is_allowed_loopback_host(request: &HttpRequest, bind: SocketAddr) -> bool {
    let Some(host) = request.headers.get("host") else {
        return false;
    };
    let host = host.trim();
    let port = bind.port();
    for name in ["localhost", "127.0.0.1", "[::1]"] {
        if host.eq_ignore_ascii_case(name) || host.eq_ignore_ascii_case(&format!("{name}:{port}")) {
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

fn is_authorized(request: &HttpRequest, token: Option<&str>) -> bool {
    let Some(expected) = token else {
        return true;
    };
    let Some(value) = request.headers.get("authorization") else {
        return false;
    };
    let Some((scheme, actual)) = value.split_once(' ') else {
        return false;
    };
    scheme.eq_ignore_ascii_case("bearer")
        && constant_time_eq(actual.as_bytes(), expected.as_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let maximum = left.len().max(right.len());
    for index in 0..maximum {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

fn write_json<W: Write>(writer: &mut W, status: u16, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value).context("cannot encode admin JSON response")?;
    write_response(writer, status, "application/json; charset=utf-8", &body)
}

fn write_json_error<W: Write>(writer: &mut W, status: u16, message: &str) -> Result<()> {
    write_json(
        writer,
        status,
        &json!({
            "ok": false,
            "error": message,
        }),
    )
}

fn write_response<W: Write>(
    writer: &mut W,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        500 => "Internal Server Error",
        _ => "Response",
    };
    write!(
        writer,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nCross-Origin-Resource-Policy: same-origin\r\nContent-Security-Policy: default-src 'self'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'\r\n\r\n",
        body.len()
    )
    .context("cannot write admin HTTP response headers")?;
    writer
        .write_all(body)
        .context("cannot write admin HTTP response body")?;
    writer.flush().context("cannot flush admin HTTP response")
}

const ADMIN_HTML: &str = r#"<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>RoM Admin</title>
<style>
:root{color-scheme:dark;--bg:#090c11;--panel:#121823;--line:#283142;--text:#edf3fa;--muted:#91a0b4;--accent:#76e3a6;--danger:#ff7d89}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at top,#19283a 0,#090c11 44%);color:var(--text);font:15px/1.5 system-ui,-apple-system,sans-serif}.wrap{width:min(1060px,100%);margin:auto;padding:24px 16px 48px}header{display:flex;align-items:center;justify-content:space-between;gap:16px;margin-bottom:20px}h1{font-size:24px;margin:0}.status{display:flex;align-items:center;gap:8px;color:var(--muted)}.dot{width:10px;height:10px;border-radius:50%;background:var(--danger);box-shadow:0 0 12px currentColor}.dot.ok{background:var(--accent)}.grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px}.card,.console,.auth{background:rgba(18,24,35,.95);border:1px solid var(--line);border-radius:16px;padding:16px;box-shadow:0 16px 40px #0004}.label{font-size:12px;text-transform:uppercase;letter-spacing:.1em;color:var(--muted)}.value{font-size:28px;font-weight:750;margin:6px 0}.detail{color:var(--muted);font-size:13px;overflow-wrap:anywhere}.bar{height:8px;background:#070a0f;border-radius:999px;overflow:hidden;margin-top:12px}.fill{height:100%;width:0;background:linear-gradient(90deg,#4fc987,#76e3a6);transition:width .35s}.console{margin-top:12px}.console h2{font-size:16px;margin:0 0 12px}.output{height:260px;overflow:auto;background:#06080c;border:1px solid var(--line);border-radius:12px;padding:12px;font:13px/1.55 ui-monospace,SFMono-Regular,Consolas,monospace;white-space:pre-wrap}.row{display:flex;gap:8px;margin-top:10px}input,button{border:1px solid var(--line);border-radius:10px;background:#0b1018;color:var(--text);font:inherit;padding:11px 12px}input{min-width:0;flex:1}button{cursor:pointer;background:#1d633f;border-color:#2e8e5e;font-weight:700}button:disabled{opacity:.55;cursor:wait}.auth{display:none;margin-bottom:12px}.auth.show{display:block}.meta{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px;margin-top:12px}.mini{background:#0b1018;border:1px solid #1f2836;border-radius:10px;padding:10px}.mini strong{display:block;font-size:17px}.mini span{color:var(--muted);font-size:12px}@media(max-width:760px){.grid{grid-template-columns:1fr}.meta{grid-template-columns:repeat(2,minmax(0,1fr))}.value{font-size:25px}.wrap{padding-top:16px}}
</style>
</head>
<body>
<div class="wrap">
<header><div><h1>RoM Admin</h1><div class="detail" id="version">接続中...</div></div><div class="status"><span class="dot" id="dot"></span><span id="health">Offline</span></div></header>
<section class="auth" id="auth"><div class="label">Admin token</div><div class="row"><input id="token" type="password" autocomplete="current-password" placeholder="--admin-token の値"><button id="saveToken">接続</button></div></section>
<section class="grid">
<article class="card"><div class="label">CPU</div><div class="value" id="cpu">--</div><div class="detail" id="cpuDetail">--</div><div class="bar"><div class="fill" id="cpuBar"></div></div></article>
<article class="card"><div class="label">Memory</div><div class="value" id="memory">--</div><div class="detail" id="memoryDetail">--</div><div class="bar"><div class="fill" id="memoryBar"></div></div></article>
<article class="card"><div class="label">Disk</div><div class="value" id="disk">--</div><div class="detail" id="diskDetail">--</div><div class="bar"><div class="fill" id="diskBar"></div></div></article>
</section>
<section class="meta">
<div class="mini"><strong id="players">--</strong><span>Online players</span></div><div class="mini"><strong id="uptime">--</strong><span>Server uptime</span></div><div class="mini"><strong id="gameTime">--</strong><span>Game ticks</span></div><div class="mini"><strong id="entities">--</strong><span>Entities</span></div>
</section>
<section class="console"><h2>Server console</h2><div class="output" id="output">RoM Admin ready. Type help to list commands.</div><form class="row" id="commandForm"><input id="command" maxlength="1024" autocomplete="off" placeholder="help / say hello / list / save-all / stop"><button id="send" type="submit">送信</button></form></section>
</div>
<script>
const $=id=>document.getElementById(id);const fmtBytes=n=>{if(!Number.isFinite(n)||n<=0)return'0 B';const u=['B','KiB','MiB','GiB','TiB'];let i=0;while(n>=1024&&i<u.length-1){n/=1024;i++}return`${n.toFixed(i?1:0)} ${u[i]}`};const fmtTime=s=>{s=Math.max(0,Math.floor(s));const d=Math.floor(s/86400);s%=86400;const h=Math.floor(s/3600);s%=3600;const m=Math.floor(s/60);const q=[];if(d)q.push(`${d}d`);if(h||d)q.push(`${h}h`);q.push(`${m}m`);return q.join(' ')};const pct=n=>`${Number(n||0).toFixed(1)}%`;const token=()=>sessionStorage.getItem('romAdminToken')||'';async function api(path,opt={}){const headers={Accept:'application/json',...(opt.headers||{})};if(token())headers.Authorization=`Bearer ${token()}`;const r=await fetch(path,{...opt,headers,credentials:'same-origin',cache:'no-store'});const j=await r.json().catch(()=>({error:`HTTP ${r.status}`}));if(r.status===401){$('auth').classList.add('show');throw new Error(j.error||'Unauthorized')}if(!r.ok)throw new Error(j.error||`HTTP ${r.status}`);$('auth').classList.remove('show');return j}function bar(id,n){$(id).style.width=`${Math.max(0,Math.min(100,Number(n)||0))}%`}function log(text,bad=false){const line=document.createElement('div');line.textContent=text;if(bad)line.style.color='var(--danger)';$('output').append(document.createTextNode('\n'));$('output').append(line);$('output').scrollTop=$('output').scrollHeight}let refreshing=false;async function refresh(){if(refreshing)return;refreshing=true;try{const d=await api('/api/status');$('dot').classList.add('ok');$('health').textContent='Online';$('version').textContent=`${d.server.version} · protocol ${d.server.protocol}`;$('cpu').textContent=pct(d.system.cpu_percent);$('cpuDetail').textContent=`${d.system.cpu_count} logical CPUs`;$('memory').textContent=pct(d.system.memory_percent);$('memoryDetail').textContent=`${fmtBytes(d.system.memory_used_bytes)} / ${fmtBytes(d.system.memory_total_bytes)}`;$('disk').textContent=pct(d.system.disk_percent);$('diskDetail').textContent=`${fmtBytes(d.system.disk_used_bytes)} / ${fmtBytes(d.system.disk_total_bytes)} · ${d.system.disk_mount||'unknown mount'}`;$('players').textContent=`${d.server.online_players} / ${d.server.max_players}`;$('uptime').textContent=fmtTime(d.server.uptime_seconds);$('gameTime').textContent=Number(d.server.game_time).toLocaleString();$('entities').textContent=d.server.entities;bar('cpuBar',d.system.cpu_percent);bar('memoryBar',d.system.memory_percent);bar('diskBar',d.system.disk_percent)}catch(_){$('dot').classList.remove('ok');$('health').textContent='Offline'}finally{refreshing=false}}$('saveToken').onclick=()=>{sessionStorage.setItem('romAdminToken',$('token').value);refresh()};$('commandForm').onsubmit=async e=>{e.preventDefault();const command=$('command').value.trim();if(!command)return;const send=$('send');send.disabled=true;log(`> ${command}`);try{const r=await api('/api/command',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command})});for(const line of r.feedback||[])log(line);$('command').value=''}catch(error){log(error.message,true)}finally{send.disabled=false;$('command').focus()}};refresh();setInterval(refresh,1000);
</script>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn request(headers: impl IntoIterator<Item = (&'static str, &'static str)>) -> HttpRequest {
        HttpRequest {
            method: "GET".to_owned(),
            path: "/api/status".to_owned(),
            headers: headers
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
            body: Vec::new(),
        }
    }

    #[test]
    fn non_loopback_bind_requires_a_token() {
        let config = AdminGuiConfig {
            bind: "0.0.0.0:25575".parse().unwrap(),
            token: None,
            disk_path: PathBuf::from("."),
            version_name: "test".to_owned(),
            protocol: 1,
            max_players: 20,
        };
        assert!(config.validate().is_err());
        let short = AdminGuiConfig {
            token: Some("short".to_owned()),
            ..config.clone()
        };
        assert!(short.validate().is_err());
        let secure = AdminGuiConfig {
            token: Some("correct-horse-42".to_owned()),
            ..config
        };
        assert!(secure.validate().is_ok());
    }

    #[test]
    fn bearer_authentication_handles_lengths_without_early_return() {
        let valid = request([("authorization", "Bearer correct")]);
        assert!(is_authorized(&valid, Some("correct")));
        assert!(!is_authorized(&valid, Some("wrong")));
        assert!(!is_authorized(&valid, Some("correct-and-longer")));
        assert!(is_authorized(&valid, None));

        let lowercase = request([("authorization", "bearer correct")]);
        assert!(is_authorized(&lowercase, Some("correct")));
    }

    #[test]
    fn parses_a_bounded_json_request() {
        let raw = b"POST /api/command HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: 18\r\n\r\n{\"command\":\"help\"}";
        let parsed = read_request(&mut Cursor::new(raw)).unwrap();
        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.path, "/api/command");
        assert_eq!(parsed.body, b"{\"command\":\"help\"}");
        assert!(parsed.has_json_content_type());
    }

    #[test]
    fn rejects_duplicate_or_chunked_headers() {
        let duplicate = b"GET /api/status HTTP/1.1\r\nHost: localhost\r\nHost: other\r\n\r\n";
        assert!(read_request(&mut Cursor::new(duplicate)).is_err());
        let chunked =
            b"POST /api/command HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(read_request(&mut Cursor::new(chunked)).is_err());
    }

    #[test]
    fn requires_host_and_post_content_length() {
        let no_host = b"GET /api/status HTTP/1.1\r\n\r\n";
        assert!(read_request(&mut Cursor::new(no_host)).is_err());
        let no_length = b"POST /api/command HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(read_request(&mut Cursor::new(no_length)).is_err());
    }

    #[test]
    fn percentages_are_bounded() {
        assert_eq!(percent(50, 100), 50.0);
        assert_eq!(percent(1, 0), 0.0);
        assert_eq!(percent(200, 100), 100.0);
        assert_eq!(finite_percent(f32::NAN), 0.0);
    }

    #[test]
    fn response_includes_security_headers() {
        let mut output = Vec::new();
        write_response(&mut output, 200, "text/plain", b"ok").unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("X-Frame-Options: DENY"));
        assert!(output.contains("Content-Security-Policy:"));
        assert!(output.ends_with("\r\n\r\nok"));
    }

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
            &format!("GET /api/status HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"),
        );
        assert!(status.starts_with("HTTP/1.1 200 OK"));
        assert!(status.contains("\"cpu_percent\""));

        let body = r#"{"command":"help"}"#;
        let command = exchange(
            address,
            &format!(
                "POST /api/command HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        );
        assert!(command.starts_with("HTTP/1.1 200 OK"));
        assert!(command.contains("\"ok\":true"));

        state.shutdown_signal().store(true, Ordering::Release);
        handle.join().unwrap();
        let state = match Arc::try_unwrap(state) {
            Ok(state) => state,
            Err(_) => panic!("admin GUI retained the server state after shutdown"),
        };
        state.shutdown().unwrap();
    }

    #[test]
    fn embedded_page_contains_metrics_and_console() {
        assert!(ADMIN_HTML.contains("CPU"));
        assert!(ADMIN_HTML.contains("Memory"));
        assert!(ADMIN_HTML.contains("Disk"));
        assert!(ADMIN_HTML.contains("/api/command"));
        assert!(ADMIN_HTML.contains("Content-Type':'application/json"));
    }
}
