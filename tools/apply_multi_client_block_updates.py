from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Shared-world subscriber state and bounded coalescing queues.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "use std::{\n    io::{Read, Write},\n",
    "use std::{\n"
    "    collections::{BTreeMap, VecDeque},\n"
    "    io::{Read, Write},\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "const LOCAL_WORLD_EVENTS_PER_TICK: usize = 16;\n",
    "const LOCAL_WORLD_EVENTS_PER_TICK: usize = 16;\n"
    "const MAX_PENDING_WORLD_UPDATES_PER_CONNECTION: usize = 256;\n"
    "const MAX_WORLD_UPDATES_PER_DRAIN: usize = 64;\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "struct SharedWorldInner {\n"
    "    runtime: LocalWorldRuntime,\n"
    "    tick: Tick,\n"
    "}\n",
    "struct SharedWorldInner {\n"
    "    runtime: LocalWorldRuntime,\n"
    "    tick: Tick,\n"
    "    subscribers: BTreeMap<ConnectionId, PendingWorldUpdates>,\n"
    "}\n\n"
    "#[derive(Debug, Default)]\n"
    "struct PendingWorldUpdates {\n"
    "    updates: VecDeque<AppliedWorldEvent>,\n"
    "}\n\n"
    "impl PendingWorldUpdates {\n"
    "    fn push(&mut self, event: AppliedWorldEvent) {\n"
    "        let position = applied_world_event_position(&event);\n"
    "        if let Some(existing) = self\n"
    "            .updates\n"
    "            .iter_mut()\n"
    "            .find(|existing| applied_world_event_position(existing) == position)\n"
    "        {\n"
    "            *existing = event;\n"
    "            return;\n"
    "        }\n\n"
    "        if self.updates.len() == MAX_PENDING_WORLD_UPDATES_PER_CONNECTION {\n"
    "            self.updates.pop_front();\n"
    "        }\n"
    "        self.updates.push_back(event);\n"
    "    }\n\n"
    "    fn drain(&mut self, limit: usize) -> Vec<AppliedWorldEvent> {\n"
    "        let count = limit.min(self.updates.len());\n"
    "        self.updates.drain(..count).collect()\n"
    "    }\n\n"
    "    #[cfg(test)]\n"
    "    fn len(&self) -> usize {\n"
    "        self.updates.len()\n"
    "    }\n"
    "}\n\n"
    "#[derive(Debug)]\n"
    "pub(super) struct SharedWorldSubscription<'a> {\n"
    "    world: &'a SharedWorld,\n"
    "    connection: ConnectionId,\n"
    "}\n\n"
    "impl Drop for SharedWorldSubscription<'_> {\n"
    "    fn drop(&mut self) {\n"
    "        self.world.unsubscribe(self.connection);\n"
    "    }\n"
    "}\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "                runtime: new_local_world_runtime(center)?,\n"
    "                tick: Tick::ZERO,\n",
    "                runtime: new_local_world_runtime(center)?,\n"
    "                tick: Tick::ZERO,\n"
    "                subscribers: BTreeMap::new(),\n",
)

# Register before the initial chunk snapshot, close the join-time race, and
# remove the queue automatically when the Play session ends.
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    let chunk = world.shared_world.chunk_snapshot(ChunkPos {\n",
    "    let _world_subscription = world.shared_world.subscribe(world.connection)?;\n"
    "    let chunk = world.shared_world.chunk_snapshot(ChunkPos {\n",
)

# Subscription API, broadcast fan-out, and bounded draining.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {\n",
    "    pub(super) fn subscribe(\n"
    "        &self,\n"
    "        connection: ConnectionId,\n"
    "    ) -> Result<SharedWorldSubscription<'_>> {\n"
    "        let mut inner = self\n"
    "            .inner\n"
    "            .lock()\n"
    "            .map_err(|_| anyhow::anyhow!(\"shared world lock poisoned\"))?;\n"
    "        if inner.subscribers.contains_key(&connection) {\n"
    "            bail!(\"connection {} is already subscribed to the shared world\", connection.get());\n"
    "        }\n"
    "        inner\n"
    "            .subscribers\n"
    "            .insert(connection, PendingWorldUpdates::default());\n"
    "        Ok(SharedWorldSubscription {\n"
    "            world: self,\n"
    "            connection,\n"
    "        })\n"
    "    }\n\n"
    "    fn unsubscribe(&self, connection: ConnectionId) {\n"
    "        if let Ok(mut inner) = self.inner.lock() {\n"
    "            inner.subscribers.remove(&connection);\n"
    "        }\n"
    "    }\n\n"
    "    fn drain_updates(\n"
    "        &self,\n"
    "        connection: ConnectionId,\n"
    "        limit: usize,\n"
    "    ) -> Result<Vec<AppliedWorldEvent>> {\n"
    "        let mut inner = self\n"
    "            .inner\n"
    "            .lock()\n"
    "            .map_err(|_| anyhow::anyhow!(\"shared world lock poisoned\"))?;\n"
    "        let pending = inner.subscribers.get_mut(&connection).with_context(|| {\n"
    "            format!(\"connection {} is not subscribed to the shared world\", connection.get())\n"
    "        })?;\n"
    "        Ok(pending.drain(limit))\n"
    "    }\n\n"
    "    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {\n",
)
old_apply = '''        inner.tick = next_tick(inner.tick)?;
        let tick = inner.tick;
        apply_world_event(&mut inner.runtime, connection, tick, event)
'''
new_apply = '''        inner.tick = next_tick(inner.tick)?;
        let tick = inner.tick;
        let applied = apply_world_event(&mut inner.runtime, connection, tick, event)?;
        for applied_event in applied.iter().copied() {
            for (subscriber, pending) in &mut inner.subscribers {
                if *subscriber != connection {
                    pending.push(applied_event);
                }
            }
        }
        Ok(applied)
'''
replace_once("crates/ferrum-server/src/play_runtime.rs", old_apply, new_apply)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    #[cfg(test)]\n    fn world_block(&self, position: BlockPos) -> Result<BlockStateId> {\n",
    "    #[cfg(test)]\n"
    "    fn subscriber_count(&self) -> usize {\n"
    "        self.inner\n"
    "            .lock()\n"
    "            .expect(\"shared world lock must not be poisoned in tests\")\n"
    "            .subscribers\n"
    "            .len()\n"
    "    }\n\n"
    "    #[cfg(test)]\n"
    "    fn pending_update_count(&self, connection: ConnectionId) -> usize {\n"
    "        self.inner\n"
    "            .lock()\n"
    "            .expect(\"shared world lock must not be poisoned in tests\")\n"
    "            .subscribers\n"
    "            .get(&connection)\n"
    "            .map_or(0, PendingWorldUpdates::len)\n"
    "    }\n\n"
    "    #[cfg(test)]\n"
    "    fn world_block(&self, position: BlockPos) -> Result<BlockStateId> {\n",
)

# Send queued peer changes after every normal Play packet. Vanilla sends Client
# Tick End continuously, so no separate writer thread is required for this
# bounded first milestone.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "            }\n\n            if keep_alive_acknowledged && ticks_since_request >= tick_interval {\n",
    "            }\n\n"
    "            let pending_updates =\n"
    "                shared_world.drain_updates(connection, MAX_WORLD_UPDATES_PER_DRAIN)?;\n"
    "            send_world_updates(writer, profile, &pending_updates)?;\n\n"
    "            if keep_alive_acknowledged && ticks_since_request >= tick_interval {\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn send_world_updates<W: Write>(\n"
    "    writer: &mut W,\n"
    "    profile: &ProtocolProfile,\n"
    "    applied_events: &[AppliedWorldEvent],\n"
    ") -> Result<()> {\n"
    "    if profile.packets().id(PacketKind::BlockUpdate).is_none() {\n",
    "fn send_world_updates<W: Write>(\n"
    "    writer: &mut W,\n"
    "    profile: &ProtocolProfile,\n"
    "    applied_events: &[AppliedWorldEvent],\n"
    ") -> Result<()> {\n"
    "    if applied_events.is_empty() {\n"
    "        return Ok(());\n"
    "    }\n"
    "    if profile.packets().id(PacketKind::BlockUpdate).is_none() {\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn block_position_from_world(position: BlockPos) -> BlockPosition {\n",
    "fn applied_world_event_position(event: &AppliedWorldEvent) -> BlockPos {\n"
    "    match event {\n"
    "        AppliedWorldEvent::BlockMutation(mutation) => mutation.position,\n"
    "    }\n"
    "}\n\n"
    "fn block_position_from_world(position: BlockPos) -> BlockPosition {\n",
)

# Tests: peer-only delivery, same-position coalescing, bounded queues, and
# automatic unsubscribe through the guard.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    #[test]\n    fn shared_world_chunk_snapshots_include_authoritative_mutations() {\n",
    "    #[test]\n"
    "    fn shared_world_broadcasts_mutations_to_other_subscribers_only() {\n"
    "        let world = SharedWorld::static_flat();\n"
    "        let first = ConnectionId::new(1);\n"
    "        let second = ConnectionId::new(2);\n"
    "        let first_subscription = world.subscribe(first).unwrap();\n"
    "        let second_subscription = world.subscribe(second).unwrap();\n"
    "        assert_eq!(world.subscriber_count(), 2);\n\n"
    "        let position = BlockPos { x: 2, y: 65, z: 3 };\n"
    "        let state = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);\n"
    "        let applied = world\n"
    "            .apply_event(\n"
    "                first,\n"
    "                WorldEvent::BlockMutation(BlockMutation { position, state }),\n"
    "            )\n"
    "            .unwrap();\n\n"
    "        assert!(world.drain_updates(first, usize::MAX).unwrap().is_empty());\n"
    "        assert_eq!(world.drain_updates(second, usize::MAX).unwrap(), applied);\n\n"
    "        drop(second_subscription);\n"
    "        assert_eq!(world.subscriber_count(), 1);\n"
    "        drop(first_subscription);\n"
    "        assert_eq!(world.subscriber_count(), 0);\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn shared_world_coalesces_repeated_updates_for_the_same_block() {\n"
    "        let world = SharedWorld::static_flat();\n"
    "        let source = ConnectionId::new(1);\n"
    "        let receiver = ConnectionId::new(2);\n"
    "        let _receiver_subscription = world.subscribe(receiver).unwrap();\n"
    "        let position = BlockPos { x: 4, y: 65, z: 4 };\n"
    "        let stone = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);\n"
    "        let air = BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID);\n\n"
    "        world\n"
    "            .apply_event(\n"
    "                source,\n"
    "                WorldEvent::BlockMutation(BlockMutation { position, state: stone }),\n"
    "            )\n"
    "            .unwrap();\n"
    "        let latest = world\n"
    "            .apply_event(\n"
    "                source,\n"
    "                WorldEvent::BlockMutation(BlockMutation { position, state: air }),\n"
    "            )\n"
    "            .unwrap();\n\n"
    "        assert_eq!(world.pending_update_count(receiver), 1);\n"
    "        assert_eq!(world.drain_updates(receiver, 1).unwrap(), latest);\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn shared_world_bounds_pending_peer_updates() {\n"
    "        let world = SharedWorld::static_flat();\n"
    "        let source = ConnectionId::new(1);\n"
    "        let receiver = ConnectionId::new(2);\n"
    "        let _receiver_subscription = world.subscribe(receiver).unwrap();\n"
    "        let stone = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);\n\n"
    "        for index in 0..MAX_PENDING_WORLD_UPDATES_PER_CONNECTION + 5 {\n"
    "            let position = BlockPos {\n"
    "                x: -16 + i32::try_from(index % 48).unwrap(),\n"
    "                y: 65,\n"
    "                z: -16 + i32::try_from(index / 48).unwrap(),\n"
    "            };\n"
    "            world\n"
    "                .apply_event(\n"
    "                    source,\n"
    "                    WorldEvent::BlockMutation(BlockMutation { position, state: stone }),\n"
    "                )\n"
    "                .unwrap();\n"
    "        }\n\n"
    "        assert_eq!(\n"
    "            world.pending_update_count(receiver),\n"
    "            MAX_PENDING_WORLD_UPDATES_PER_CONNECTION\n"
    "        );\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn shared_world_chunk_snapshots_include_authoritative_mutations() {\n",
)

# Documentation.
replace_once(
    "README.md",
    "- Clientbound Block Update and Block Changed Ack responses for accepted interactions\n",
    "- Clientbound Block Update and Block Changed Ack responses for accepted interactions\n"
    "- Bounded per-connection peer update queues with same-position coalescing\n",
)
replace_once(
    "README.md",
    "- Multi-client broadcasting of block breaking and placement results\n",
    "- Dedicated outbound writer workers; peer updates currently drain on normal incoming Play traffic\n",
)
replace_once(
    "docs/SERVER_ROADMAP.md",
    "- Serialize bootstrap and dynamically streamed chunks from shared-world snapshots so accepted mutations survive new connections and chunk re-entry.\n",
    "- Serialize bootstrap and dynamically streamed chunks from shared-world snapshots so accepted mutations survive new connections and chunk re-entry.\n"
    "- Broadcast accepted block mutations through bounded per-connection queues with same-position coalescing.\n",
)
replace_once(
    "docs/SERVER_ROADMAP.md",
    "- Broadcast accepted block mutations back to affected clients.\n",
    "- Move outbound block updates from client-traffic-driven draining to dedicated non-blocking writer workers.\n",
)
