from pathlib import Path


def patch(path: str, transform) -> None:
    file = Path(path)
    text = file.read_text()
    updated = transform(text)
    if updated == text:
        raise SystemExit(f"no changes made to {path}")
    file.write_text(updated)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def patch_inventory(text: str) -> str:
    text = replace_once(
        text,
        """use ferrum_game::{
    ContainerClick, ContainerClickKind, ItemStack, MAX_CONTAINER_SLOTS, PLAYER_INVENTORY_SLOTS,
};
""",
        """use ferrum_game::{
    ContainerClick, ContainerClickKind, EntityId, EquipmentSlot, ItemStack, MAX_CONTAINER_SLOTS,
    PLAYER_INVENTORY_SLOTS,
};
""",
        "inventory imports",
    )
    anchor = """fn validate_registry_entry(name: &str, protocol_id: i32) -> Result<(), InventoryEncodeError> {
"""
    addition = """#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentEntry {
    pub slot: EquipmentSlot,
    pub stack: Option<ItemStack>,
}

impl EquipmentEntry {
    #[must_use]
    pub const fn new(slot: EquipmentSlot, stack: Option<ItemStack>) -> Self {
        Self { slot, stack }
    }
}

pub fn encode_set_equipment(
    entity_id: EntityId,
    entries: &[EquipmentEntry],
    items: &ItemProtocolRegistry,
    components: &DataComponentProtocolRegistry,
) -> Result<Option<Vec<u8>>, InventoryEncodeError> {
    if entries.is_empty() {
        return Err(InventoryEncodeError::EmptyEquipmentEntries);
    }
    let entity_id = i32::try_from(entity_id.get()).map_err(|_| {
        InventoryEncodeError::EntityIdOutOfRange {
            entity_id: entity_id.get(),
        }
    })?;
    let mut output = Vec::new();
    write_varint(&mut output, entity_id);
    let mut seen = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let protocol_slot = equipment_protocol_slot(entry.slot);
        if !seen.insert(protocol_slot) {
            return Err(InventoryEncodeError::DuplicateEquipmentSlot { slot: entry.slot });
        }
        let continuation = if index + 1 < entries.len() { 0x80 } else { 0 };
        output.push(protocol_slot | continuation);
        if !encode_item_stack_into(&mut output, entry.stack.as_ref(), items, components)? {
            return Ok(None);
        }
    }
    Ok(Some(output))
}

const fn equipment_protocol_slot(slot: EquipmentSlot) -> u8 {
    match slot {
        EquipmentSlot::MainHand => 0,
        EquipmentSlot::OffHand => 1,
        EquipmentSlot::Feet => 2,
        EquipmentSlot::Legs => 3,
        EquipmentSlot::Chest => 4,
        EquipmentSlot::Head => 5,
    }
}

"""
    text = replace_once(text, anchor, addition + anchor, "equipment encoder insertion")
    text = replace_once(
        text,
        """    #[error("player inventory slot {slot} is outside 0..{PLAYER_INVENTORY_SLOTS}")]
    SlotOutOfRange { slot: usize },
""",
        """    #[error("player inventory slot {slot} is outside 0..{PLAYER_INVENTORY_SLOTS}")]
    SlotOutOfRange { slot: usize },
    #[error("equipment update must contain at least one entry")]
    EmptyEquipmentEntries,
    #[error("duplicate equipment slot {slot:?}")]
    DuplicateEquipmentSlot { slot: EquipmentSlot },
    #[error("entity ID {entity_id} exceeds the protocol VarInt range")]
    EntityIdOutOfRange { entity_id: u32 },
""",
        "equipment errors",
    )
    test = """

    #[test]
    fn encodes_equipment_continuation_slots_and_rejects_duplicates() {
        let entity_id = EntityId::new(9).unwrap();
        let stone = ItemStack::new("minecraft:stone", 1).unwrap();
        let entries = [
            EquipmentEntry::new(EquipmentSlot::MainHand, Some(stone.clone())),
            EquipmentEntry::new(EquipmentSlot::Head, None),
        ];
        assert_eq!(
            encode_set_equipment(
                entity_id,
                &entries,
                &items(),
                &DataComponentProtocolRegistry::default(),
            )
            .unwrap(),
            Some(vec![9, 0x80, 1, 1, 0, 0, 5, 0])
        );
        assert_eq!(
            encode_set_equipment(
                entity_id,
                &[
                    EquipmentEntry::new(EquipmentSlot::MainHand, Some(stone.clone())),
                    EquipmentEntry::new(EquipmentSlot::MainHand, Some(stone)),
                ],
                &items(),
                &DataComponentProtocolRegistry::default(),
            )
            .unwrap_err(),
            InventoryEncodeError::DuplicateEquipmentSlot {
                slot: EquipmentSlot::MainHand
            }
        );
        let unknown = ItemStack::new("minecraft:unknown", 1).unwrap();
        assert_eq!(
            encode_set_equipment(
                entity_id,
                &[EquipmentEntry::new(
                    EquipmentSlot::MainHand,
                    Some(unknown)
                )],
                &items(),
                &DataComponentProtocolRegistry::default(),
            )
            .unwrap(),
            None
        );
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("inventory tests closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_play_lib(text: str) -> str:
    return replace_once(
        text,
        """    DataComponentProtocolRegistry, InventoryDecodeError, InventoryEncodeError,
    ItemProtocolRegistry, decode_close_container, decode_container_click,
    decode_creative_slot_update, encode_item_stack, encode_set_container_content,
    encode_set_container_slot, encode_set_player_inventory,
    encode_set_player_inventory_with_components,
""",
        """    DataComponentProtocolRegistry, EquipmentEntry, InventoryDecodeError, InventoryEncodeError,
    ItemProtocolRegistry, decode_close_container, decode_container_click,
    decode_creative_slot_update, encode_item_stack, encode_set_container_content,
    encode_set_container_slot, encode_set_equipment, encode_set_player_inventory,
    encode_set_player_inventory_with_components,
""",
        "ferrum-play exports",
    )


def patch_replication(text: str) -> str:
    text = replace_once(
        text,
        """use ferrum_game::{
    EntityId, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerUuid, Transform,
    Velocity,
};
use ferrum_play::{
    EncodedEntityMovement, EntityMovementKind, EntityProtocolRegistry, PlayerInfoEntry,
    encode_add_entity, encode_empty_entity_data, encode_entity_movement, encode_player_info_remove,
    encode_player_info_update, encode_remove_entities, encode_rotate_head, encode_teleport_entity,
};
""",
        """use ferrum_game::{
    EntityId, EquipmentSlot, GameEvent, GameMode, GameState, PLAYER_INVENTORY_SLOTS, PlayerState,
    PlayerUuid, Transform, Velocity,
};
use ferrum_play::{
    DataComponentProtocolRegistry, EncodedEntityMovement, EntityMovementKind,
    EntityProtocolRegistry, EquipmentEntry, ItemProtocolRegistry, PlayerInfoEntry,
    encode_add_entity, encode_empty_entity_data, encode_entity_movement, encode_player_info_remove,
    encode_player_info_update, encode_remove_entities, encode_rotate_head, encode_set_equipment,
    encode_teleport_entity,
};
""",
        "replication imports",
    )
    text = replace_once(
        text,
        """    pub poll_interval: Duration,
    pub entity_protocol_ids: EntityProtocolRegistry,
""",
        """    pub poll_interval: Duration,
    pub entity_protocol_ids: EntityProtocolRegistry,
    pub item_protocol_ids: ItemProtocolRegistry,
    pub data_component_protocol_ids: DataComponentProtocolRegistry,
""",
        "replication config fields",
    )
    text = replace_once(
        text,
        """            poll_interval: DEFAULT_POLL_INTERVAL,
            entity_protocol_ids: EntityProtocolRegistry::default(),
""",
        """            poll_interval: DEFAULT_POLL_INTERVAL,
            entity_protocol_ids: EntityProtocolRegistry::default(),
            item_protocol_ids: ItemProtocolRegistry::default(),
            data_component_protocol_ids: DataComponentProtocolRegistry::default(),
""",
        "replication config defaults",
    )
    text = replace_once(
        text,
        """    transform: Transform,
    velocity: Velocity,
""",
        """    transform: Transform,
    velocity: Velocity,
    equipment: Vec<EquipmentEntry>,
    selected_hotbar: u8,
""",
        "player snapshot equipment fields",
    )
    text = replace_once(
        text,
        """            &mut connections,
            config.pending_output_limit.get(),
            &config.entity_protocol_ids,
            &mut exit,
""",
        """            &mut connections,
            &config,
            &mut exit,
""",
        "process commands config call",
    )
    text = text.replace(
        """                    &config.entity_protocol_ids,
                    &mut connections,
""",
        """                    &config,
                    &mut connections,
""",
    )
    if text.count("&config,\n                    &mut connections,") != 2:
        raise SystemExit("dispatch config calls were not both updated")
    text = replace_once(
        text,
        """    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    pending_limit: usize,
    entity_protocol_ids: &EntityProtocolRegistry,
    exit: &mut GameReplicationExit,
""",
        """    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
""",
        "process commands signature",
    )
    text = replace_once(
        text,
        """                let mut connection = ReplicationConnection::new(endpoint, pending_limit);
                if entity_replication_enabled(entity_protocol_ids) {
""",
        """                let mut connection = ReplicationConnection::new(
                    endpoint,
                    config.pending_output_limit.get(),
                );
                if entity_replication_enabled(&config.entity_protocol_ids) {
""",
        "register config use",
    )
    text = replace_once(
        text,
        """                                    snapshot,
                                    entity_protocol_ids,
                                    exit,
""",
        """                                    snapshot,
                                    config,
                                    exit,
""",
        "register spawn config",
    )
    text = replace_once(
        text,
        """    runtime: &SharedGameRuntime,
    entity_protocol_ids: &EntityProtocolRegistry,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
""",
        """    runtime: &SharedGameRuntime,
    config: &GameReplicationConfig,
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
""",
        "dispatch signature",
    )
    text = text.replace(
        "entity_replication_enabled(entity_protocol_ids)",
        "entity_replication_enabled(&config.entity_protocol_ids)",
    )
    text = text.replace(
        """                            snapshot.clone(),
                            entity_protocol_ids,
                            exit,
""",
        """                            snapshot.clone(),
                            config,
                            exit,
""",
    )
    text = text.replace(
        """                                    snapshot,
                                    entity_protocol_ids,
                                    exit,
""",
        """                                    snapshot,
                                    config,
                                    exit,
""",
    )
    if "entity_protocol_ids," in text[text.index("fn dispatch_event("):text.index("fn entity_replication_enabled(")]:
        raise SystemExit("dispatch still contains stale entity_protocol_ids argument")

    old_inventory = """        GameEvent::InventorySlotChanged { uuid, slot, stack } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(PlayOutput::SetPlayerInventory { slot, stack }, exit);
            }
        }
"""
    new_inventory = """        GameEvent::InventorySlotChanged { uuid, slot, stack } => {
            let equipment_update = if entity_replication_enabled(&config.entity_protocol_ids) {
                player_snapshot(runtime, uuid)?.and_then(|snapshot| {
                    equipment_slot_for_inventory_index(slot, snapshot.selected_hotbar).map(
                        |equipment_slot| {
                            (
                                snapshot.entity_id,
                                EquipmentEntry::new(equipment_slot, stack.clone()),
                            )
                        },
                    )
                })
            } else {
                None
            };
            if let Some(connection) = connections.get_mut(&uuid) {
                connection.queue(
                    PlayOutput::SetPlayerInventory {
                        slot,
                        stack: stack.clone(),
                    },
                    exit,
                );
            }
            if let Some((entity_id, entry)) = equipment_update {
                queue_equipment_except(connections, uuid, entity_id, &[entry], config, exit)?;
            }
        }
"""
    text = replace_once(text, old_inventory, new_inventory, "inventory equipment dispatch")
    text = replace_once(
        text,
        """        GameEvent::SelectedHotbarChanged { .. }
        | GameEvent::TimeChanged { .. }
""",
        """        GameEvent::SelectedHotbarChanged { uuid, current, .. } => {
            if entity_replication_enabled(&config.entity_protocol_ids) {
                if let Some(snapshot) = player_snapshot(runtime, uuid)? {
                    let entry = snapshot
                        .equipment
                        .iter()
                        .find(|entry| entry.slot == EquipmentSlot::MainHand)
                        .cloned()
                        .unwrap_or_else(|| EquipmentEntry::new(EquipmentSlot::MainHand, None));
                    debug_assert_eq!(snapshot.selected_hotbar, current);
                    queue_equipment_except(
                        connections,
                        uuid,
                        snapshot.entity_id,
                        &[entry],
                        config,
                        exit,
                    )?;
                }
            }
        }
        GameEvent::TimeChanged { .. }
""",
        "selected hotbar equipment dispatch",
    )
    text = replace_once(
        text,
        """        transform: entity.transform,
        velocity: entity.velocity,
""",
        """        transform: entity.transform,
        velocity: entity.velocity,
        equipment: player_equipment(player),
        selected_hotbar: player.inventory.selected_hotbar(),
""",
        "snapshot equipment population",
    )
    text = replace_once(
        text,
        """fn queue_player_spawn(
    connection: &mut ReplicationConnection,
    snapshot: PlayerEntitySnapshot,
    registry: &EntityProtocolRegistry,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !entity_replication_enabled(registry) {
""",
        """fn queue_player_spawn(
    connection: &mut ReplicationConnection,
    snapshot: PlayerEntitySnapshot,
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    if !entity_replication_enabled(&config.entity_protocol_ids) {
""",
        "spawn config signature",
    )
    text = replace_once(
        text,
        """        snapshot.velocity,
        registry,
""",
        """        snapshot.velocity,
        &config.entity_protocol_ids,
""",
        "spawn entity registry",
    )
    text = replace_once(
        text,
        """    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::RotateHead,
""",
        """    if snapshot.equipment.iter().any(|entry| entry.stack.is_some()) {
        queue_player_equipment(
            connection,
            snapshot.entity_id,
            &snapshot.equipment,
            config,
            exit,
        )?;
    }
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::RotateHead,
""",
        "initial equipment insertion",
    )
    helper_anchor = """fn queue_player_remove(
"""
    helpers = """fn player_equipment(player: &PlayerState) -> Vec<EquipmentEntry> {
    [
        EquipmentSlot::MainHand,
        EquipmentSlot::OffHand,
        EquipmentSlot::Feet,
        EquipmentSlot::Legs,
        EquipmentSlot::Chest,
        EquipmentSlot::Head,
    ]
    .into_iter()
    .map(|slot| EquipmentEntry::new(slot, player.inventory.equipment(slot).cloned()))
    .collect()
}

fn equipment_slot_for_inventory_index(
    inventory_index: usize,
    selected_hotbar: u8,
) -> Option<EquipmentSlot> {
    [
        EquipmentSlot::MainHand,
        EquipmentSlot::OffHand,
        EquipmentSlot::Feet,
        EquipmentSlot::Legs,
        EquipmentSlot::Chest,
        EquipmentSlot::Head,
    ]
    .into_iter()
    .find(|slot| slot.inventory_index(selected_hotbar) == inventory_index)
}

fn queue_player_equipment(
    connection: &mut ReplicationConnection,
    entity_id: EntityId,
    entries: &[EquipmentEntry],
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    let Some(payload) = encode_set_equipment(
        entity_id,
        entries,
        &config.item_protocol_ids,
        &config.data_component_protocol_ids,
    )
    .context("cannot encode player equipment")?
    else {
        return Ok(());
    };
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetEquipment,
            payload,
        },
        exit,
    );
    Ok(())
}

fn queue_equipment_except(
    connections: &mut BTreeMap<PlayerUuid, ReplicationConnection>,
    excluded: PlayerUuid,
    entity_id: EntityId,
    entries: &[EquipmentEntry],
    config: &GameReplicationConfig,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    for (uuid, connection) in connections {
        if *uuid != excluded && connection.entities.contains_key(&excluded) {
            queue_player_equipment(connection, entity_id, entries, config, exit)?;
        }
    }
    Ok(())
}

"""
    text = replace_once(text, helper_anchor, helpers + helper_anchor, "equipment helpers")
    text = replace_once(
        text,
        """        GameReplicationConfig {
            entity_protocol_ids: EntityProtocolRegistry::new([("minecraft:player", 148)]).unwrap(),
            ..GameReplicationConfig::default()
        }
""",
        """        GameReplicationConfig {
            entity_protocol_ids: EntityProtocolRegistry::new([("minecraft:player", 148)]).unwrap(),
            item_protocol_ids: ItemProtocolRegistry::new([("minecraft:stone", 1)]).unwrap(),
            ..GameReplicationConfig::default()
        }
""",
        "test entity config items",
    )
    text = replace_once(
        text,
        """    use ferrum_game::{CommandSource, Transform};
""",
        """    use ferrum_game::{CommandSource, HOTBAR_START, ItemStack, Transform};
""",
        "test imports",
    )
    test = r'''

    #[test]
    fn synchronizes_initial_equipment_and_selected_hotbar_changes() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, entity_config()).unwrap();
        let steve = PlayerUuid::new(301);
        let alex = PlayerUuid::new(302);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(256).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(301),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let (alex_reader, alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(302),
            NonZeroUsize::new(128).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(256).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        recv_protocol(
            &steve_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );
        let steve_entity_id = game
            .with_state(|state| state.player(steve).and_then(|player| player.entity_id))
            .unwrap()
            .unwrap();
        let stone = ItemStack::new("minecraft:stone", 1).unwrap();
        game.with_state_mut(|state| {
            state
                .player_mut(steve)
                .unwrap()
                .inventory
                .set_slot(HOTBAR_START, Some(stone.clone()))
                .unwrap();
            Ok(())
        })
        .unwrap();

        service.control().register(alex, alex_reader).unwrap();
        for kind in [
            PacketKind::PlayerInfoUpdate,
            PacketKind::AddEntity,
            PacketKind::SetEntityData,
        ] {
            recv_protocol(&alex_writer, &mut workers, &mut inputs, kind);
        }
        let equipment = recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEquipment,
        );
        let (entity_id, entity_bytes) = read_varint(&equipment);
        assert_eq!(entity_id, steve_entity_id.get() as i32);
        assert_eq!(equipment[entity_bytes] & 0x7f, 0);
        let (count, count_bytes) = read_varint(&equipment[entity_bytes + 1..]);
        assert_eq!(count, 1);
        let item_offset = entity_bytes + 1 + count_bytes;
        assert_eq!(read_varint(&equipment[item_offset..]).0, 1);
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::RotateHead,
        );

        game.connect_player(alex, "Alex", spawn()).unwrap();
        for kind in [
            PacketKind::PlayerInfoUpdate,
            PacketKind::AddEntity,
            PacketKind::SetEntityData,
            PacketKind::RotateHead,
        ] {
            recv_protocol(&steve_writer, &mut workers, &mut inputs, kind);
        }
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { .. }
        ));
        recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::PlayerInfoUpdate,
        );

        game.with_state_mut(|state| {
            state
                .player_mut(steve)
                .unwrap()
                .inventory
                .set_slot(HOTBAR_START + 1, Some(stone.clone()))
                .unwrap();
            Ok(())
        })
        .unwrap();
        game.publish(&[GameEvent::InventorySlotChanged {
            uuid: steve,
            slot: HOTBAR_START + 1,
            stack: Some(stone),
        }])
        .unwrap();
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SetPlayerInventory { slot, .. } if slot == HOTBAR_START + 1
        ));
        assert!(alex_writer.try_recv_output().is_err());

        game.select_hotbar(steve, 1).unwrap();
        let equipment = recv_protocol(
            &alex_writer,
            &mut workers,
            &mut inputs,
            PacketKind::SetEquipment,
        );
        let (entity_id, entity_bytes) = read_varint(&equipment);
        assert_eq!(entity_id, steve_entity_id.get() as i32);
        assert_eq!(equipment[entity_bytes] & 0x7f, 0);
        assert_eq!(read_varint(&equipment[entity_bytes + 1..]).0, 1);
        assert!(steve_writer.try_recv_output().is_err());
        service.shutdown().unwrap();
    }
'''
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("replication tests closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_main(text: str) -> str:
    text = replace_once(
        text,
        """struct RuntimeRegistryData {
    configuration_payloads: Vec<Vec<u8>>,
    entity_protocol_ids: EntityProtocolRegistry,
}
""",
        """struct RuntimeRegistryData {
    configuration_payloads: Vec<Vec<u8>>,
    entity_protocol_ids: EntityProtocolRegistry,
    item_protocol_ids: ItemProtocolRegistry,
    data_component_protocol_ids: DataComponentProtocolRegistry,
}
""",
        "runtime registry bundle fields",
    )
    default_literal = """                entity_protocol_ids: EntityProtocolRegistry::default(),
"""
    if text.count(default_literal) != 2:
        raise SystemExit(f"runtime default literals: expected 2, found {text.count(default_literal)}")
    text = text.replace(
        default_literal,
        """                entity_protocol_ids: EntityProtocolRegistry::default(),
                item_protocol_ids: ItemProtocolRegistry::default(),
                data_component_protocol_ids: DataComponentProtocolRegistry::default(),
""",
    )
    text = replace_once(
        text,
        """        let RuntimeRegistryData {
            configuration_payloads,
            entity_protocol_ids,
        } = registries;
""",
        """        let RuntimeRegistryData {
            configuration_payloads,
            entity_protocol_ids,
            item_protocol_ids,
            data_component_protocol_ids,
        } = registries;
""",
        "runtime registry destructure",
    )
    text = replace_once(
        text,
        """            GameReplicationConfig {
                entity_protocol_ids,
                ..GameReplicationConfig::default()
            },
""",
        """            GameReplicationConfig {
                entity_protocol_ids,
                item_protocol_ids,
                data_component_protocol_ids,
                ..GameReplicationConfig::default()
            },
""",
        "replication registry config",
    )
    text = replace_once(
        text,
        """    config.item_protocol_ids = item_protocol_ids;
    config.data_component_protocol_ids = data_component_protocol_ids;
""",
        """    config.item_protocol_ids = item_protocol_ids.clone();
    config.data_component_protocol_ids = data_component_protocol_ids.clone();
""",
        "retain connection item registries",
    )
    text = replace_once(
        text,
        """        RuntimeRegistryData {
            configuration_payloads: registry_payloads,
            entity_protocol_ids,
        },
""",
        """        RuntimeRegistryData {
            configuration_payloads: registry_payloads,
            entity_protocol_ids,
            item_protocol_ids,
            data_component_protocol_ids,
        },
""",
        "production runtime registry literal",
    )
    return text


def patch_roadmap(text: str) -> str:
    text = replace_once(
        text,
        """Dedicated Play reader/writer queues, a shared 20 TPS runtime, authoritative gameplay persistence, inventory replication, join/leave messages, and offline-mode chat are implemented. Non-player entity gameplay, full multi-client player entity tracking, and complete Vanilla systems remain incomplete.
""",
        """Dedicated Play reader/writer queues, a shared 20 TPS runtime, authoritative gameplay persistence, inventory replication, join/leave messages, offline-mode chat, multi-client player spawning, relative/absolute movement, tab-list lifecycle, metadata placeholders, head rotation, and equipment synchronization are implemented. Non-player entity gameplay, visibility/range-based entity tracking, and complete Vanilla systems remain incomplete.
""",
        "roadmap current implementation",
    )
    return replace_once(
        text,
        """- Broadcast player state to other connected clients.
""",
        """- Add visibility/range-based tracking instead of globally broadcasting every connected player.
""",
        "roadmap movement remaining",
    )


patch("crates/ferrum-play/src/inventory.rs", patch_inventory)
patch("crates/ferrum-play/src/lib.rs", patch_play_lib)
patch("crates/ferrum-server/src/game_replication.rs", patch_replication)
patch("crates/ferrum-server/src/main.rs", patch_main)
patch("docs/SERVER_ROADMAP.md", patch_roadmap)
