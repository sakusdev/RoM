use anyhow::{Context, Result, bail};
use clap::Parser;
mod codec;
mod identity;
#[cfg(test)]
use codec::read_varint_io;
use codec::{
    PacketReader, build_packet, read_packet, write_packet, write_string, write_varint_vec,
};
use identity::offline_player_identity;
use serde_json::{Map, Value, json};
use std::{
    fs,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
    time::Duration,
};

const DEFAULT_BIND: &str = "127.0.0.1:25565";
const DEFAULT_VERSION_NAME: &str = "Minecraft Java Edition 26.*.*";
const DEFAULT_PROTOCOL: i32 = 0;
const DEFAULT_MOTD: &str = "Ferrum native Rust server";

#[derive(Debug, Parser)]
#[command(
    name = "ferrum-server",
    version,
    about = "Native Rust Minecraft-compatible server runtime"
)]
struct Cli {
    /// Path to the server configuration file.
    #[arg(long, value_name = "PATH")]
    config: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerConfig {
    bind: String,
    version_name: String,
    protocol: i32,
    motd: String,
    max_players: i32,
    online_players: i32,
    login_disconnect_message: String,
    allow_offline_login: bool,
    online_mode: bool,
    hide_online_players: bool,
    enforces_secure_chat: bool,
    previews_chat: bool,
    server_icon: Option<String>,
    sample_players: Vec<SamplePlayer>,
    packets: PacketIds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SamplePlayer {
    name: String,
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PacketIds {
    handshake_serverbound: i32,
    status_request_serverbound: i32,
    status_response_clientbound: i32,
    ping_request_serverbound: i32,
    pong_response_clientbound: i32,
    login_start_serverbound: i32,
    login_disconnect_clientbound: i32,
    login_success_clientbound: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Status,
    Login,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Handshake {
    protocol: i32,
    server_address: String,
    server_port: u16,
    next_state: i32,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    if !cli.config.is_file() {
        bail!(
            "server config {} does not exist or is not a file",
            cli.config.display()
        );
    }

    let config_path = cli
        .config
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", cli.config.display()))?;
    let config = ServerConfig::from_file(&config_path)
        .with_context(|| format!("cannot load {}", config_path.display()))?;
    let listener = TcpListener::bind(&config.bind)
        .with_context(|| format!("cannot bind Minecraft status listener on {}", config.bind))?;
    println!(
        "ferrum-server listening on {} as {} protocol {}",
        config.bind, config.version_name, config.protocol
    );

    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                let config = config.clone();
                thread::spawn(move || {
                    if let Err(error) = handle_client(&mut stream, &config) {
                        eprintln!("connection closed: {error:#}");
                    }
                });
            }
            Err(error) => eprintln!("incoming connection failed: {error}"),
        }
    }
    Ok(())
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND.to_owned(),
            version_name: DEFAULT_VERSION_NAME.to_owned(),
            protocol: DEFAULT_PROTOCOL,
            motd: DEFAULT_MOTD.to_owned(),
            max_players: 20,
            online_players: 0,
            login_disconnect_message: "Ferrum native server currently implements status ping only"
                .to_owned(),
            allow_offline_login: false,
            online_mode: false,
            hide_online_players: false,
            enforces_secure_chat: false,
            previews_chat: false,
            server_icon: None,
            sample_players: Vec::new(),
            packets: PacketIds::default(),
        }
    }
}

impl Default for PacketIds {
    fn default() -> Self {
        Self {
            handshake_serverbound: 0,
            status_request_serverbound: 0,
            status_response_clientbound: 0,
            ping_request_serverbound: 1,
            pong_response_clientbound: 1,
            login_start_serverbound: 0,
            login_disconnect_clientbound: 0,
            login_success_clientbound: 2,
        }
    }
}

impl ServerConfig {
    fn from_file(path: &PathBuf) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let base_dir = path.parent().map(PathBuf::from).unwrap_or_default();
        Self::from_toml_like_with_base(&text, Some(&base_dir))
    }

    fn from_toml_like_with_base(text: &str, base_dir: Option<&PathBuf>) -> Result<Self> {
        let mut config = Self::default();
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
                bail!("invalid config line {}: {}", line_index + 1, raw_line);
            };
            let key = key.trim();
            let value = value.trim();
            match (section.as_str(), key) {
                ("server", "bind") => config.bind = parse_string(value),
                ("server", "version_name") => config.version_name = parse_string(value),
                ("server", "protocol") => config.protocol = parse_i32(value, line_index + 1)?,
                ("server", "motd") => config.motd = parse_string(value),
                ("server", "max_players") => config.max_players = parse_i32(value, line_index + 1)?,
                ("server", "online_players") => {
                    config.online_players = parse_i32(value, line_index + 1)?
                }
                ("server", "login_disconnect_message") => {
                    config.login_disconnect_message = parse_string(value)
                }
                ("server", "allow_offline_login") => {
                    config.allow_offline_login = parse_bool(value, line_index + 1)?
                }
                ("server", "online_mode") => {
                    config.online_mode = parse_bool(value, line_index + 1)?
                }
                ("server", "hide_online_players") => {
                    config.hide_online_players = parse_bool(value, line_index + 1)?
                }
                ("server", "enforces_secure_chat") => {
                    config.enforces_secure_chat = parse_bool(value, line_index + 1)?
                }
                ("server", "previews_chat") => {
                    config.previews_chat = parse_bool(value, line_index + 1)?
                }
                ("server", "server_icon") => {
                    config.server_icon = Some(load_server_icon(
                        &parse_string(value),
                        base_dir,
                        line_index + 1,
                    )?)
                }
                ("server", "sample_players") => {
                    config.sample_players = parse_sample_players(&parse_string(value))?
                }
                ("protocol", "handshake_serverbound") => {
                    config.packets.handshake_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "status_request_serverbound") => {
                    config.packets.status_request_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "status_response_clientbound") => {
                    config.packets.status_response_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "ping_request_serverbound") => {
                    config.packets.ping_request_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "pong_response_clientbound") => {
                    config.packets.pong_response_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_start_serverbound") => {
                    config.packets.login_start_serverbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_disconnect_clientbound") => {
                    config.packets.login_disconnect_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_success_clientbound") => {
                    config.packets.login_success_clientbound = parse_i32(value, line_index + 1)?
                }
                _ => bail!("unknown config key [{section}].{key}"),
            }
        }
        if config.max_players < 0 || config.online_players < 0 {
            bail!("player counts must be non-negative");
        }
        if config.online_players > config.max_players {
            bail!("online_players cannot exceed max_players");
        }
        Ok(config)
    }

    fn status_json(&self) -> String {
        let mut root = Map::new();
        root.insert(
            "version".to_owned(),
            json!({
                "name": self.version_name,
                "protocol": self.protocol,
            }),
        );
        if !self.hide_online_players {
            let mut players = Map::new();
            players.insert("max".to_owned(), json!(self.max_players));
            players.insert("online".to_owned(), json!(self.online_players));
            if !self.sample_players.is_empty() {
                players.insert(
                    "sample".to_owned(),
                    Value::Array(
                        self.sample_players
                            .iter()
                            .map(|player| {
                                json!({
                                    "name": &player.name,
                                    "id": &player.id,
                                })
                            })
                            .collect(),
                    ),
                );
            }
            root.insert("players".to_owned(), Value::Object(players));
        }
        root.insert(
            "description".to_owned(),
            json!({
                "text": self.motd,
            }),
        );
        root.insert(
            "enforcesSecureChat".to_owned(),
            json!(self.enforces_secure_chat),
        );
        root.insert("previewsChat".to_owned(), json!(self.previews_chat));
        if let Some(favicon) = &self.server_icon {
            root.insert("favicon".to_owned(), json!(favicon));
        }
        Value::Object(root).to_string()
    }

    fn login_disconnect_json(&self) -> String {
        json!({
            "text": self.login_disconnect_message,
        })
        .to_string()
    }
}

fn parse_string(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn parse_i32(value: &str, line: usize) -> Result<i32> {
    value
        .parse()
        .with_context(|| format!("line {line} is not a valid i32: {value}"))
}

fn parse_bool(value: &str, line: usize) -> Result<bool> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => bail!("line {line} is not a valid bool: {other}"),
    }
}

fn parse_sample_players(value: &str) -> Result<Vec<SamplePlayer>> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let Some((name, id)) = entry.split_once(':') else {
                bail!("sample player entry must be name:uuid, got {entry}");
            };
            let name = name.trim();
            let id = id.trim();
            if name.is_empty() || id.is_empty() {
                bail!("sample player entry must include non-empty name and uuid");
            }
            Ok(SamplePlayer {
                name: name.to_owned(),
                id: id.to_owned(),
            })
        })
        .collect()
}

fn load_server_icon(value: &str, base_dir: Option<&PathBuf>, line: usize) -> Result<String> {
    if value.starts_with("data:image/png;base64,") {
        return Ok(value.to_owned());
    }
    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else if let Some(base_dir) = base_dir {
        base_dir.join(path)
    } else {
        path
    };
    let bytes =
        fs::read(&path).with_context(|| format!("line {line}: cannot read {}", path.display()))?;
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        bail!("line {line}: server_icon must point to a PNG file");
    }
    Ok(format!("data:image/png;base64,{}", base64_encode(&bytes)))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);

        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(third & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn handle_client(stream: &mut TcpStream, config: &ServerConfig) -> Result<()> {
    let mut reader = stream.try_clone().context("cannot clone TCP stream")?;
    handle_connection_protocol(&mut reader, stream, config)
}

fn handle_connection_protocol<R: Read, W: Write>(
    mut reader: R,
    writer: W,
    config: &ServerConfig,
) -> Result<()> {
    let handshake_packet = read_packet(&mut reader).context("cannot read handshake packet")?;
    let handshake = parse_handshake_packet(&handshake_packet, &config.packets)?;
    match handshake.connection_state()? {
        ConnectionState::Status => handle_status_protocol(reader, writer, config),
        ConnectionState::Login => handle_login_protocol(reader, writer, config, &handshake),
    }
}

impl Handshake {
    fn connection_state(&self) -> Result<ConnectionState> {
        match self.next_state {
            1 => Ok(ConnectionState::Status),
            2 => Ok(ConnectionState::Login),
            other => bail!("unsupported handshake next_state {other}"),
        }
    }
}

fn handle_status_protocol<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    config: &ServerConfig,
) -> Result<()> {
    let request_packet = read_packet(&mut reader).context("cannot read status request")?;
    let mut request_reader = PacketReader::new(&request_packet);
    let request_id = request_reader.read_varint()?;
    if request_id != config.packets.status_request_serverbound {
        bail!(
            "expected status request packet id {}, got {request_id}",
            config.packets.status_request_serverbound
        );
    }
    write_packet(
        &mut writer,
        &build_packet(config.packets.status_response_clientbound, |body| {
            write_string(body, &config.status_json())
        })?,
    )?;

    match read_packet(&mut reader) {
        Ok(ping_packet) => {
            let mut ping_reader = PacketReader::new(&ping_packet);
            let ping_id = ping_reader.read_varint()?;
            if ping_id != config.packets.ping_request_serverbound {
                bail!(
                    "expected ping packet id {}, got {ping_id}",
                    config.packets.ping_request_serverbound
                );
            }
            let payload = ping_reader.read_i64()?;
            write_packet(
                &mut writer,
                &build_packet(config.packets.pong_response_clientbound, |body| {
                    body.extend_from_slice(&payload.to_be_bytes());
                    Ok(())
                })?,
            )?;
        }
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {}
        Err(error) => return Err(error).context("cannot read ping packet"),
    }

    writer.flush()?;
    Ok(())
}

fn handle_login_protocol<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    config: &ServerConfig,
    handshake: &Handshake,
) -> Result<()> {
    let login_packet = read_packet(&mut reader).context("cannot read login start packet")?;
    let mut login_reader = PacketReader::new(&login_packet);
    let packet_id = login_reader.read_varint()?;
    if packet_id != config.packets.login_start_serverbound {
        bail!(
            "expected login start packet id {}, got {packet_id}",
            config.packets.login_start_serverbound
        );
    }
    let username = login_reader.read_string()?;
    let identity = offline_player_identity(&username);
    println!(
        "login attempt from {} ({}) for {}:{} using protocol {} online_mode={}",
        identity.username,
        identity.uuid.hyphenated(),
        handshake.server_address,
        handshake.server_port,
        handshake.protocol,
        config.online_mode
    );

    if config.allow_offline_login && !config.online_mode {
        write_packet(
            &mut writer,
            &build_packet(config.packets.login_success_clientbound, |body| {
                body.extend_from_slice(identity.uuid.as_bytes());
                write_string(body, &identity.username)?;
                write_varint_vec(body, 0);
                Ok(())
            })?,
        )?;
    } else {
        write_packet(
            &mut writer,
            &build_packet(config.packets.login_disconnect_clientbound, |body| {
                write_string(body, &config.login_disconnect_json())
            })?,
        )?;
    }
    writer.flush()?;
    Ok(())
}

fn parse_handshake_packet(packet: &[u8], packets: &PacketIds) -> Result<Handshake> {
    let mut reader = PacketReader::new(packet);
    let packet_id = reader.read_varint()?;
    if packet_id != packets.handshake_serverbound {
        bail!(
            "expected handshake packet id {}, got {packet_id}",
            packets.handshake_serverbound
        );
    }
    Ok(Handshake {
        protocol: reader.read_varint()?,
        server_address: reader.read_string()?,
        server_port: reader.read_u16()?,
        next_state: reader.read_varint()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_expected_config_argument() {
        let cli = Cli::try_parse_from(["ferrum-server", "--config", "server.toml"])
            .expect("expected CLI should parse");

        assert_eq!(cli.config, PathBuf::from("server.toml"));
    }

    #[test]
    fn requires_config_argument() {
        assert!(Cli::try_parse_from(["ferrum-server"]).is_err());
    }

    #[test]
    fn parses_server_config() {
        let config = ServerConfig::from_toml_like_with_base(
            r#"
            [server]
            bind = "127.0.0.1:25566"
            version_name = "Minecraft Java Edition 26.0.0"
            protocol = 2600
            motd = "Ferrum test"
            max_players = 5
            online_players = 1
            login_disconnect_message = "Not ready"
            allow_offline_login = false
            online_mode = false
            hide_online_players = false
            enforces_secure_chat = true
            previews_chat = false
            server_icon = "data:image/png;base64,iVBORw0KGgo="
            sample_players = "Steve:00000000-0000-0000-0000-000000000000;Alex:11111111-1111-1111-1111-111111111111"

            [protocol]
            handshake_serverbound = 0
            status_request_serverbound = 0
            status_response_clientbound = 0
            ping_request_serverbound = 1
            pong_response_clientbound = 1
            login_start_serverbound = 0
            login_disconnect_clientbound = 0
            login_success_clientbound = 2
            "#,
            None,
        )
        .expect("config should parse");

        assert_eq!(config.bind, "127.0.0.1:25566");
        assert_eq!(config.protocol, 2600);
        assert_eq!(config.motd, "Ferrum test");
        assert_eq!(config.max_players, 5);
        assert_eq!(config.online_players, 1);
        assert_eq!(config.login_disconnect_message, "Not ready");
        assert!(!config.allow_offline_login);
        assert!(!config.online_mode);
        assert!(config.enforces_secure_chat);
        assert!(!config.previews_chat);
        assert_eq!(
            config.server_icon.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        assert_eq!(config.sample_players.len(), 2);
        assert_eq!(config.sample_players[0].name, "Steve");
        assert_eq!(config.packets.ping_request_serverbound, 1);
        assert_eq!(config.packets.login_success_clientbound, 2);
    }

    #[test]
    fn status_json_includes_vanilla_server_list_metadata() {
        let config = ServerConfig {
            protocol: 2600,
            version_name: "Minecraft Java Edition 26.0.0".to_owned(),
            motd: "Ferrum status".to_owned(),
            online_players: 1,
            max_players: 10,
            enforces_secure_chat: true,
            previews_chat: false,
            server_icon: Some("data:image/png;base64,iVBORw0KGgo=".to_owned()),
            sample_players: vec![SamplePlayer {
                name: "Steve".to_owned(),
                id: "00000000-0000-0000-0000-000000000000".to_owned(),
            }],
            ..ServerConfig::default()
        };

        let status: Value =
            serde_json::from_str(&config.status_json()).expect("status should be valid JSON");
        assert_eq!(status["version"]["protocol"], 2600);
        assert_eq!(status["description"]["text"], "Ferrum status");
        assert_eq!(status["players"]["online"], 1);
        assert_eq!(status["players"]["sample"][0]["name"], "Steve");
        assert_eq!(status["enforcesSecureChat"], true);
        assert_eq!(status["previewsChat"], false);
        assert_eq!(status["favicon"], "data:image/png;base64,iVBORw0KGgo=");
    }

    #[test]
    fn can_hide_online_players_in_status_json() {
        let config = ServerConfig {
            hide_online_players: true,
            ..ServerConfig::default()
        };
        let status: Value =
            serde_json::from_str(&config.status_json()).expect("status should be valid JSON");
        assert!(status.get("players").is_none());
    }

    #[test]
    fn base64_encoder_matches_png_header_fixture() {
        assert_eq!(
            base64_encode(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            "iVBORw0KGgo="
        );
    }

    #[test]
    fn varint_round_trips_protocol_values() {
        for value in [0, 1, 127, 128, 255, 2_600, i32::MAX, -1] {
            let mut encoded = Vec::new();
            write_varint_vec(&mut encoded, value);
            let mut cursor = Cursor::new(encoded);
            assert_eq!(read_varint_io(&mut cursor).unwrap(), value);
        }
    }

    #[test]
    fn parses_handshake_packet() {
        let packet = build_packet(0, |body| {
            write_varint_vec(body, 2600);
            write_string(body, "localhost")?;
            body.extend_from_slice(&25565u16.to_be_bytes());
            write_varint_vec(body, 1);
            Ok(())
        })
        .unwrap();

        assert_eq!(
            parse_handshake_packet(&packet, &PacketIds::default()).unwrap(),
            Handshake {
                protocol: 2600,
                server_address: "localhost".to_owned(),
                server_port: 25565,
                next_state: 1,
            }
        );
    }

    #[test]
    fn answers_status_request_and_ping() {
        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(&mut input, &build_packet(0, |_| Ok(())).unwrap()).unwrap();
        write_packet(
            &mut input,
            &build_packet(1, |body| {
                body.extend_from_slice(&12345i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();

        let config = ServerConfig {
            protocol: 2600,
            version_name: "Minecraft Java Edition 26.0.0".to_owned(),
            motd: "Ferrum status".to_owned(),
            ..ServerConfig::default()
        };
        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();

        let mut cursor = Cursor::new(output);
        let response = read_packet(&mut cursor).unwrap();
        let mut response_reader = PacketReader::new(&response);
        assert_eq!(response_reader.read_varint().unwrap(), 0);
        let status = response_reader.read_string().unwrap();
        assert!(status.contains("Minecraft Java Edition 26.0.0"));
        assert!(status.contains("Ferrum status"));

        let pong = read_packet(&mut cursor).unwrap();
        let mut pong_reader = PacketReader::new(&pong);
        assert_eq!(pong_reader.read_varint().unwrap(), 1);
        assert_eq!(pong_reader.read_i64().unwrap(), 12345);
    }

    #[test]
    fn disconnects_login_attempts_with_configured_message() {
        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0, |body| write_string(body, "sakus")).expect("login start should build"),
        )
        .unwrap();

        let config = ServerConfig {
            login_disconnect_message: "Play login is not implemented yet".to_owned(),
            ..ServerConfig::default()
        };
        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();

        let mut cursor = Cursor::new(output);
        let disconnect = read_packet(&mut cursor).unwrap();
        let mut disconnect_reader = PacketReader::new(&disconnect);
        assert_eq!(disconnect_reader.read_varint().unwrap(), 0);
        let reason = disconnect_reader.read_string().unwrap();
        assert!(reason.contains("Play login is not implemented yet"));
    }

    #[test]
    fn can_accept_offline_login_with_login_success() {
        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0, |body| write_string(body, "Steve")).expect("login start should build"),
        )
        .unwrap();

        let config = ServerConfig {
            allow_offline_login: true,
            ..ServerConfig::default()
        };
        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();

        let mut cursor = Cursor::new(output);
        let login_success = read_packet(&mut cursor).unwrap();
        let mut success_reader = PacketReader::new(&login_success);
        assert_eq!(success_reader.read_varint().unwrap(), 2);
        assert_eq!(
            success_reader.read_uuid_bytes().unwrap(),
            [
                0x56, 0x27, 0xdd, 0x98, 0xe6, 0xbe, 0x3c, 0x21, 0xb8, 0xa8, 0xe9, 0x23, 0x44, 0x18,
                0x36, 0x41
            ]
        );
        assert_eq!(success_reader.read_string().unwrap(), "Steve");
        assert_eq!(success_reader.read_varint().unwrap(), 0);
    }

    #[test]
    fn supports_configured_packet_ids() {
        let packets = PacketIds {
            status_request_serverbound: 9,
            status_response_clientbound: 10,
            ping_request_serverbound: 11,
            pong_response_clientbound: 12,
            login_start_serverbound: 13,
            login_disconnect_clientbound: 14,
            login_success_clientbound: 15,
            ..PacketIds::default()
        };
        let config = ServerConfig {
            packets,
            login_disconnect_message: "custom packet table".to_owned(),
            ..ServerConfig::default()
        };

        let mut status_input = Vec::new();
        write_packet(
            &mut status_input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut status_input,
            &build_packet(9, |_| Ok(())).expect("custom status request should build"),
        )
        .unwrap();
        write_packet(
            &mut status_input,
            &build_packet(11, |body| {
                body.extend_from_slice(&99i64.to_be_bytes());
                Ok(())
            })
            .expect("custom ping should build"),
        )
        .unwrap();
        let mut status_output = Vec::new();
        handle_connection_protocol(Cursor::new(status_input), &mut status_output, &config).unwrap();
        let mut status_cursor = Cursor::new(status_output);
        let status_response = read_packet(&mut status_cursor).unwrap();
        assert_eq!(
            PacketReader::new(&status_response).read_varint().unwrap(),
            10
        );
        let pong_response = read_packet(&mut status_cursor).unwrap();
        assert_eq!(PacketReader::new(&pong_response).read_varint().unwrap(), 12);

        let mut login_input = Vec::new();
        write_packet(
            &mut login_input,
            &build_packet(0, |body| {
                write_varint_vec(body, 2600);
                write_string(body, "localhost")?;
                body.extend_from_slice(&25565u16.to_be_bytes());
                write_varint_vec(body, 2);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut login_input,
            &build_packet(13, |body| write_string(body, "sakus"))
                .expect("custom login start should build"),
        )
        .unwrap();
        let mut login_output = Vec::new();
        handle_connection_protocol(Cursor::new(login_input), &mut login_output, &config).unwrap();
        let mut login_cursor = Cursor::new(login_output);
        let disconnect = read_packet(&mut login_cursor).unwrap();
        assert_eq!(PacketReader::new(&disconnect).read_varint().unwrap(), 14);
    }
}
