from pathlib import Path

path = Path("crates/ferrum-protocol/src/packet_catalog.rs")
text = path.read_text(encoding="utf-8")
marker = "#[derive(Debug, Error, PartialEq, Eq)]\npub enum PacketCatalogError {"
if marker not in text:
    raise SystemExit("packet catalog error marker not found")
function = r'''#[must_use]
pub const fn canonical_packet_name(kind: PacketKind) -> &'static str {
    match kind {
        PacketKind::Handshake => "minecraft:intention",
        PacketKind::StatusRequest => "minecraft:status_request",
        PacketKind::PingRequest => "minecraft:ping_request",
        PacketKind::StatusResponse => "minecraft:status_response",
        PacketKind::PongResponse => "minecraft:pong_response",
        PacketKind::LoginStart => "minecraft:hello",
        PacketKind::LoginAcknowledged => "minecraft:login_acknowledged",
        PacketKind::LoginDisconnect => "minecraft:login_disconnect",
        PacketKind::LoginSuccess => "minecraft:game_profile",
        PacketKind::ConfigurationAcknowledged => "minecraft:configuration_acknowledged",
        PacketKind::ConfigurationClientInformation => "minecraft:client_information",
        PacketKind::ConfigurationDisconnect => "minecraft:disconnect",
        PacketKind::RegistryData => "minecraft:registry_data",
        PacketKind::FeatureFlags => "minecraft:update_enabled_features",
        PacketKind::UpdateTags => "minecraft:update_tags",
        PacketKind::SelectKnownPacksRequest | PacketKind::SelectKnownPacksResponse => {
            "minecraft:select_known_packs"
        }
        PacketKind::FinishConfiguration => "minecraft:finish_configuration",
        PacketKind::PlayLogin => "minecraft:login",
        PacketKind::ChunkBatchStart => "minecraft:chunk_batch_start",
        PacketKind::ChunkBatchFinished => "minecraft:chunk_batch_finished",
        PacketKind::ChunkBatchReceived => "minecraft:chunk_batch_received",
        PacketKind::LevelChunkWithLight => "minecraft:level_chunk_with_light",
        PacketKind::SetChunkCacheCenter => "minecraft:set_chunk_cache_center",
        PacketKind::DefaultSpawnPosition => "minecraft:set_default_spawn_position",
        PacketKind::PlayerPosition => "minecraft:player_position",
        PacketKind::SystemChat => "minecraft:system_chat",
        PacketKind::AcceptTeleportation => "minecraft:accept_teleportation",
        PacketKind::PlayDisconnect => "minecraft:disconnect",
        PacketKind::KeepAliveRequest | PacketKind::KeepAliveResponse => "minecraft:keep_alive",
        PacketKind::ClientTickEnd => "minecraft:client_tick_end",
        PacketKind::MovePlayerPosition => "minecraft:move_player_pos",
        PacketKind::MovePlayerPositionRotation => "minecraft:move_player_pos_rot",
        PacketKind::MovePlayerRotation => "minecraft:move_player_rot",
        PacketKind::MovePlayerStatusOnly => "minecraft:move_player_status_only",
        PacketKind::PlayerAction => "minecraft:player_action",
        PacketKind::UseItemOn => "minecraft:use_item_on",
        PacketKind::BlockChangedAck => "minecraft:block_changed_ack",
        PacketKind::BlockUpdate => "minecraft:block_update",
        PacketKind::ForgetLevelChunk => "minecraft:forget_level_chunk",
        PacketKind::ChatCommand => "minecraft:chat_command",
        PacketKind::ChatMessage => "minecraft:chat",
        PacketKind::SetCarriedItem => "minecraft:set_carried_item",
        PacketKind::ContainerClick => "minecraft:container_click",
        PacketKind::CloseContainer => "minecraft:container_close",
        PacketKind::SetCreativeModeSlot => "minecraft:set_creative_mode_slot",
        PacketKind::SetHeldSlot => "minecraft:set_held_slot",
        PacketKind::SetContainerContent => "minecraft:container_set_content",
        PacketKind::SetContainerSlot => "minecraft:container_set_slot",
        PacketKind::AddEntity => "minecraft:add_entity",
        PacketKind::RemoveEntities => "minecraft:remove_entities",
        PacketKind::MoveEntityPosition => "minecraft:move_entity_pos",
        PacketKind::MoveEntityPositionRotation => "minecraft:move_entity_pos_rot",
        PacketKind::MoveEntityRotation => "minecraft:move_entity_rot",
        PacketKind::TeleportEntity => "minecraft:teleport_entity",
        PacketKind::RotateHead => "minecraft:rotate_head",
        PacketKind::SetEntityData => "minecraft:set_entity_data",
        PacketKind::SetEquipment => "minecraft:set_equipment",
        PacketKind::PlayerInfoUpdate => "minecraft:player_info_update",
        PacketKind::PlayerInfoRemove => "minecraft:player_info_remove",
    }
}

'''
path.write_text(text.replace(marker, function + marker, 1), encoding="utf-8")

path = Path("crates/ferrum-protocol/src/lib.rs")
text = path.read_text(encoding="utf-8")
old = '''pub use packet_catalog::{
    PacketCatalog, PacketCatalogError, PacketDescriptor, known_packet_kind,
    normalize_packet_name,
};'''
new = '''pub use packet_catalog::{
    PacketCatalog, PacketCatalogError, PacketDescriptor, canonical_packet_name,
    known_packet_kind, normalize_packet_name,
};'''
if old not in text:
    raise SystemExit("packet catalog export target not found")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
