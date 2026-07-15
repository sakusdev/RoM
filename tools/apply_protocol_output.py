from pathlib import Path


def one(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected 1, found {count}")
    return text.replace(old, new, 1)


path = Path("crates/ferrum-server/src/authoritative_runtime.rs")
text = path.read_text()
text = one(text, "use ferrum_play::PlayerMovement;", "use ferrum_play::PlayerMovement;\nuse ferrum_protocol::PacketKind;", "packet kind import")
text = one(
    text,
    """    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
""",
    """    /// Unframed protocol packet bytes including the packet ID.
    Packet(Vec<u8>),
    /// Version-neutral packet body resolved through the active packet table.
    ProtocolPacket { kind: PacketKind, payload: Vec<u8> },
""",
    "protocol output variant",
)
path.write_text(text)

path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text()
text = one(
    text,
    """    let data_component_protocol_ids = data_component_protocol_ids.clone();
    spawn_play_writer(
""",
    """    let data_component_protocol_ids = data_component_protocol_ids.clone();
    let protocol_profile = profile.clone();
    spawn_play_writer(
""",
    "profile clone",
)
text = one(
    text,
    """        move |writer, output| match output {
            PlayOutput::SetPlayerInventory { slot, stack } => {
""",
    """        move |writer, output| match output {
            PlayOutput::ProtocolPacket { kind, payload } => {
                if let Some(packet_id) = protocol_profile.packets().id(kind) {
                    write_packet(
                        writer,
                        &build_packet(packet_id, |body| {
                            body.extend_from_slice(&payload);
                            Ok(())
                        })?,
                    )?;
                    writer.flush()?;
                }
                Ok(PlayWriterDirective::Continue)
            }
            PlayOutput::SetPlayerInventory { slot, stack } => {
""",
    "protocol output handler",
)
text = one(
    text,
    """        PlayOutput::SetPlayerInventory { .. }
        | PlayOutput::SetContainerContent { .. }
        | PlayOutput::SetContainerSlot { .. } => {
""",
    """        PlayOutput::ProtocolPacket { .. }
        | PlayOutput::SetPlayerInventory { .. }
        | PlayOutput::SetContainerContent { .. }
        | PlayOutput::SetContainerSlot { .. } => {
""",
    "fallback exhaustiveness",
)
path.write_text(text)

path = Path("crates/ferrum-server/src/play_writer.rs")
text = path.read_text()
text = text.replace(
    """                    PlayOutput::Packet(bytes) => writer.write_all(&bytes)?,
                    PlayOutput::KeepAliveRequest(id) => writer.write_all(&id.to_be_bytes())?,
""",
    """                    PlayOutput::Packet(bytes) => writer.write_all(&bytes)?,
                    PlayOutput::ProtocolPacket { .. } => {}
                    PlayOutput::KeepAliveRequest(id) => writer.write_all(&id.to_be_bytes())?,
""",
)
text = text.replace(
    """                PlayOutput::Packet(_)
                | PlayOutput::KeepAliveRequest(_)
""",
    """                PlayOutput::Packet(_)
                | PlayOutput::ProtocolPacket { .. }
                | PlayOutput::KeepAliveRequest(_)
""",
)
path.write_text(text)
