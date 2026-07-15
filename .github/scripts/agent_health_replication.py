from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def patch(path: str, transform) -> None:
    file = Path(path)
    text = file.read_text()
    updated = transform(text)
    if updated == text:
        raise SystemExit(f"no changes made to {path}")
    file.write_text(updated)


def patch_protocol_lib(text: str) -> str:
    text = replace_once(
        text,
        """    SetEquipment,
    PlayerInfoUpdate,
""",
        """    SetEquipment,
    SetHealth,
    PlayerInfoUpdate,
""",
        "PacketKind variant",
    )
    text = replace_once(
        text,
        """        Self::SetEquipment,
        Self::PlayerInfoUpdate,
""",
        """        Self::SetEquipment,
        Self::SetHealth,
        Self::PlayerInfoUpdate,
""",
        "PacketKind ALL",
    )
    text = replace_once(
        text,
        """            | Self::SetEquipment
            | Self::PlayerInfoUpdate
""",
        """            | Self::SetEquipment
            | Self::SetHealth
            | Self::PlayerInfoUpdate
""",
        "PacketKind phase",
    )
    return text


def patch_packet_catalog(text: str) -> str:
    text = replace_once(
        text,
        """        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_equipment") => {
            Some(PacketKind::SetEquipment)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "player_info_update") => {
""",
        """        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_equipment") => {
            Some(PacketKind::SetEquipment)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "set_health") => {
            Some(PacketKind::SetHealth)
        }
        (ProtocolPhase::Play, PacketDirection::Clientbound, "player_info_update") => {
""",
        "set health packet mapping",
    )
    text = replace_once(
        text,
        """        PacketKind::SetEquipment => "minecraft:set_equipment",
        PacketKind::PlayerInfoUpdate => "minecraft:player_info_update",
""",
        """        PacketKind::SetEquipment => "minecraft:set_equipment",
        PacketKind::SetHealth => "minecraft:set_health",
        PacketKind::PlayerInfoUpdate => "minecraft:player_info_update",
""",
        "set health canonical name",
    )
    test = """

    #[test]
    fn recognizes_set_health_as_optional_typed_packet() {
        assert_eq!(
            known_packet_kind(
                ProtocolPhase::Play,
                PacketDirection::Clientbound,
                "set_health",
            ),
            Some(PacketKind::SetHealth)
        );
        assert_eq!(
            canonical_packet_name(PacketKind::SetHealth),
            "minecraft:set_health"
        );
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("packet catalog test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_version(text: str) -> str:
    text = replace_once(
        text,
        """        (PacketKind::SystemChat, 0x79),
        (PacketKind::AcceptTeleportation, 0x00),
""",
        """        (PacketKind::SystemChat, 0x79),
        (PacketKind::SetHealth, 0x68),
        (PacketKind::AcceptTeleportation, 0x00),
""",
        "26.1.2 Set Health packet ID",
    )
    text = replace_once(
        text,
        """        assert_eq!(packets.require(PacketKind::UpdateTags).unwrap(), 0x0d);
        assert_eq!(packets.require(PacketKind::ChunkBatchStart).unwrap(), 0x0c);
""",
        """        assert_eq!(packets.require(PacketKind::UpdateTags).unwrap(), 0x0d);
        assert_eq!(packets.require(PacketKind::SetHealth).unwrap(), 0x68);
        assert_eq!(packets.require(PacketKind::ChunkBatchStart).unwrap(), 0x0c);
""",
        "26.1.2 Set Health test",
    )
    return text


def patch_play_lib(text: str) -> str:
    text = replace_once(
        text,
        """mod entity;
mod inventory;
mod movement;
""",
        """mod entity;
mod health;
mod inventory;
mod movement;
""",
        "health module declaration",
    )
    return replace_once(
        text,
        """pub use inventory::{
""",
        """pub use health::{HealthEncodeError, encode_set_health};
pub use inventory::{
""",
        "health exports",
    )


def patch_game_state(text: str) -> str:
    text = replace_once(
        text,
        """    EntityId, EntityStore, EntityType, EntityUuid, GameMode, InventoryError, ItemStack,
    PlayerError, PlayerState, PlayerUuid, Transform,
""",
        """    EntityId, EntityStore, EntityType, EntityUuid, GameMode, InventoryError, ItemStack,
    PlayerError, PlayerState, PlayerUuid, Transform, Vitals,
""",
        "state Vitals import",
    )
    text = replace_once(
        text,
        """    SelectedHotbarChanged {
        uuid: PlayerUuid,
        previous: u8,
        current: u8,
    },
    PlayerKilled {
""",
        """    SelectedHotbarChanged {
        uuid: PlayerUuid,
        previous: u8,
        current: u8,
    },
    PlayerDamaged {
        uuid: PlayerUuid,
        entity_id: EntityId,
        amount: f32,
        previous: Vitals,
        current: Vitals,
    },
    PlayerVitalsChanged {
        uuid: PlayerUuid,
        vitals: Vitals,
    },
    PlayerKilled {
""",
        "vitals events",
    )
    old_kill = """    pub fn kill_player(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {
        let keep_inventory = matches!(
            self.game_rules.get("keepInventory"),
            Some(GameRuleValue::Boolean(true))
        );
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        player.vitals.health = 0.0;
        let mut events = vec![GameEvent::PlayerKilled { uuid }];
        if !keep_inventory {
            let before = player.inventory.slots().to_vec();
            let stacks = player.inventory.drain();
            events.extend(slot_diff_events(uuid, &before, player.inventory.slots()));
            if !stacks.is_empty() {
                events.push(GameEvent::ItemsDropped { uuid, stacks });
            }
            events.push(GameEvent::ContainerContentChanged {
                uuid,
                snapshot: player.inventory_session.snapshot(&player.inventory),
            });
        }
        Ok(events)
    }
"""
    new_kill = """    pub fn damage_player(
        &mut self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let entity_id = self.connected_entity_id(uuid)?;
        let (previous, current) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if player.abilities.invulnerable || player.vitals.is_dead() {
                return Ok(Vec::new());
            }
            let previous = player.vitals;
            player.vitals.damage(amount)?;
            (previous, player.vitals)
        };
        if previous == current {
            return Ok(Vec::new());
        }
        let mut events = vec![
            GameEvent::PlayerDamaged {
                uuid,
                entity_id,
                amount,
                previous,
                current,
            },
            GameEvent::PlayerVitalsChanged {
                uuid,
                vitals: current,
            },
        ];
        if !previous.is_dead() && current.is_dead() {
            events.extend(self.finish_player_death(uuid)?);
        }
        Ok(events)
    }

    pub fn heal_player(
        &mut self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        self.connected_entity_id(uuid)?;
        let (previous, current) = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if player.vitals.is_dead() {
                return Err(GameStateError::PlayerDead { uuid });
            }
            let previous = player.vitals;
            player.vitals.heal(amount)?;
            (previous, player.vitals)
        };
        if previous == current {
            return Ok(Vec::new());
        }
        Ok(vec![GameEvent::PlayerVitalsChanged {
            uuid,
            vitals: current,
        }])
    }

    pub fn kill_player(&mut self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameStateError> {
        self.connected_entity_id(uuid)?;
        let current = {
            let player = self
                .players
                .get_mut(&uuid)
                .ok_or(GameStateError::UnknownPlayer { uuid })?;
            if player.vitals.is_dead() {
                return Ok(Vec::new());
            }
            player.vitals.health = 0.0;
            player.vitals
        };
        let mut events = vec![GameEvent::PlayerVitalsChanged {
            uuid,
            vitals: current,
        }];
        events.extend(self.finish_player_death(uuid)?);
        Ok(events)
    }

    fn finish_player_death(
        &mut self,
        uuid: PlayerUuid,
    ) -> Result<Vec<GameEvent>, GameStateError> {
        let keep_inventory = matches!(
            self.game_rules.get("keepInventory"),
            Some(GameRuleValue::Boolean(true))
        );
        let player = self
            .players
            .get_mut(&uuid)
            .ok_or(GameStateError::UnknownPlayer { uuid })?;
        let mut events = vec![GameEvent::PlayerKilled { uuid }];
        if !keep_inventory {
            let before = player.inventory.slots().to_vec();
            let stacks = player.inventory.drain();
            events.extend(slot_diff_events(uuid, &before, player.inventory.slots()));
            if !stacks.is_empty() {
                events.push(GameEvent::ItemsDropped { uuid, stacks });
            }
            events.push(GameEvent::ContainerContentChanged {
                uuid,
                snapshot: player.inventory_session.snapshot(&player.inventory),
            });
        }
        Ok(events)
    }
"""
    text = replace_once(text, old_kill, new_kill, "authoritative health methods")
    text = replace_once(
        text,
        """    #[error("connected player {uuid:?} has no entity")]
    PlayerMissingEntity { uuid: PlayerUuid },
""",
        """    #[error("connected player {uuid:?} has no entity")]
    PlayerMissingEntity { uuid: PlayerUuid },
    #[error("player {uuid:?} is dead and must respawn before healing")]
    PlayerDead { uuid: PlayerUuid },
""",
        "dead player error",
    )
    test = """

    #[test]
    fn damage_heal_and_death_publish_authoritative_vitals() {
        let uuid = PlayerUuid::new(30);
        let mut state = GameState::default();
        state.connect_player(uuid, "Steve", spawn()).unwrap();
        state.player_mut(uuid).unwrap().vitals.absorption = 2.0;

        let damaged = state.damage_player(uuid, 5.0).unwrap();
        assert!(matches!(
            damaged.as_slice(),
            [
                GameEvent::PlayerDamaged {
                    amount,
                    previous,
                    current,
                    ..
                },
                GameEvent::PlayerVitalsChanged { vitals, .. }
            ] if *amount == 5.0
                && previous.health == 20.0
                && previous.absorption == 2.0
                && current.health == 17.0
                && current.absorption == 0.0
                && *vitals == *current
        ));

        let healed = state.heal_player(uuid, 2.0).unwrap();
        assert!(matches!(
            healed.as_slice(),
            [GameEvent::PlayerVitalsChanged { vitals, .. }] if vitals.health == 19.0
        ));

        let fatal = state.damage_player(uuid, 100.0).unwrap();
        assert!(matches!(fatal[0], GameEvent::PlayerDamaged { .. }));
        assert!(matches!(
            fatal[1],
            GameEvent::PlayerVitalsChanged { vitals, .. } if vitals.health == 0.0
        ));
        assert!(matches!(fatal[2], GameEvent::PlayerKilled { .. }));
        assert!(matches!(
            state.heal_player(uuid, 1.0),
            Err(GameStateError::PlayerDead { .. })
        ));
    }

    #[test]
    fn invulnerable_players_ignore_damage() {
        let uuid = PlayerUuid::new(31);
        let mut state = GameState::default();
        state.connect_player(uuid, "Alex", spawn()).unwrap();
        state.set_game_mode(uuid, GameMode::Creative).unwrap();
        assert!(state.damage_player(uuid, 20.0).unwrap().is_empty());
        assert_eq!(state.player(uuid).unwrap().vitals.health, 20.0);
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("state test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_runtime(text: str) -> str:
    text = replace_once(
        text,
        """    pub fn select_hotbar(
        &self,
        uuid: PlayerUuid,
        selected_hotbar: u8,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.select_hotbar(uuid, selected_hotbar)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn click_container(
""",
        """    pub fn select_hotbar(
        &self,
        uuid: PlayerUuid,
        selected_hotbar: u8,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.select_hotbar(uuid, selected_hotbar)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn damage_player(
        &self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.damage_player(uuid, amount)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn heal_player(
        &self,
        uuid: PlayerUuid,
        amount: f32,
    ) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.heal_player(uuid, amount)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn kill_player(&self, uuid: PlayerUuid) -> Result<Vec<GameEvent>, GameRuntimeError> {
        let events = self.write()?.kill_player(uuid)?;
        self.publish(&events)?;
        Ok(events)
    }

    pub fn click_container(
""",
        "runtime health wrappers",
    )
    test = """

    #[test]
    fn publishes_damage_vitals_and_death_events() {
        let runtime = SharedGameRuntime::vanilla_overworld();
        let subscription = runtime.subscribe(NonZeroUsize::new(8).unwrap()).unwrap();
        let uuid = PlayerUuid::new(40);
        runtime.connect_player(uuid, "Steve", spawn()).unwrap();
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerConnected { .. }
        ));
        runtime.damage_player(uuid, 20.0).unwrap();
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerDamaged { .. }
        ));
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerVitalsChanged { vitals, .. } if vitals.health == 0.0
        ));
        assert!(matches!(
            subscription.try_recv().unwrap(),
            GameEvent::PlayerKilled { .. }
        ));
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("runtime test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_replication(text: str) -> str:
    text = replace_once(
        text,
        """    PlayerUuid, Transform, Velocity,
""",
        """    PlayerUuid, Transform, Velocity, Vitals,
""",
        "replication Vitals import",
    )
    text = replace_once(
        text,
        """    encode_player_info_update, encode_remove_entities, encode_rotate_head, encode_set_equipment,
    encode_teleport_entity,
""",
        """    encode_player_info_update, encode_remove_entities, encode_rotate_head, encode_set_equipment,
    encode_set_health, encode_teleport_entity,
""",
        "replication health encoder import",
    )
    text = replace_once(
        text,
        """                if let Some(connection) = connections.get_mut(&uuid) {
                    queue_player_info_update(connection, &snapshot, exit)?;
                }
                for (target, connection) in connections.iter_mut() {
""",
        """                if let Some(connection) = connections.get_mut(&uuid) {
                    queue_player_info_update(connection, &snapshot, exit)?;
                }
                for (target, connection) in connections.iter_mut() {
""",
        "player connected info anchor",
    )
    # Queue health independently of entity palette, immediately after optional self tab-list state.
    old_connected_tail = """            }
            broadcast_except(
                connections,
                uuid,
                PlayOutput::SystemChat {
"""
    new_connected_tail = """            }
            if let Some(vitals) = runtime
                .with_state(|state| state.player(uuid).map(|player| player.vitals))
                .context("cannot read connected player vitals")?
            {
                if let Some(connection) = connections.get_mut(&uuid) {
                    queue_set_health(connection, vitals, exit)?;
                }
            }
            broadcast_except(
                connections,
                uuid,
                PlayOutput::SystemChat {
"""
    text = replace_once(text, old_connected_tail, new_connected_tail, "initial health replication")
    text = replace_once(
        text,
        """        GameEvent::PlayerKilled { uuid } => {
            target_chat(connections, uuid, "You died".to_owned(), false, exit)
        }
""",
        """        GameEvent::PlayerDamaged { .. } => {}
        GameEvent::PlayerVitalsChanged { uuid, vitals } => {
            if let Some(connection) = connections.get_mut(&uuid) {
                queue_set_health(connection, vitals, exit)?;
            }
        }
        GameEvent::PlayerKilled { uuid } => {
            target_chat(connections, uuid, "You died".to_owned(), false, exit)
        }
""",
        "health event dispatch",
    )
    helper = """fn queue_set_health(
    connection: &mut ReplicationConnection,
    vitals: Vitals,
    exit: &mut GameReplicationExit,
) -> Result<()> {
    connection.queue(
        PlayOutput::ProtocolPacket {
            kind: PacketKind::SetHealth,
            payload: encode_set_health(vitals).context("cannot encode player health")?,
        },
        exit,
    );
    Ok(())
}

"""
    text = replace_once(text, "fn target_chat(\n", helper + "fn target_chat(\n", "health queue helper")
    old_recv = """    fn recv_output(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) -> PlayOutput {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            ingest(workers, inputs);
            match writer.try_recv_output() {
                Ok(output) => return output,
                Err(ferrum_runtime::WorkerReceiveError::Empty) => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "replication output timeout"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
                Err(ferrum_runtime::WorkerReceiveError::RuntimeDisconnected) => {
                    panic!("replication runtime disconnected")
                }
            }
        }
    }
"""
    new_recv = """    fn recv_raw_output(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) -> PlayOutput {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            ingest(workers, inputs);
            match writer.try_recv_output() {
                Ok(output) => return output,
                Err(ferrum_runtime::WorkerReceiveError::Empty) => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "replication output timeout"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
                Err(ferrum_runtime::WorkerReceiveError::RuntimeDisconnected) => {
                    panic!("replication runtime disconnected")
                }
            }
        }
    }

    fn recv_output(
        writer: &crate::play_connection::PlayWriterEndpoint,
        workers: &mut ferrum_runtime::WorkerRuntime<
            crate::authoritative_runtime::PlayInput,
            PlayOutput,
        >,
        inputs: &mut BoundedInputQueue<crate::authoritative_runtime::PlayInput>,
    ) -> PlayOutput {
        loop {
            let output = recv_raw_output(writer, workers, inputs);
            if matches!(
                output,
                PlayOutput::ProtocolPacket {
                    kind: PacketKind::SetHealth,
                    ..
                }
            ) {
                continue;
            }
            return output;
        }
    }
"""
    text = replace_once(text, old_recv, new_recv, "test health filtering helper")
    test = """

    #[test]
    fn synchronizes_initial_and_changed_health_only_to_the_subject() {
        let game = SharedGameRuntime::vanilla_overworld();
        let service = spawn_game_replication(&game, GameReplicationConfig::default()).unwrap();
        let steve = PlayerUuid::new(401);
        let alex = PlayerUuid::new(402);
        let (connector, mut workers) = worker_channel(NonZeroUsize::new(128).unwrap());
        let (steve_reader, steve_writer) = register_play_connection(
            &connector,
            ConnectionId::new(401),
            NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
        let (alex_reader, alex_writer) = register_play_connection(
            &connector,
            ConnectionId::new(402),
            NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
        let mut inputs = BoundedInputQueue::try_new(128).unwrap();
        ingest(&mut workers, &mut inputs);

        service.control().register(steve, steve_reader).unwrap();
        game.connect_player(steve, "Steve", spawn()).unwrap();
        assert_eq!(
            recv_raw_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload: vec![0x41, 0xa0, 0, 0, 0x14, 0x40, 0xa0, 0, 0],
            }
        );

        service.control().register(alex, alex_reader).unwrap();
        game.connect_player(alex, "Alex", spawn()).unwrap();
        assert!(matches!(
            recv_raw_output(&alex_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                ..
            }
        ));
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, .. } if message == "Alex joined the game"
        ));

        game.damage_player(steve, 4.0).unwrap();
        match recv_raw_output(&steve_writer, &mut workers, &mut inputs) {
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload,
            } => {
                assert_eq!(f32::from_be_bytes(payload[0..4].try_into().unwrap()), 16.0);
                assert_eq!(payload[4], 20);
                assert_eq!(f32::from_be_bytes(payload[5..9].try_into().unwrap()), 5.0);
            }
            output => panic!("expected health packet, got {output:?}"),
        }
        assert!(alex_writer.try_recv_output().is_err());

        game.damage_player(steve, 100.0).unwrap();
        assert!(matches!(
            recv_raw_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::ProtocolPacket {
                kind: PacketKind::SetHealth,
                payload,
            } if f32::from_be_bytes(payload[0..4].try_into().unwrap()) == 0.0
        ));
        assert!(matches!(
            recv_output(&steve_writer, &mut workers, &mut inputs),
            PlayOutput::SystemChat { message, overlay: false } if message == "You died"
        ));
        service.shutdown().unwrap();
    }
"""
    closing = text.rfind("\n}")
    if closing < 0:
        raise SystemExit("replication test module closing brace not found")
    return text[:closing] + test + text[closing:]


def patch_roadmap(text: str) -> str:
    return replace_once(
        text,
        """Dedicated Play reader/writer queues, a shared 20 TPS runtime, authoritative gameplay persistence, inventory replication, join/leave messages, offline-mode chat, multi-client player spawning, relative/absolute movement, tab-list lifecycle, metadata placeholders, head rotation, and equipment synchronization are implemented. Non-player entity gameplay, visibility/range-based entity tracking, and complete Vanilla systems remain incomplete.
""",
        """Dedicated Play reader/writer queues, a shared 20 TPS runtime, authoritative gameplay persistence, inventory replication, join/leave messages, offline-mode chat, multi-client player spawning, relative/absolute movement, tab-list lifecycle, metadata placeholders, head rotation, equipment synchronization, authoritative damage/healing state, and subject-only Set Health replication are implemented. Damage-source animation, combat death packets, respawn flow, non-player entity gameplay, visibility/range-based entity tracking, and complete Vanilla systems remain incomplete.
""",
        "roadmap health status",
    )


patch("crates/ferrum-protocol/src/lib.rs", patch_protocol_lib)
patch("crates/ferrum-protocol/src/packet_catalog.rs", patch_packet_catalog)
patch("crates/ferrum-version-26-1-2/src/lib.rs", patch_version)
patch("crates/ferrum-play/src/lib.rs", patch_play_lib)
patch("crates/ferrum-game/src/state.rs", patch_game_state)
patch("crates/ferrum-server/src/game_runtime.rs", patch_runtime)
patch("crates/ferrum-server/src/game_replication.rs", patch_replication)
patch("docs/SERVER_ROADMAP.md", patch_roadmap)
