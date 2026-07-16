from pathlib import Path
import re


def load(path: str) -> str:
    return Path(path).read_text()


def save(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, repl: str, label: str) -> str:
    new, count = re.subn(pattern, repl, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{label}: expected one regex match, found {count}")
    return new

# Expose exact bounded primitive reads for strict packet decoding.
path = "crates/ferrum-server/src/codec.rs"
text = load(path)
text = replace_once(text, "    fn read_u8(&mut self) -> Result<u8> {", "    pub(crate) fn read_u8(&mut self) -> Result<u8> {", "codec read_u8 visibility")
text = replace_once(text, "    fn read_bytes(&mut self, length: usize) -> Result<&'a [u8]> {", "    pub(crate) fn read_bytes(&mut self, length: usize) -> Result<&'a [u8]> {", "codec read_bytes visibility")
save(path, text)

# Preserve the actual previous game mode across respawn.
path = "crates/ferrum-game/src/state.rs"
text = load(path)
text = replace_once(text, "        previous_game_mode: GameMode,", "        previous_game_mode: Option<GameMode>,", "respawn event previous game mode")
text = replace_once(
    text,
    "            let previous_game_mode = player.game_mode;\n            player.vitals = Vitals::default();\n            (player.game_mode, previous_game_mode, player.vitals)",
    "            let previous_game_mode = player.previous_game_mode;\n            player.vitals = Vitals::default();\n            (player.game_mode, previous_game_mode, player.vitals)",
    "respawn state previous game mode",
)
text = replace_once(
    text,
    "        state.connect_player(uuid, \"Steve\", spawn()).unwrap();\n        let entity_id = state.player(uuid).unwrap().entity_id.unwrap();",
    "        state.connect_player(uuid, \"Steve\", spawn()).unwrap();\n        state.set_game_mode(uuid, GameMode::Creative).unwrap();\n        let entity_id = state.player(uuid).unwrap().entity_id.unwrap();",
    "respawn test game mode setup",
)
text = replace_once(
    text,
    "        assert_eq!(state.player(uuid).unwrap().vitals, Vitals::default());\n        assert!(matches!(",
    "        assert_eq!(state.player(uuid).unwrap().vitals, Vitals::default());\n        assert!(matches!(\n            events[0],\n            GameEvent::PlayerRespawned {\n                previous_game_mode: Some(GameMode::Survival),\n                ..\n            }\n        ));\n        assert!(matches!(",
    "respawn test previous game mode assertion",
)
save(path, text)

# Decode the complete official 26.1.2 chat packet instead of accepting arbitrary trailing bytes.
path = "crates/ferrum-server/src/play_runtime.rs"
text = load(path)
text = replace_once(
    text,
    "const MAX_CHAT_MESSAGE_BYTES: usize = 256;",
    "const MAX_CHAT_MESSAGE_CHARS: usize = 256;\nconst MAX_CHAT_MESSAGE_ENCODED_BYTES: usize = MAX_CHAT_MESSAGE_CHARS * 3;\nconst MESSAGE_SIGNATURE_BYTES: usize = 256;\nconst LAST_SEEN_ACKNOWLEDGED_BYTES: usize = 3;",
    "chat constants",
)
text = sub_once(
    text,
    r"fn decode_chat_message\(reader: &mut PacketReader<'_>\) -> Result<String> \{.*?\n\}",
    '''fn decode_chat_message(reader: &mut PacketReader<'_>) -> Result<String> {
    let message = reader.read_string()?;
    if message.is_empty()
        || message.encode_utf16().count() > MAX_CHAT_MESSAGE_CHARS
        || message.len() > MAX_CHAT_MESSAGE_ENCODED_BYTES
        || message.chars().any(char::is_control)
    {
        bail!(
            "chat message must contain 1..={MAX_CHAT_MESSAGE_CHARS} UTF-16 code units, fit the protocol UTF-8 bound, and contain no control characters"
        );
    }

    // ServerboundChatPacket 26.1.2: message, Instant(epoch millis), salt,
    // nullable 256-byte signature, then LastSeenMessages.Update(offset,
    // fixed 20-bit acknowledgement set, checksum).
    let _timestamp_millis = reader.read_i64()?;
    let _salt = reader.read_i64()?;
    let signature_present = reader.read_u8()? != 0;
    if signature_present {
        reader.read_bytes(MESSAGE_SIGNATURE_BYTES)?;
    }
    let last_seen_offset = reader.read_varint()?;
    if last_seen_offset < 0 {
        bail!("last-seen message offset cannot be negative");
    }
    reader.read_bytes(LAST_SEEN_ACKNOWLEDGED_BYTES)?;
    let _checksum = reader.read_u8()?;
    if !reader.take_remaining().is_empty() {
        bail!("chat message packet contains trailing bytes");
    }
    Ok(message)
}''',
    "strict chat decoder",
)
text = replace_once(
    text,
    '''        let mut message = Vec::new();
        write_string(&mut message, "hello world").unwrap();
        message.extend_from_slice(&123_i64.to_be_bytes());
        assert_eq!(
            decode_chat_message(&mut PacketReader::new(&message)).unwrap(),
            "hello world"
        );
        assert!(decode_chat_message(&mut PacketReader::new(&[0])).is_err());
        let mut control = Vec::new();
        write_string(&mut control, "bad\\nmessage").unwrap();
        assert!(decode_chat_message(&mut PacketReader::new(&control)).is_err());''',
    '''        let mut message = Vec::new();
        write_string(&mut message, "hello world").unwrap();
        message.extend_from_slice(&123_i64.to_be_bytes());
        message.extend_from_slice(&456_i64.to_be_bytes());
        message.push(0); // no signature
        message.push(0); // last-seen offset VarInt
        message.extend_from_slice(&[0; LAST_SEEN_ACKNOWLEDGED_BYTES]);
        message.push(0); // ignored checksum in offline mode
        assert_eq!(
            decode_chat_message(&mut PacketReader::new(&message)).unwrap(),
            "hello world"
        );
        let mut trailing = message.clone();
        trailing.push(0);
        assert!(decode_chat_message(&mut PacketReader::new(&trailing)).is_err());
        assert!(decode_chat_message(&mut PacketReader::new(&message[..message.len() - 1])).is_err());
        assert!(decode_chat_message(&mut PacketReader::new(&[0])).is_err());
        let mut control = Vec::new();
        write_string(&mut control, "bad\\nmessage").unwrap();
        assert!(decode_chat_message(&mut PacketReader::new(&control)).is_err());''',
    "chat decoder test",
)
save(path, text)
