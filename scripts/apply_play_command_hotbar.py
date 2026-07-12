from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement target, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Authoritative game state owns the selected hotbar slot and emits a stable event.
replace_once(
    "crates/ferrum-game/src/state.rs",
    '''    InventoryChanged {
        uuid: PlayerUuid,
        inserted: u32,
        item: String,
    },
    PlayerKilled {''',
    '''    InventoryChanged {
        uuid: PlayerUuid,
        inserted: u32,
        item: String,
    },
    SelectedHotbarChanged {
        uuid: PlayerUuid,
        previous: u8,
        current: u8,
    },
    PlayerKilled {''',
)
replace_once(
    "crates/ferrum-game/src/state.rs",
    '''    pub fn kill_player(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {''',
    '''    pub fn select_hotbar(
        &mut self,
        uuid: PlayerUuid,
        selected_hotbar: u8,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let previous = player.inventory.selected_hotbar();
        player.inventory.select_hotbar(selected_hotbar)?;
        if previous == selected_hotbar {
            return Ok(Vec::new());
        }
        Ok(vec![GameEvent::SelectedHotbarChanged {
            uuid,
            previous,
            current: selected_hotbar,
        }])
    }

    pub fn kill_player(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {''',
)
replace_once(
    "crates/ferrum-game/src/state.rs",
    '''    #[test]
    fn detaches_live_connections_for_restart() {''',
    '''    #[test]
    fn selected_hotbar_is_authoritative_and_validated() {
        let uuid = PlayerUuid::new(8);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        let events = state.select_hotbar(uuid, 5).unwrap();
        assert_eq!(state.player(uuid).unwrap().inventory.selected_hotbar(), 5);
        assert!(matches!(
            events.as_slice(),
            [GameEvent::SelectedHotbarChanged {
                uuid: event_uuid,
                previous: 0,
                current: 5,
            }] if *event_uuid == uuid
        ));
        assert!(state.select_hotbar(uuid, 9).is_err());
    }

    #[test]
    fn detaches_live_connections_for_restart() {''',
)

# Shared runtime publishes the new mutation like every other gameplay change.
replace_once(
    "crates/ferrum-server/src/game_runtime.rs",
    '''    pub fn execute_command(
        &self,
        source: &CommandSource,
        input: &str,
    ) -> Result<CommandOutcome, GameRuntimeError> {''',
    '''    pub fn select_hotbar(
        &self,
        uuid: PlayerUuid,
        selected_hotbar: u8,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.select_hotbar(uuid, selected_hotbar)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn execute_command(
        &self,
        source: &CommandSource,
        input: &str,
    ) -> Result<CommandOutcome, GameRuntimeError> {''',
)

# Wire decoder needs the exact signed-short primitive used by 26.1.2.
replace_once(
    "crates/ferrum-server/src/codec.rs",
    '''    pub(crate) fn read_i64(&mut self) -> Result<i64> {''',
    '''    pub(crate) fn read_i16(&mut self) -> Result<i16> {
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_i64(&mut self) -> Result<i64> {''',
)
replace_once(
    "crates/ferrum-server/src/codec.rs",
    '''    #[test]
    fn reads_big_endian_float_payloads() {''',
    '''    #[test]
    fn reads_big_endian_signed_short_payloads() {
        let bytes = (-12_i16).to_be_bytes();
        let mut reader = PacketReader::new(&bytes);
        assert_eq!(reader.read_i16().unwrap(), -12);
    }

    #[test]
    fn reads_big_endian_float_payloads() {''',
)

# Play runtime connects catalog-resolved packets to authoritative gameplay.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''use ferrum_game::{PlayerUuid as GamePlayerUuid, Transform};''',
    '''use ferrum_game::{CommandSource, PlayerUuid as GamePlayerUuid, Transform};''',
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''    encode_level_chunk_with_light, encode_set_chunk_cache_center, player_action_to_world_event,
    use_item_on_block_to_world_event,
};''',
    '''    encode_level_chunk_with_light, encode_set_chunk_cache_center, encode_system_chat,
    player_action_to_world_event, use_item_on_block_to_world_event,
};''',
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''const MAX_BLOCK_INTERACTION_REACH_SQUARED: f64 =
    MAX_BLOCK_INTERACTION_REACH * MAX_BLOCK_INTERACTION_REACH;
''',
    '''const MAX_BLOCK_INTERACTION_REACH_SQUARED: f64 =
    MAX_BLOCK_INTERACTION_REACH * MAX_BLOCK_INTERACTION_REACH;
const MAX_CHAT_COMMAND_BYTES: usize = 32_767;
''',
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''    fn refresh(self, player: &mut PlayerState) -> Result<()> {
        let transform = self.runtime.with_state(|state| {
            state
                .player(self.player_uuid)
                .and_then(|player| player.entity_id)
                .and_then(|entity_id| state.entities().get(entity_id))
                .map(|entity| entity.transform)
        })?;
        if let Some(transform) = transform {
            player.position = transform.position;
            player.yaw = transform.yaw;
            player.pitch = transform.pitch;
            player.on_ground = transform.on_ground;
        }
        Ok(())
    }
}''',
    '''    fn refresh(self, player: &mut PlayerState) -> Result<()> {
        let transform = self.runtime.with_state(|state| {
            state
                .player(self.player_uuid)
                .and_then(|player| player.entity_id)
                .and_then(|entity_id| state.entities().get(entity_id))
                .map(|entity| entity.transform)
        })?;
        if let Some(transform) = transform {
            player.position = transform.position;
            player.yaw = transform.yaw;
            player.pitch = transform.pitch;
            player.on_ground = transform.on_ground;
        }
        Ok(())
    }

    fn execute_command(self, command: &str) -> Result<String> {
        let source = self
            .runtime
            .with_state(|state| {
                state.player(self.player_uuid).map(|player| {
                    CommandSource::player(
                        player.name.clone(),
                        self.player_uuid,
                        player.permission_level,
                    )
                })
            })?
            .context("authoritative player is missing while executing a command")?;
        Ok(self.runtime.execute_command(&source, command)?.feedback)
    }

    fn select_hotbar(self, selected_hotbar: u8) -> Result<()> {
        self.runtime
            .select_hotbar(self.player_uuid, selected_hotbar)?;
        Ok(())
    }
}''',
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''                Some(PacketKind::PlayerAction) => {''',
    '''                Some(PacketKind::ChatCommand) => {
                    let command = decode_chat_command(&mut packet_reader)?;
                    if let Some(gameplay) = gameplay {
                        let feedback = match gameplay.execute_command(&command) {
                            Ok(feedback) => feedback,
                            Err(error) => format!("Command failed: {error}"),
                        };
                        send_system_chat(writer, profile, &feedback, play_reader)?;
                    }
                }
                Some(PacketKind::SetCarriedItem) => {
                    let selected_hotbar = decode_hotbar_selection(&mut packet_reader)?;
                    if let Some(gameplay) = gameplay {
                        gameplay.select_hotbar(selected_hotbar)?;
                    }
                }
                Some(PacketKind::PlayerAction) => {''',
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''fn keep_alive_tick_interval(play_policy: &PlayPolicy) -> Result<usize> {''',
    '''fn decode_chat_command(reader: &mut PacketReader<'_>) -> Result<String> {
    let command = reader.read_string()?;
    if command.is_empty()
        || command.len() > MAX_CHAT_COMMAND_BYTES
        || command.chars().any(char::is_control)
    {
        bail!(
            "chat command must contain 1..={MAX_CHAT_COMMAND_BYTES} bytes and no control characters"
        );
    }
    if !reader.take_remaining().is_empty() {
        bail!("chat command packet contains trailing bytes");
    }
    Ok(command)
}

fn decode_hotbar_selection(reader: &mut PacketReader<'_>) -> Result<u8> {
    let selected = reader.read_i16()?;
    if !reader.take_remaining().is_empty() {
        bail!("set carried item packet contains trailing bytes");
    }
    let selected = u8::try_from(selected).context("selected hotbar slot cannot be negative")?;
    if selected >= ferrum_game::HOTBAR_SLOTS {
        bail!("selected hotbar slot {selected} is outside 0..{}", ferrum_game::HOTBAR_SLOTS);
    }
    Ok(selected)
}

fn send_system_chat<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    message: &str,
    play_reader: Option<&PlayReaderEndpoint>,
) -> Result<()> {
    write_or_route_play_payload(
        writer,
        profile,
        PacketKind::SystemChat,
        &encode_system_chat(message, false)?,
        play_reader,
    )?;
    if play_reader.is_none() {
        writer.flush()?;
    }
    Ok(())
}

fn keep_alive_tick_interval(play_policy: &PlayPolicy) -> Result<usize> {''',
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''    use crate::codec::{build_packet, write_packet, write_varint_vec};''',
    '''    use crate::codec::{build_packet, write_packet, write_string, write_varint_vec};''',
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    '''    #[test]
    fn configured_play_policy_controls_loaded_radius_and_keep_alive_cadence() {''',
    '''    #[test]
    fn decodes_chat_commands_and_hotbar_selection_exactly() {
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
        assert!(decode_hotbar_selection(&mut PacketReader::new(&(-1_i16).to_be_bytes())).is_err());
        assert!(decode_hotbar_selection(&mut PacketReader::new(&9_i16.to_be_bytes())).is_err());
    }

    #[test]
    fn configured_play_policy_controls_loaded_radius_and_keep_alive_cadence() {''',
)

# The replication layer records this state transition without producing a packet yet;
# equipment replication will consume it in the next capability slice.
replace_once(
    "crates/ferrum-server/src/game_replication.rs",
    '''        GameEvent::PlayerMoved { .. }
        | GameEvent::TimeChanged { .. }''',
    '''        GameEvent::PlayerMoved { .. }
        | GameEvent::SelectedHotbarChanged { .. }
        | GameEvent::TimeChanged { .. }''',
)
