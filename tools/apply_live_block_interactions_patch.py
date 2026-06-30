from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:160]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Protocol semantics: block prediction acknowledgement is a clientbound Play packet.
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "    PlayerAction,\n    UseItemOn,\n    BlockUpdate,\n",
    "    PlayerAction,\n    UseItemOn,\n    BlockChangedAck,\n    BlockUpdate,\n",
)
replace_once(
    "crates/ferrum-protocol/src/lib.rs",
    "            | Self::PlayerAction\n            | Self::UseItemOn\n            | Self::BlockUpdate\n",
    "            | Self::PlayerAction\n"
    "            | Self::UseItemOn\n"
    "            | Self::BlockChangedAck\n"
    "            | Self::BlockUpdate\n",
)

# Exact Minecraft Java Edition 26.1.2 / protocol 775 packet IDs.
replace_once(
    "crates/ferrum-version-26-1-2/src/lib.rs",
    "        (PacketKind::MovePlayerStatusOnly, 0x21),\n"
    "        (PacketKind::ForgetLevelChunk, 0x25),\n",
    "        (PacketKind::MovePlayerStatusOnly, 0x21),\n"
    "        (PacketKind::PlayerAction, 0x29),\n"
    "        (PacketKind::UseItemOn, 0x42),\n"
    "        (PacketKind::BlockChangedAck, 0x04),\n"
    "        (PacketKind::BlockUpdate, 0x08),\n"
    "        (PacketKind::ForgetLevelChunk, 0x25),\n",
)
replace_once(
    "crates/ferrum-version-26-1-2/src/lib.rs",
    "        assert_eq!(packets.require(PacketKind::ForgetLevelChunk).unwrap(), 0x25);\n",
    "        assert_eq!(packets.require(PacketKind::PlayerAction).unwrap(), 0x29);\n"
    "        assert_eq!(packets.require(PacketKind::UseItemOn).unwrap(), 0x42);\n"
    "        assert_eq!(packets.require(PacketKind::BlockChangedAck).unwrap(), 0x04);\n"
    "        assert_eq!(packets.require(PacketKind::BlockUpdate).unwrap(), 0x08);\n"
    "        assert_eq!(packets.require(PacketKind::ForgetLevelChunk).unwrap(), 0x25);\n",
)

# Add the prediction acknowledgement payload codec.
replace_once(
    "crates/ferrum-play/src/lib.rs",
    "pub fn encode_block_update(\n"
    "    position: BlockPosition,\n"
    "    state: BlockStateId,\n"
    ") -> Result<Vec<u8>, PlayEncodeError> {\n",
    "pub fn encode_block_changed_ack(sequence: i32) -> Result<Vec<u8>, PlayEncodeError> {\n"
    "    require_non_negative(\"block change sequence\", sequence)?;\n"
    "    let mut output = Vec::new();\n"
    "    write_varint(&mut output, sequence);\n"
    "    Ok(output)\n"
    "}\n\n"
    "pub fn encode_block_update(\n"
    "    position: BlockPosition,\n"
    "    state: BlockStateId,\n"
    ") -> Result<Vec<u8>, PlayEncodeError> {\n",
)
replace_once(
    "crates/ferrum-play/src/lib.rs",
    "    #[test]\n    fn encodes_block_update_exactly() {\n",
    "    #[test]\n"
    "    fn encodes_block_change_ack_exactly() {\n"
    "        assert_eq!(encode_block_changed_ack(300).unwrap(), vec![0xac, 0x02]);\n"
    "        assert_eq!(\n"
    "            encode_block_changed_ack(-1).unwrap_err(),\n"
    "            PlayEncodeError::NegativeValue {\n"
    "                field: \"block change sequence\",\n"
    "                value: -1,\n"
    "            }\n"
    "        );\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn encodes_block_update_exactly() {\n",
)

# Complete the official 26.1.2 action enum, validate prediction sequences,
# and place the simplified stone block on the clicked face's adjacent position.
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "    SwapItemWithOffhand,\n}\n",
    "    SwapItemWithOffhand,\n    Stab,\n}\n",
)
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "    let sequence = reader.read_varint()?;\n"
    "    reader.require_empty()?;\n"
    "    Ok(PlayerAction {\n",
    "    let sequence = decode_sequence(reader.read_varint()?)?;\n"
    "    reader.require_empty()?;\n"
    "    Ok(PlayerAction {\n",
)
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "    let sequence = reader.read_varint()?;\n"
    "    reader.require_empty()?;\n"
    "    Ok(UseItemOnBlock {\n",
    "    let sequence = decode_sequence(reader.read_varint()?)?;\n"
    "    reader.require_empty()?;\n"
    "    Ok(UseItemOnBlock {\n",
)
old_mapping = '''#[must_use]
pub fn use_item_on_block_to_world_event(
    interaction: UseItemOnBlock,
    placed_state: BlockStateId,
) -> WorldEvent {
    WorldEvent::BlockMutation(BlockMutation {
        position: block_position_to_world(interaction.position),
        state: placed_state,
    })
}
'''
new_mapping = '''#[must_use]
pub fn use_item_on_block_to_world_event(
    interaction: UseItemOnBlock,
    placed_state: BlockStateId,
) -> Option<WorldEvent> {
    if interaction.world_border_hit {
        return None;
    }
    let [dx, dy, dz] = interaction.face.offset();
    Some(WorldEvent::BlockMutation(BlockMutation {
        position: BlockPos {
            x: interaction.position.x + dx,
            y: interaction.position.y + dy,
            z: interaction.position.z + dz,
        },
        state: placed_state,
    }))
}
'''
replace_once("crates/ferrum-play/src/block_interaction.rs", old_mapping, new_mapping)
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "fn decode_hand(value: i32) -> Result<InteractionHand, BlockInteractionDecodeError> {\n",
    "impl BlockFace {\n"
    "    #[must_use]\n"
    "    const fn offset(self) -> [i32; 3] {\n"
    "        match self {\n"
    "            Self::Down => [0, -1, 0],\n"
    "            Self::Up => [0, 1, 0],\n"
    "            Self::North => [0, 0, -1],\n"
    "            Self::South => [0, 0, 1],\n"
    "            Self::West => [-1, 0, 0],\n"
    "            Self::East => [1, 0, 0],\n"
    "        }\n"
    "    }\n"
    "}\n\n"
    "fn decode_sequence(value: i32) -> Result<i32, BlockInteractionDecodeError> {\n"
    "    if value < 0 {\n"
    "        Err(BlockInteractionDecodeError::NegativeSequence(value))\n"
    "    } else {\n"
    "        Ok(value)\n"
    "    }\n"
    "}\n\n"
    "fn decode_hand(value: i32) -> Result<InteractionHand, BlockInteractionDecodeError> {\n",
)
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "        6 => Ok(PlayerActionStatus::SwapItemWithOffhand),\n"
    "        other => Err(BlockInteractionDecodeError::InvalidPlayerActionStatus(\n",
    "        6 => Ok(PlayerActionStatus::SwapItemWithOffhand),\n"
    "        7 => Ok(PlayerActionStatus::Stab),\n"
    "        other => Err(BlockInteractionDecodeError::InvalidPlayerActionStatus(\n",
)
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "    #[error(\"boolean field must be 0 or 1, got {0}\")]\n"
    "    InvalidBool(u8),\n",
    "    #[error(\"block interaction sequence cannot be negative: {0}\")]\n"
    "    NegativeSequence(i32),\n"
    "    #[error(\"boolean field must be 0 or 1, got {0}\")]\n"
    "    InvalidBool(u8),\n",
)
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "            decode_player_action(&[7]).unwrap_err(),\n"
    "            BlockInteractionDecodeError::InvalidPlayerActionStatus(7)\n",
    "            decode_player_action(&[8]).unwrap_err(),\n"
    "            BlockInteractionDecodeError::InvalidPlayerActionStatus(8)\n",
)
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "            WorldEvent::BlockMutation(BlockMutation {\n"
    "                position: BlockPos { x: 4, y: 65, z: -2 },\n"
    "                state: stone,\n"
    "            })\n"
    "        );\n",
    "            Some(WorldEvent::BlockMutation(BlockMutation {\n"
    "                position: BlockPos { x: 5, y: 65, z: -2 },\n"
    "                state: stone,\n"
    "            }))\n"
    "        );\n"
    "        assert_eq!(\n"
    "            use_item_on_block_to_world_event(\n"
    "                UseItemOnBlock {\n"
    "                    hand: InteractionHand::Main,\n"
    "                    position,\n"
    "                    face: BlockFace::Up,\n"
    "                    cursor: [0.5, 0.5, 0.5],\n"
    "                    inside_block: false,\n"
    "                    world_border_hit: true,\n"
    "                    sequence: 6,\n"
    "                },\n"
    "                stone,\n"
    "            ),\n"
    "            None\n"
    "        );\n",
)
# Add a complete STAB action fixture without changing its no-mutation behavior.
replace_once(
    "crates/ferrum-play/src/block_interaction.rs",
    "    #[test]\n    fn decodes_use_item_on_block_exactly() {\n",
    "    #[test]\n"
    "    fn decodes_26_1_2_stab_action() {\n"
    "        let mut payload = Vec::new();\n"
    "        write_varint(&mut payload, 7);\n"
    "        payload.extend_from_slice(&0_i64.to_be_bytes());\n"
    "        payload.push(0);\n"
    "        write_varint(&mut payload, 1);\n"
    "        assert_eq!(\n"
    "            decode_player_action(&payload).unwrap().status,\n"
    "            PlayerActionStatus::Stab\n"
    "        );\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn decodes_use_item_on_block_exactly() {\n",
)

# Activate the packet handlers, send prediction acknowledgements, and only
# apply simplified placement when the world border was not hit.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    decode_player_action, decode_use_item_on_block, encode_block_update,\n",
    "    decode_player_action, decode_use_item_on_block, encode_block_changed_ack,\n"
    "    encode_block_update,\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "                Some(PacketKind::PlayerAction) => {\n"
    "                    let action = decode_player_action(packet_reader.take_remaining())?;\n"
    "                    if let Some(event) = player_action_to_world_event(\n"
    "                        action,\n"
    "                        BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),\n"
    "                    ) {\n"
    "                        let applied = shared_world.apply_event(connection, event)?;\n"
    "                        send_world_updates(writer, profile, &applied)?;\n"
    "                    }\n"
    "                }\n",
    "                Some(PacketKind::PlayerAction) => {\n"
    "                    let action = decode_player_action(packet_reader.take_remaining())?;\n"
    "                    let sequence = action.sequence;\n"
    "                    if let Some(event) = player_action_to_world_event(\n"
    "                        action,\n"
    "                        BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),\n"
    "                    ) {\n"
    "                        let applied = shared_world.apply_event(connection, event)?;\n"
    "                        send_world_updates(writer, profile, &applied)?;\n"
    "                    }\n"
    "                    send_block_changed_ack(writer, profile, sequence)?;\n"
    "                }\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "                Some(PacketKind::UseItemOn) => {\n"
    "                    let interaction = decode_use_item_on_block(packet_reader.take_remaining())?;\n"
    "                    let event = use_item_on_block_to_world_event(\n"
    "                        interaction,\n"
    "                        BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),\n"
    "                    );\n"
    "                    let applied = shared_world.apply_event(connection, event)?;\n"
    "                    send_world_updates(writer, profile, &applied)?;\n"
    "                }\n",
    "                Some(PacketKind::UseItemOn) => {\n"
    "                    let interaction = decode_use_item_on_block(packet_reader.take_remaining())?;\n"
    "                    let sequence = interaction.sequence;\n"
    "                    if let Some(event) = use_item_on_block_to_world_event(\n"
    "                        interaction,\n"
    "                        BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),\n"
    "                    ) {\n"
    "                        let applied = shared_world.apply_event(connection, event)?;\n"
    "                        send_world_updates(writer, profile, &applied)?;\n"
    "                    }\n"
    "                    send_block_changed_ack(writer, profile, sequence)?;\n"
    "                }\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn send_world_updates<W: Write>(\n",
    "fn send_block_changed_ack<W: Write>(\n"
    "    writer: &mut W,\n"
    "    profile: &ProtocolProfile,\n"
    "    sequence: i32,\n"
    ") -> Result<()> {\n"
    "    write_play_payload(\n"
    "        writer,\n"
    "        profile,\n"
    "        PacketKind::BlockChangedAck,\n"
    "        &encode_block_changed_ack(sequence)?,\n"
    "    )?;\n"
    "    writer.flush()?;\n"
    "    Ok(())\n"
    "}\n\n"
    "fn send_world_updates<W: Write>(\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    #[test]\n    fn shared_world_applies_events_from_multiple_connections_to_one_store() {\n",
    "    #[test]\n"
    "    fn sends_block_change_prediction_acknowledgement() {\n"
    "        let mut packets = PacketTable::new();\n"
    "        packets.insert(PacketKind::BlockChangedAck, 0x04).unwrap();\n"
    "        let profile = ProtocolProfile::new(\"Test\", 1, packets).unwrap();\n"
    "        let mut output = Vec::new();\n"
    "        send_block_changed_ack(&mut output, &profile, 300).unwrap();\n"
    "        assert_eq!(output, [3, 0x04, 0xac, 0x02]);\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn shared_world_applies_events_from_multiple_connections_to_one_store() {\n",
)

# Documentation: mark live protocol IDs and prediction acknowledgement complete.
replace_once(
    "README.md",
    "- Bounded serverbound block interaction payload decoding and local deterministic mutation application\n"
    "- Shared in-memory world state for accepted block mutations across Play connections\n"
    "- Clientbound block-update payload encoding for accepted mutations when the active protocol profile exposes that packet ID\n",
    "- Bounded protocol-775 Player Action and Use Item On decoding with prediction-sequence validation\n"
    "- Shared in-memory world state for accepted block mutations across Play connections\n"
    "- Live simplified block breaking and adjacent-face stone placement\n"
    "- Clientbound Block Update and Block Changed Ack responses for accepted interactions\n",
)
replace_once(
    "README.md",
    "- Verified 26.1.2 packet IDs for live block breaking and placement packets in the built-in profile\n",
    "- Full item, inventory, replaceability, reach, collision, and game-mode validation for block interactions\n",
)
replace_once(
    "README.md",
    "→ Dynamic Chunk Load / Unload\n→ Keep Alive\n",
    "→ Dynamic Chunk Load / Unload\n"
    "→ Block Break / Simplified Placement\n"
    "→ Block Update / Prediction Ack\n"
    "→ Keep Alive\n",
)
replace_once(
    "docs/SERVER_ROADMAP.md",
    "- Encode clientbound Block Update packets and send accepted mutations when the profile exposes `BlockUpdate`.\n"
    "- Keep protocol serialization and version-specific numeric IDs outside the world crate.\n",
    "- Encode clientbound Block Update packets and send accepted mutations when the profile exposes `BlockUpdate`.\n"
    "- Register exact protocol-775 IDs for Player Action, Use Item On, Block Update, and Block Changed Ack.\n"
    "- Acknowledge client prediction sequences and support the complete 26.1.2 Player Action enum.\n"
    "- Apply simplified adjacent-face stone placement while rejecting world-border hits.\n"
    "- Keep protocol serialization and version-specific numeric IDs outside the world crate.\n",
)
replace_once(
    "docs/SERVER_ROADMAP.md",
    "- Verify exact 26.1.2 packet IDs for Player Action, Use Item On, and Block Update before adding them to the built-in profile.\n",
    "- Add inventory-aware placement, block replaceability, reach, collision, game-mode, and tool-speed validation.\n",
)
