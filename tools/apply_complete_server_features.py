from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


play_path = Path("crates/ferrum-server/src/play_runtime.rs")
play = play_path.read_text()
play = replace_once(
    play,
    "use ferrum_game::{CommandSource, PlayerUuid as GamePlayerUuid, Transform};",
    "use ferrum_game::{CommandSource, GameEvent, PlayerUuid as GamePlayerUuid, Transform};",
    "game event import",
)
play = replace_once(
    play,
    "const MAX_CHAT_COMMAND_BYTES: usize = 32_767;",
    "const MAX_CHAT_COMMAND_BYTES: usize = 32_767;\nconst MAX_CHAT_MESSAGE_BYTES: usize = 256;",
    "chat message limit",
)
play = replace_once(
    play,
    """    fn select_hotbar(self, selected_hotbar: u8) -> Result<()> {
        self.runtime
            .select_hotbar(self.player_uuid, selected_hotbar)?;
        Ok(())
    }
""",
    """    fn broadcast_chat(self, message: &str) -> Result<()> {
        let name = self
            .runtime
            .with_state(|state| {
                state
                    .player(self.player_uuid)
                    .map(|player| player.name.clone())
            })?
            .context("authoritative player is missing while broadcasting chat")?;
        self.runtime.publish(&[GameEvent::Broadcast {
            message: format!("<{name}> {message}"),
        }])?;
        Ok(())
    }

    fn select_hotbar(self, selected_hotbar: u8) -> Result<()> {
        self.runtime
            .select_hotbar(self.player_uuid, selected_hotbar)?;
        Ok(())
    }
""",
    "gameplay chat broadcast",
)
play = replace_once(
    play,
    """                Some(PacketKind::SetCarriedItem) => {
                    let selected_hotbar = decode_hotbar_selection(&mut packet_reader)?;
                    if let Some(gameplay) = gameplay {
                        gameplay.select_hotbar(selected_hotbar)?;
                    }
                }
""",
    """                Some(PacketKind::ChatMessage) => {
                    let message = decode_chat_message(&mut packet_reader)?;
                    if let Some(gameplay) = gameplay {
                        gameplay.broadcast_chat(&message)?;
                    }
                }
                Some(PacketKind::SetCarriedItem) => {
                    let selected_hotbar = decode_hotbar_selection(&mut packet_reader)?;
                    if let Some(gameplay) = gameplay {
                        gameplay.select_hotbar(selected_hotbar)?;
                    }
                }
""",
    "chat packet handler",
)
play = replace_once(
    play,
    """fn decode_hotbar_selection(reader: &mut PacketReader<'_>) -> Result<u8> {
""",
    """fn decode_chat_message(reader: &mut PacketReader<'_>) -> Result<String> {
    let message = reader.read_string()?;
    if message.is_empty()
        || message.len() > MAX_CHAT_MESSAGE_BYTES
        || message.chars().any(char::is_control)
    {
        bail!(
            "chat message must contain 1..={MAX_CHAT_MESSAGE_BYTES} bytes and no control characters"
        );
    }

    // Modern offline-mode chat packets carry timestamp/signature/last-seen
    // metadata after the message. RoM does not claim secure-chat verification
    // yet, but it accepts and bounds the complete outer packet before reaching
    // this decoder instead of incorrectly treating those fields as trailing
    // garbage.
    let _metadata = reader.take_remaining();
    Ok(message)
}

fn decode_hotbar_selection(reader: &mut PacketReader<'_>) -> Result<u8> {
""",
    "chat message decoder",
)
play = replace_once(
    play,
    """    fn decodes_chat_commands_and_hotbar_selection_exactly() {
        let mut command = Vec::new();
        write_string(&mut command, "list").unwrap();
        assert_eq!(
            decode_chat_command(&mut PacketReader::new(&command)).unwrap(),
            "list"
        );
        assert!(decode_chat_command(&mut PacketReader::new(&[0])).is_err());

        assert_eq!(
            decode_hotbar_selection(&mut PacketReader::new(&5_i16.to_be_bytes())).unwrap(),
            5
        );
""",
    """    fn decodes_chat_messages_commands_and_hotbar_selection() {
        let mut command = Vec::new();
        write_string(&mut command, "list").unwrap();
        assert_eq!(
            decode_chat_command(&mut PacketReader::new(&command)).unwrap(),
            "list"
        );
        assert!(decode_chat_command(&mut PacketReader::new(&[0])).is_err());

        let mut message = Vec::new();
        write_string(&mut message, "hello world").unwrap();
        message.extend_from_slice(&123_i64.to_be_bytes());
        assert_eq!(
            decode_chat_message(&mut PacketReader::new(&message)).unwrap(),
            "hello world"
        );
        assert!(decode_chat_message(&mut PacketReader::new(&[0])).is_err());
        let mut control = Vec::new();
        write_string(&mut control, "bad\\nmessage").unwrap();
        assert!(decode_chat_message(&mut PacketReader::new(&control)).is_err());

        assert_eq!(
            decode_hotbar_selection(&mut PacketReader::new(&5_i16.to_be_bytes())).unwrap(),
            5
        );
""",
    "chat decoder tests",
)
play_path.write_text(play)

readme_path = Path("README.md")
readme = readme_path.read_text()
readme = replace_once(
    readme,
    "- Feature Flags, all synchronized registry data, the complete official 26.1.2 network tag manifest, and Finish Configuration",
    "- Feature Flags, all synchronized registry data, the client-synchronized subset of the official 26.1.2 network tag manifest, and Finish Configuration",
    "README configuration tags",
)
readme = replace_once(
    readme,
    "- Live online-player count in status responses",
    "- Live online-player count in status responses\n- Offline-mode player chat accepted from the generated `chat` packet and replicated through authoritative gameplay events",
    "README chat capability",
)
readme_path.write_text(readme)

roadmap_path = Path("docs/SERVER_ROADMAP.md")
roadmap = roadmap_path.read_text()
roadmap = replace_once(
    roadmap,
    "Entities, dedicated network-worker queues, broader multi-client entity tracking, and persistence are not implemented yet.",
    "Dedicated Play reader/writer queues, a shared 20 TPS runtime, authoritative gameplay persistence, inventory replication, join/leave messages, and offline-mode chat are implemented. Non-player entity gameplay, full multi-client player entity tracking, and complete Vanilla systems remain incomplete.",
    "roadmap current runtime",
)
roadmap = replace_once(
    roadmap,
    "- Send configured Feature Flags and an empty Tags payload.",
    "- Send configured Feature Flags and the client-synchronized subset of the generated official Tags payload.",
    "roadmap tags",
)
roadmap = replace_once(
    roadmap,
    "Status: started with deterministic in-memory primitives.",
    "Status: implemented through deterministic in-memory chunks, Anvil reading, and validated native snapshot persistence; Anvil writing and procedural generation remain incomplete.",
    "roadmap persistence status",
)
roadmap_path.write_text(roadmap)
