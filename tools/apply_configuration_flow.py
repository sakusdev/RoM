from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:100]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


nbt = Path("crates/ferrum-nbt/src/lib.rs")
replace_once(
    nbt,
    '''/// Encode an unnamed root, as commonly required by Minecraft packet payloads.
pub fn encode_unnamed<W: Write>(writer: W, tag: &Tag) -> Result<(), NbtError> {
    encode_named(writer, &NamedTag::unnamed(tag.clone()))
}
''',
    '''/// Encode a standard named-root value using an empty root name.
pub fn encode_unnamed<W: Write>(writer: W, tag: &Tag) -> Result<(), NbtError> {
    encode_named(writer, &NamedTag::unnamed(tag.clone()))
}

/// Encode protocol anonymous NBT: a root tag type followed directly by its payload.
pub fn encode_anonymous<W: Write>(mut writer: W, tag: &Tag) -> Result<(), NbtError> {
    let tag_type = tag.tag_type();
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    write_u8(&mut writer, tag_type as u8)?;
    write_payload(&mut writer, tag)
}
''',
)
replace_once(
    nbt,
    '''pub fn decode_named_with_limits<R: Read>(
    mut reader: R,
    limits: DecodeLimits,
) -> Result<NamedTag, NbtError> {
    let tag_type = TagType::try_from(read_u8(&mut reader)?)?;
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    let name = read_string(&mut reader, limits)?;
    let tag = read_payload(&mut reader, tag_type, limits, 0)?;
    Ok(NamedTag { name, tag })
}
''',
    '''pub fn decode_named_with_limits<R: Read>(
    mut reader: R,
    limits: DecodeLimits,
) -> Result<NamedTag, NbtError> {
    let tag_type = TagType::try_from(read_u8(&mut reader)?)?;
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    let name = read_string(&mut reader, limits)?;
    let tag = read_payload(&mut reader, tag_type, limits, 0)?;
    Ok(NamedTag { name, tag })
}

/// Decode protocol anonymous NBT using conservative default limits.
pub fn decode_anonymous<R: Read>(reader: R) -> Result<Tag, NbtError> {
    decode_anonymous_with_limits(reader, DecodeLimits::default())
}

/// Decode protocol anonymous NBT using caller-provided resource limits.
pub fn decode_anonymous_with_limits<R: Read>(
    mut reader: R,
    limits: DecodeLimits,
) -> Result<Tag, NbtError> {
    let tag_type = TagType::try_from(read_u8(&mut reader)?)?;
    if tag_type == TagType::End {
        return Err(NbtError::InvalidNamedEnd);
    }
    read_payload(&mut reader, tag_type, limits, 0)
}
''',
)
replace_once(
    nbt,
    '''    #[test]
    fn rejects_truncated_input() {
''',
    '''    #[test]
    fn anonymous_nbt_omits_the_root_name() {
        let mut values = BTreeMap::new();
        values.insert("value".to_owned(), Tag::Int(7));
        let tag = Tag::Compound(values);
        let mut encoded = Vec::new();
        encode_anonymous(&mut encoded, &tag).unwrap();
        assert_eq!(
            encoded,
            vec![10, 3, 0, 5, b'v', b'a', b'l', b'u', b'e', 0, 0, 0, 7, 0]
        );
        assert_eq!(decode_anonymous(Cursor::new(encoded)).unwrap(), tag);
    }

    #[test]
    fn rejects_truncated_input() {
''',
)

protocol = Path("crates/ferrum-protocol/src/lib.rs")
replace_once(
    protocol,
    '''    ConfigurationAcknowledged,
    ConfigurationDisconnect,
    FinishConfiguration,
''',
    '''    ConfigurationAcknowledged,
    ConfigurationDisconnect,
    RegistryData,
    FeatureFlags,
    UpdateTags,
    FinishConfiguration,
''',
)
replace_once(
    protocol,
    '''            Self::ConfigurationAcknowledged
            | Self::ConfigurationDisconnect
            | Self::FinishConfiguration => ProtocolPhase::Configuration,
''',
    '''            Self::ConfigurationAcknowledged
            | Self::ConfigurationDisconnect
            | Self::RegistryData
            | Self::FeatureFlags
            | Self::UpdateTags
            | Self::FinishConfiguration => ProtocolPhase::Configuration,
''',
)

main = Path("crates/ferrum-server/src/main.rs")
replace_once(
    main,
    '''use ferrum_protocol::{HandshakeIntent, PacketKind, PacketTable, ProtocolProfile, ProtocolSession};
''',
    '''use ferrum_configuration::{encode_feature_flags, encode_tags};
use ferrum_protocol::{HandshakeIntent, PacketKind, PacketTable, ProtocolProfile, ProtocolSession};
''',
)
replace_once(
    main,
    '''    allow_offline_login: bool,
    online_mode: bool,
''',
    '''    allow_offline_login: bool,
    configuration_enabled: bool,
    configuration_features: Vec<String>,
    online_mode: bool,
''',
)
replace_once(
    main,
    '''    login_disconnect_clientbound: i32,
    login_success_clientbound: i32,
''',
    '''    login_disconnect_clientbound: i32,
    login_success_clientbound: i32,
    login_acknowledged_serverbound: Option<i32>,
    configuration_acknowledged_serverbound: Option<i32>,
    configuration_finish_clientbound: Option<i32>,
    configuration_feature_flags_clientbound: Option<i32>,
    configuration_tags_clientbound: Option<i32>,
    configuration_registry_data_clientbound: Option<i32>,
''',
)
replace_once(
    main,
    '''            allow_offline_login: false,
            online_mode: false,
''',
    '''            allow_offline_login: false,
            configuration_enabled: false,
            configuration_features: vec!["minecraft:vanilla".to_owned()],
            online_mode: false,
''',
)
replace_once(
    main,
    '''            login_disconnect_clientbound: 0,
            login_success_clientbound: 2,
''',
    '''            login_disconnect_clientbound: 0,
            login_success_clientbound: 2,
            login_acknowledged_serverbound: None,
            configuration_acknowledged_serverbound: None,
            configuration_finish_clientbound: None,
            configuration_feature_flags_clientbound: None,
            configuration_tags_clientbound: None,
            configuration_registry_data_clientbound: None,
''',
)
replace_once(
    main,
    '''                ("server", "online_mode") => {
                    config.online_mode = parse_bool(value, line_index + 1)?
                }
''',
    '''                ("configuration", "enabled") => {
                    config.configuration_enabled = parse_bool(value, line_index + 1)?
                }
                ("configuration", "features") => {
                    config.configuration_features = parse_string_list(&parse_string(value))
                }
                ("server", "online_mode") => {
                    config.online_mode = parse_bool(value, line_index + 1)?
                }
''',
)
replace_once(
    main,
    '''                ("protocol", "login_success_clientbound") => {
                    config.packets.login_success_clientbound = parse_i32(value, line_index + 1)?
                }
                _ => bail!("unknown config key [{section}].{key}"),
''',
    '''                ("protocol", "login_success_clientbound") => {
                    config.packets.login_success_clientbound = parse_i32(value, line_index + 1)?
                }
                ("protocol", "login_acknowledged_serverbound") => {
                    config.packets.login_acknowledged_serverbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_acknowledged_serverbound") => {
                    config.packets.configuration_acknowledged_serverbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_finish_clientbound") => {
                    config.packets.configuration_finish_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_feature_flags_clientbound") => {
                    config.packets.configuration_feature_flags_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_tags_clientbound") => {
                    config.packets.configuration_tags_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                ("protocol", "configuration_registry_data_clientbound") => {
                    config.packets.configuration_registry_data_clientbound =
                        Some(parse_i32(value, line_index + 1)?)
                }
                _ => bail!("unknown config key [{section}].{key}"),
''',
)
replace_once(
    main,
    '''        if config.online_players > config.max_players {
            bail!("online_players cannot exceed max_players");
        }
        Ok(config)
''',
    '''        if config.online_players > config.max_players {
            bail!("online_players cannot exceed max_players");
        }
        if config.configuration_enabled {
            for (name, packet_id) in [
                (
                    "login_acknowledged_serverbound",
                    config.packets.login_acknowledged_serverbound,
                ),
                (
                    "configuration_acknowledged_serverbound",
                    config.packets.configuration_acknowledged_serverbound,
                ),
                (
                    "configuration_finish_clientbound",
                    config.packets.configuration_finish_clientbound,
                ),
            ] {
                if packet_id.is_none() {
                    bail!("configuration is enabled but [protocol].{name} is missing");
                }
            }
        }
        Ok(config)
''',
)
replace_once(
    main,
    '''fn parse_i32(value: &str, line: usize) -> Result<i32> {
''',
    '''fn parse_string_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_i32(value: &str, line: usize) -> Result<i32> {
''',
)
replace_once(
    main,
    '''        packets.insert(
            PacketKind::LoginSuccess,
            self.packets.login_success_clientbound,
        )?;
        ProtocolProfile::new(self.version_name.clone(), self.protocol, packets)
''',
    '''        packets.insert(
            PacketKind::LoginSuccess,
            self.packets.login_success_clientbound,
        )?;
        for (kind, id) in [
            (
                PacketKind::LoginAcknowledged,
                self.packets.login_acknowledged_serverbound,
            ),
            (
                PacketKind::ConfigurationAcknowledged,
                self.packets.configuration_acknowledged_serverbound,
            ),
            (
                PacketKind::FinishConfiguration,
                self.packets.configuration_finish_clientbound,
            ),
            (
                PacketKind::FeatureFlags,
                self.packets.configuration_feature_flags_clientbound,
            ),
            (
                PacketKind::UpdateTags,
                self.packets.configuration_tags_clientbound,
            ),
            (
                PacketKind::RegistryData,
                self.packets.configuration_registry_data_clientbound,
            ),
        ] {
            if let Some(id) = id {
                packets.insert(kind, id)?;
            }
        }
        ProtocolProfile::new(self.version_name.clone(), self.protocol, packets)
''',
)
replace_once(
    main,
    '''        session.login_success_sent()?;
    } else {
''',
    '''        session.login_success_sent()?;
        if config.configuration_enabled {
            handle_configuration_protocol(&mut reader, &mut writer, config, profile, session)?;
        }
    } else {
''',
)
replace_once(
    main,
    '''fn parse_handshake_packet(packet: &[u8], packets: &PacketTable) -> Result<Handshake> {
''',
    '''fn handle_configuration_protocol<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    config: &ServerConfig,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
) -> Result<()> {
    let login_acknowledged = read_packet(reader).context("cannot read login acknowledged packet")?;
    let mut login_acknowledged_reader = PacketReader::new(&login_acknowledged);
    let expected_login_acknowledged = profile
        .packets()
        .require(PacketKind::LoginAcknowledged)?;
    let packet_id = login_acknowledged_reader.read_varint()?;
    if packet_id != expected_login_acknowledged {
        bail!(
            "expected login acknowledged packet id {expected_login_acknowledged}, got {packet_id}"
        );
    }
    session.login_acknowledged()?;

    if let Some(packet_id) = profile.packets().id(PacketKind::FeatureFlags) {
        let body = encode_feature_flags(&config.configuration_features)?;
        write_packet(writer, &build_packet(packet_id, |output| {
            output.extend_from_slice(&body);
            Ok(())
        })?)?;
    }
    if let Some(packet_id) = profile.packets().id(PacketKind::UpdateTags) {
        let body = encode_tags(&[])?;
        write_packet(writer, &build_packet(packet_id, |output| {
            output.extend_from_slice(&body);
            Ok(())
        })?)?;
    }

    write_packet(
        writer,
        &build_packet(
            profile.packets().require(PacketKind::FinishConfiguration)?,
            |_| Ok(()),
        )?,
    )?;
    session.finish_configuration_sent()?;
    writer.flush()?;

    let acknowledged = read_packet(reader).context("cannot read configuration acknowledged packet")?;
    let mut acknowledged_reader = PacketReader::new(&acknowledged);
    let expected_acknowledged = profile
        .packets()
        .require(PacketKind::ConfigurationAcknowledged)?;
    let packet_id = acknowledged_reader.read_varint()?;
    if packet_id != expected_acknowledged {
        bail!(
            "expected configuration acknowledged packet id {expected_acknowledged}, got {packet_id}"
        );
    }
    session.configuration_acknowledged()?;
    println!("configuration completed; connection entered Play state");
    Ok(())
}

fn parse_handshake_packet(packet: &[u8], packets: &PacketTable) -> Result<Handshake> {
''',
)
replace_once(
    main,
    '''            allow_offline_login = false
            online_mode = false
''',
    '''            allow_offline_login = false
            online_mode = false

            [configuration]
            enabled = true
            features = "minecraft:vanilla;minecraft:trade_rebalance"
''',
)
replace_once(
    main,
    '''            login_success_clientbound = 2
            "#,
''',
    '''            login_success_clientbound = 2
            login_acknowledged_serverbound = 3
            configuration_acknowledged_serverbound = 4
            configuration_finish_clientbound = 5
            configuration_feature_flags_clientbound = 6
            configuration_tags_clientbound = 7
            configuration_registry_data_clientbound = 8
            "#,
''',
)
replace_once(
    main,
    '''        assert!(!config.allow_offline_login);
        assert!(!config.online_mode);
''',
    '''        assert!(!config.allow_offline_login);
        assert!(config.configuration_enabled);
        assert_eq!(
            config.configuration_features,
            ["minecraft:vanilla", "minecraft:trade_rebalance"]
        );
        assert!(!config.online_mode);
''',
)
replace_once(
    main,
    '''    #[test]
    fn supports_configured_packet_ids() {
''',
    '''    #[test]
    fn completes_the_configured_configuration_sequence() {
        let packets = PacketIds {
            login_acknowledged_serverbound: Some(3),
            configuration_acknowledged_serverbound: Some(4),
            configuration_finish_clientbound: Some(5),
            configuration_feature_flags_clientbound: Some(6),
            configuration_tags_clientbound: Some(7),
            ..PacketIds::default()
        };
        let config = ServerConfig {
            allow_offline_login: true,
            configuration_enabled: true,
            configuration_features: vec!["minecraft:vanilla".to_owned()],
            packets,
            ..ServerConfig::default()
        };

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
            &build_packet(0, |body| write_string(body, "Steve")).unwrap(),
        )
        .unwrap();
        write_packet(&mut input, &build_packet(3, |_| Ok(())).unwrap()).unwrap();
        write_packet(&mut input, &build_packet(4, |_| Ok(())).unwrap()).unwrap();

        let mut output = Vec::new();
        handle_connection_protocol(Cursor::new(input), &mut output, &config).unwrap();
        let mut cursor = Cursor::new(output);

        let login_success = read_packet(&mut cursor).unwrap();
        assert_eq!(PacketReader::new(&login_success).read_varint().unwrap(), 2);

        let feature_flags = read_packet(&mut cursor).unwrap();
        let mut feature_reader = PacketReader::new(&feature_flags);
        assert_eq!(feature_reader.read_varint().unwrap(), 6);
        assert_eq!(feature_reader.read_varint().unwrap(), 1);
        assert_eq!(feature_reader.read_string().unwrap(), "minecraft:vanilla");

        let tags = read_packet(&mut cursor).unwrap();
        let mut tags_reader = PacketReader::new(&tags);
        assert_eq!(tags_reader.read_varint().unwrap(), 7);
        assert_eq!(tags_reader.read_varint().unwrap(), 0);

        let finish = read_packet(&mut cursor).unwrap();
        assert_eq!(PacketReader::new(&finish).read_varint().unwrap(), 5);
    }

    #[test]
    fn supports_configured_packet_ids() {
''',
)
