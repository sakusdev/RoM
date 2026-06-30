from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "crates/ferrum-configuration/src/lib.rs",
    "use thiserror::Error;\n\n",
    "use thiserror::Error;\n\n"
    "pub const MAX_CLIENT_LANGUAGE_BYTES: usize = 16;\n"
    "const CHAT_VISIBILITY_VARIANTS: i32 = 3;\n"
    "const MAIN_HAND_VARIANTS: i32 = 2;\n"
    "const PARTICLE_STATUS_VARIANTS: i32 = 3;\n\n"
    "#[derive(Debug, Clone, PartialEq, Eq)]\n"
    "pub struct ClientInformation {\n"
    "    pub language: String,\n"
    "    pub view_distance: i8,\n"
    "    pub chat_visibility: i32,\n"
    "    pub chat_colors: bool,\n"
    "    pub model_customization: u8,\n"
    "    pub main_hand: i32,\n"
    "    pub text_filtering_enabled: bool,\n"
    "    pub allows_listing: bool,\n"
    "    pub particle_status: i32,\n"
    "}\n\n",
)
replace_once(
    "crates/ferrum-configuration/src/lib.rs",
    "    #[error(\"Configuration string is not valid UTF-8: {0}\")]\n"
    "    InvalidUtf8(#[from] std::string::FromUtf8Error),\n",
    "    #[error(\"invalid boolean byte {0}\")]\n"
    "    InvalidBoolean(u8),\n"
    "    #[error(\"invalid {what} enum value {value}; expected 0..{variant_count}\")]\n"
    "    InvalidEnum {\n"
    "        what: &'static str,\n"
    "        value: i32,\n"
    "        variant_count: i32,\n"
    "    },\n"
    "    #[error(\"Configuration string is not valid UTF-8: {0}\")]\n"
    "    InvalidUtf8(#[from] std::string::FromUtf8Error),\n",
)
replace_once(
    "crates/ferrum-configuration/src/lib.rs",
    "pub fn encode_known_packs(packs: &[KnownPack]) -> Result<Vec<u8>, ConfigurationEncodeError> {\n",
    "pub fn decode_client_information(\n"
    "    payload: &[u8],\n"
    ") -> Result<ClientInformation, ConfigurationDecodeError> {\n"
    "    let mut decoder = Decoder::new(payload);\n"
    "    let information = ClientInformation {\n"
    "        language: decoder.read_string(MAX_CLIENT_LANGUAGE_BYTES)?,\n"
    "        view_distance: decoder.read_i8()?,\n"
    "        chat_visibility: decoder.read_enum(\"chat visibility\", CHAT_VISIBILITY_VARIANTS)?,\n"
    "        chat_colors: decoder.read_bool()?,\n"
    "        model_customization: decoder.read_u8()?,\n"
    "        main_hand: decoder.read_enum(\"main hand\", MAIN_HAND_VARIANTS)?,\n"
    "        text_filtering_enabled: decoder.read_bool()?,\n"
    "        allows_listing: decoder.read_bool()?,\n"
    "        particle_status: decoder.read_enum(\"particle status\", PARTICLE_STATUS_VARIANTS)?,\n"
    "    };\n"
    "    decoder.finish()?;\n"
    "    Ok(information)\n"
    "}\n\n"
    "pub fn encode_known_packs(packs: &[KnownPack]) -> Result<Vec<u8>, ConfigurationEncodeError> {\n",
)
replace_once(
    "crates/ferrum-configuration/src/lib.rs",
    "    fn read_u8(&mut self) -> Result<u8, ConfigurationDecodeError> {\n"
    "        Ok(*self.read_bytes(1)?.first().expect(\"one byte was just read\"))\n"
    "    }\n\n",
    "    fn read_i8(&mut self) -> Result<i8, ConfigurationDecodeError> {\n"
    "        Ok(self.read_u8()? as i8)\n"
    "    }\n\n"
    "    fn read_bool(&mut self) -> Result<bool, ConfigurationDecodeError> {\n"
    "        match self.read_u8()? {\n"
    "            0 => Ok(false),\n"
    "            1 => Ok(true),\n"
    "            value => Err(ConfigurationDecodeError::InvalidBoolean(value)),\n"
    "        }\n"
    "    }\n\n"
    "    fn read_enum(\n"
    "        &mut self,\n"
    "        what: &'static str,\n"
    "        variant_count: i32,\n"
    "    ) -> Result<i32, ConfigurationDecodeError> {\n"
    "        let value = self.read_varint()?;\n"
    "        if !(0..variant_count).contains(&value) {\n"
    "            return Err(ConfigurationDecodeError::InvalidEnum {\n"
    "                what,\n"
    "                value,\n"
    "                variant_count,\n"
    "            });\n"
    "        }\n"
    "        Ok(value)\n"
    "    }\n\n"
    "    fn read_u8(&mut self) -> Result<u8, ConfigurationDecodeError> {\n"
    "        Ok(*self.read_bytes(1)?.first().expect(\"one byte was just read\"))\n"
    "    }\n\n",
)
replace_once(
    "crates/ferrum-configuration/src/lib.rs",
    "    #[test]\n    fn known_packs_round_trip() {\n",
    "    #[test]\n"
    "    fn decodes_official_26_1_2_client_information_fixture() {\n"
    "        let payload = [\n"
    "            0x05, b'e', b'n', b'_', b'u', b's', 0x02, 0x00, 0x01, 0x00, 0x01, 0x00,\n"
    "            0x00, 0x00,\n"
    "        ];\n"
    "        assert_eq!(\n"
    "            decode_client_information(&payload).unwrap(),\n"
    "            ClientInformation {\n"
    "                language: \"en_us\".to_owned(),\n"
    "                view_distance: 2,\n"
    "                chat_visibility: 0,\n"
    "                chat_colors: true,\n"
    "                model_customization: 0,\n"
    "                main_hand: 1,\n"
    "                text_filtering_enabled: false,\n"
    "                allows_listing: false,\n"
    "                particle_status: 0,\n"
    "            }\n"
    "        );\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn client_information_decoder_rejects_invalid_values() {\n"
    "        let mut invalid = vec![\n"
    "            0x05, b'e', b'n', b'_', b'u', b's', 0x02, 0x00, 0x02, 0x00, 0x01, 0x00,\n"
    "            0x00, 0x00,\n"
    "        ];\n"
    "        assert!(matches!(\n"
    "            decode_client_information(&invalid),\n"
    "            Err(ConfigurationDecodeError::InvalidBoolean(2))\n"
    "        ));\n"
    "        invalid[8] = 0x01;\n"
    "        invalid[10] = 0x03;\n"
    "        assert!(matches!(\n"
    "            decode_client_information(&invalid),\n"
    "            Err(ConfigurationDecodeError::InvalidEnum {\n"
    "                what: \"main hand\",\n"
    "                value: 3,\n"
    "                variant_count: 2,\n"
    "            })\n"
    "        ));\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn known_packs_round_trip() {\n",
)

replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "    ConfigurationAcknowledged,\n    ConfigurationDisconnect,\n",
    "    ConfigurationAcknowledged,\n"
    "    ConfigurationClientInformation,\n"
    "    ConfigurationDisconnect,\n",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "            Self::ConfigurationAcknowledged\n            | Self::ConfigurationDisconnect\n",
    "            Self::ConfigurationAcknowledged\n"
    "            | Self::ConfigurationClientInformation\n"
    "            | Self::ConfigurationDisconnect\n",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "            | Self::ConfigurationAcknowledged\n            | Self::SelectKnownPacksResponse\n",
    "            | Self::ConfigurationAcknowledged\n"
    "            | Self::ConfigurationClientInformation\n"
    "            | Self::SelectKnownPacksResponse\n",
)

replace_once(
    "crates/ferrum-version-26-1-2/src/lib.rs",
    "        (PacketKind::LoginAcknowledged, 0x03),\n        (PacketKind::ConfigurationAcknowledged, 0x03),\n",
    "        (PacketKind::LoginAcknowledged, 0x03),\n"
    "        (PacketKind::ConfigurationClientInformation, 0x00),\n"
    "        (PacketKind::ConfigurationAcknowledged, 0x03),\n",
)
replace_once(
    "crates/ferrum-version-26-1-2/src/lib.rs",
    "        assert_eq!(\n            packets.require(PacketKind::FinishConfiguration).unwrap(),\n            0x03\n        );\n",
    "        assert_eq!(\n"
    "            packets\n"
    "                .require(PacketKind::ConfigurationClientInformation)\n"
    "                .unwrap(),\n"
    "            0x00\n"
    "        );\n"
    "        assert_eq!(\n"
    "            packets.require(PacketKind::FinishConfiguration).unwrap(),\n"
    "            0x03\n"
    "        );\n",
)

replace_once(
    "crates/ferrum-server/src/main.rs",
    "    KnownPack, KnownPackDecodeLimits, decode_known_packs, encode_feature_flags, encode_known_packs,\n"
    "    encode_registry_data, encode_tags,\n",
    "    KnownPack, KnownPackDecodeLimits, decode_client_information, decode_known_packs,\n"
    "    encode_feature_flags, encode_known_packs, encode_registry_data, encode_tags,\n",
)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "const MAX_IGNORED_PLAY_PACKETS: usize = 1_024;\n",
    "const MAX_CONFIGURATION_AUXILIARY_PACKETS: usize = 16;\n"
    "const MAX_IGNORED_PLAY_PACKETS: usize = 1_024;\n",
)
old_response = '''    let response = read_packet(reader).context("cannot read Select Known Packs response")?;
    let mut response_reader = PacketReader::new(&response);
    let expected_id = profile
        .packets()
        .require(PacketKind::SelectKnownPacksResponse)?;
    let received_id = response_reader.read_varint()?;
    if received_id != expected_id {
        bail!("expected Select Known Packs packet id {expected_id}, got {received_id}");
    }
    let accepted = decode_known_packs(
        response_reader.take_remaining(),
        KnownPackDecodeLimits::default(),
    )?;
'''
new_response = '''    let expected_id = profile
        .packets()
        .require(PacketKind::SelectKnownPacksResponse)?;
    let client_information_id = profile
        .packets()
        .id(PacketKind::ConfigurationClientInformation);
    let mut auxiliary_packets = 0_usize;
    let accepted = loop {
        let response = read_packet(reader).context("cannot read Select Known Packs response")?;
        let mut response_reader = PacketReader::new(&response);
        let received_id = response_reader.read_varint()?;

        if received_id == expected_id {
            break decode_known_packs(
                response_reader.take_remaining(),
                KnownPackDecodeLimits::default(),
            )?;
        }

        if client_information_id == Some(received_id) {
            decode_client_information(response_reader.take_remaining())?;
            auxiliary_packets = auxiliary_packets
                .checked_add(1)
                .context("Configuration auxiliary packet count overflow")?;
            if auxiliary_packets > MAX_CONFIGURATION_AUXILIARY_PACKETS {
                bail!("Configuration auxiliary packet limit exceeded");
            }
            continue;
        }

        bail!("expected Select Known Packs packet id {expected_id}, got {received_id}");
    };
'''
replace_once("crates/ferrum-server/src/main.rs", old_response, new_response)
replace_once(
    "crates/ferrum-server/src/main.rs",
    "        write_packet(&mut input, &build_packet(0x03, |_| Ok(())).unwrap()).unwrap();\n"
    "        let accepted = encode_known_packs(&version_26_1_2::known_packs()).unwrap();\n",
    "        write_packet(&mut input, &build_packet(0x03, |_| Ok(())).unwrap()).unwrap();\n"
    "        write_packet(\n"
    "            &mut input,\n"
    "            &build_packet(0x00, |body| {\n"
    "                body.extend_from_slice(&[\n"
    "                    0x05, b'e', b'n', b'_', b'u', b's', 0x02, 0x00, 0x01, 0x00, 0x01,\n"
    "                    0x00, 0x00, 0x00,\n"
    "                ]);\n"
    "                Ok(())\n"
    "            })\n"
    "            .unwrap(),\n"
    "        )\n"
    "        .unwrap();\n"
    "        let accepted = encode_known_packs(&version_26_1_2::known_packs()).unwrap();\n",
)
