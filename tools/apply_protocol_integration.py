from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")


def replace_once(old: str, new: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected exactly one match, found {count}: {old[:80]!r}")
    text = text.replace(old, new, 1)


replace_once(
    "use identity::offline_player_identity;\nuse serde_json::{Map, Value, json};",
    "use ferrum_protocol::{\n"
    "    HandshakeIntent, PacketKind, PacketTable, ProtocolProfile, ProtocolSession,\n"
    "};\n"
    "use identity::offline_player_identity;\n"
    "use serde_json::{Map, Value, json};",
)

replace_once(
    "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n"
    "enum ConnectionState {\n"
    "    Status,\n"
    "    Login,\n"
    "}\n\n",
    "",
)

replace_once(
    "    let config = ServerConfig::from_file(&config_path)\n"
    "        .with_context(|| format!(\"cannot load {}\", config_path.display()))?;\n"
    "    let listener = TcpListener::bind(&config.bind)",
    "    let config = ServerConfig::from_file(&config_path)\n"
    "        .with_context(|| format!(\"cannot load {}\", config_path.display()))?;\n"
    "    config\n"
    "        .protocol_profile()\n"
    "        .context(\"cannot build configured protocol profile\")?;\n"
    "    let listener = TcpListener::bind(&config.bind)",
)

replace_once(
    "    fn login_disconnect_json(&self) -> String {\n"
    "        json!({\n"
    "            \"text\": self.login_disconnect_message,\n"
    "        })\n"
    "        .to_string()\n"
    "    }\n",
    "    fn login_disconnect_json(&self) -> String {\n"
    "        json!({\n"
    "            \"text\": self.login_disconnect_message,\n"
    "        })\n"
    "        .to_string()\n"
    "    }\n\n"
    "    fn protocol_profile(&self) -> Result<ProtocolProfile> {\n"
    "        let mut packets = PacketTable::new();\n"
    "        packets.insert(PacketKind::Handshake, self.packets.handshake_serverbound)?;\n"
    "        packets.insert(\n"
    "            PacketKind::StatusRequest,\n"
    "            self.packets.status_request_serverbound,\n"
    "        )?;\n"
    "        packets.insert(\n"
    "            PacketKind::StatusResponse,\n"
    "            self.packets.status_response_clientbound,\n"
    "        )?;\n"
    "        packets.insert(PacketKind::PingRequest, self.packets.ping_request_serverbound)?;\n"
    "        packets.insert(\n"
    "            PacketKind::PongResponse,\n"
    "            self.packets.pong_response_clientbound,\n"
    "        )?;\n"
    "        packets.insert(PacketKind::LoginStart, self.packets.login_start_serverbound)?;\n"
    "        packets.insert(\n"
    "            PacketKind::LoginDisconnect,\n"
    "            self.packets.login_disconnect_clientbound,\n"
    "        )?;\n"
    "        packets.insert(\n"
    "            PacketKind::LoginSuccess,\n"
    "            self.packets.login_success_clientbound,\n"
    "        )?;\n"
    "        ProtocolProfile::new(self.version_name.clone(), self.protocol, packets)\n"
    "            .context(\"invalid protocol profile\")\n"
    "    }\n",
)

old_handlers = '''fn handle_connection_protocol<R: Read, W: Write>(
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
'''

new_handlers = '''fn handle_connection_protocol<R: Read, W: Write>(
    mut reader: R,
    writer: W,
    config: &ServerConfig,
) -> Result<()> {
    let profile = config.protocol_profile()?;
    let handshake_packet = read_packet(&mut reader).context("cannot read handshake packet")?;
    let handshake = parse_handshake_packet(&handshake_packet, profile.packets())?;
    let intent = handshake.intent()?;
    let mut session = ProtocolSession::new();
    session.handshake(handshake.protocol, intent)?;
    match intent {
        HandshakeIntent::Status => {
            handle_status_protocol(reader, writer, config, &profile, &mut session)
        }
        HandshakeIntent::Login => {
            handle_login_protocol(reader, writer, config, &handshake, &profile, &mut session)
        }
    }
}

impl Handshake {
    fn intent(&self) -> Result<HandshakeIntent> {
        match self.next_state {
            1 => Ok(HandshakeIntent::Status),
            2 => Ok(HandshakeIntent::Login),
            other => bail!("unsupported handshake next_state {other}"),
        }
    }
}

fn handle_status_protocol<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    config: &ServerConfig,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
) -> Result<()> {
    let expected_request_id = profile.packets().require(PacketKind::StatusRequest)?;
    let request_packet = read_packet(&mut reader).context("cannot read status request")?;
    let mut request_reader = PacketReader::new(&request_packet);
    let request_id = request_reader.read_varint()?;
    if request_id != expected_request_id {
        bail!("expected status request packet id {expected_request_id}, got {request_id}");
    }
    session.status_request()?;
    write_packet(
        &mut writer,
        &build_packet(
            profile.packets().require(PacketKind::StatusResponse)?,
            |body| write_string(body, &config.status_json()),
        )?,
    )?;
    session.status_response_sent()?;

    match read_packet(&mut reader) {
        Ok(ping_packet) => {
            let expected_ping_id = profile.packets().require(PacketKind::PingRequest)?;
            let mut ping_reader = PacketReader::new(&ping_packet);
            let ping_id = ping_reader.read_varint()?;
            if ping_id != expected_ping_id {
                bail!("expected ping packet id {expected_ping_id}, got {ping_id}");
            }
            session.ping()?;
            let payload = ping_reader.read_i64()?;
            write_packet(
                &mut writer,
                &build_packet(
                    profile.packets().require(PacketKind::PongResponse)?,
                    |body| {
                        body.extend_from_slice(&payload.to_be_bytes());
                        Ok(())
                    },
                )?,
            )?;
            session.pong_sent()?;
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
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
) -> Result<()> {
    let expected_login_id = profile.packets().require(PacketKind::LoginStart)?;
    let login_packet = read_packet(&mut reader).context("cannot read login start packet")?;
    let mut login_reader = PacketReader::new(&login_packet);
    let packet_id = login_reader.read_varint()?;
    if packet_id != expected_login_id {
        bail!("expected login start packet id {expected_login_id}, got {packet_id}");
    }
    let username = login_reader.read_string()?;
    session.login_start(username.clone())?;
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
            &build_packet(
                profile.packets().require(PacketKind::LoginSuccess)?,
                |body| {
                    body.extend_from_slice(identity.uuid.as_bytes());
                    write_string(body, &identity.username)?;
                    write_varint_vec(body, 0);
                    Ok(())
                },
            )?,
        )?;
        session.login_success_sent()?;
    } else {
        write_packet(
            &mut writer,
            &build_packet(
                profile.packets().require(PacketKind::LoginDisconnect)?,
                |body| write_string(body, &config.login_disconnect_json()),
            )?,
        )?;
        session.disconnect();
    }
    writer.flush()?;
    Ok(())
}

fn parse_handshake_packet(packet: &[u8], packets: &PacketTable) -> Result<Handshake> {
    let mut reader = PacketReader::new(packet);
    let expected_packet_id = packets.require(PacketKind::Handshake)?;
    let packet_id = reader.read_varint()?;
    if packet_id != expected_packet_id {
        bail!("expected handshake packet id {expected_packet_id}, got {packet_id}");
    }
    Ok(Handshake {
        protocol: reader.read_varint()?,
        server_address: reader.read_string()?,
        server_port: reader.read_u16()?,
        next_state: reader.read_varint()?,
    })
}
'''

replace_once(old_handlers, new_handlers)

replace_once(
    "        assert_eq!(\n"
    "            parse_handshake_packet(&packet, &PacketIds::default()).unwrap(),",
    "        let profile = ServerConfig::default().protocol_profile().unwrap();\n\n"
    "        assert_eq!(\n"
    "            parse_handshake_packet(&packet, profile.packets()).unwrap(),",
)

path.write_text(text, encoding="utf-8")
