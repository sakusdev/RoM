from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:160]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


# Expose immutable chunk snapshots from the shared authoritative store.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    fn apply_event(\n"
    "        &self,\n"
    "        connection: ConnectionId,\n"
    "        event: WorldEvent,\n"
    "    ) -> Result<Vec<AppliedWorldEvent>> {\n",
    "    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {\n"
    "        let inner = self\n"
    "            .inner\n"
    "            .lock()\n"
    "            .map_err(|_| anyhow::anyhow!(\"shared world lock poisoned\"))?;\n"
    "        inner\n"
    "            .runtime\n"
    "            .state()\n"
    "            .chunk(pos)\n"
    "            .cloned()\n"
    "            .with_context(|| format!(\"shared world is missing chunk ({}, {})\", pos.x, pos.z))\n"
    "    }\n\n"
    "    fn apply_event(\n"
    "        &self,\n"
    "        connection: ConnectionId,\n"
    "        event: WorldEvent,\n"
    "    ) -> Result<Vec<AppliedWorldEvent>> {\n",
)

# Every dynamic chunk packet must serialize the shared store, not a newly
# generated pristine flat chunk.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        send_chunk_view_delta(writer, profile, view.center(), &initial_delta)?;\n",
    "        send_chunk_view_delta(\n"
    "            writer,\n"
    "            profile,\n"
    "            shared_world,\n"
    "            view.center(),\n"
    "            &initial_delta,\n"
    "        )?;\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "                        send_chunk_view_delta(writer, profile, current_chunk, &delta)?;\n",
    "                        send_chunk_view_delta(\n"
    "                            writer,\n"
    "                            profile,\n"
    "                            shared_world,\n"
    "                            current_chunk,\n"
    "                            &delta,\n"
    "                        )?;\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "fn send_chunk_view_delta<W: Write>(\n"
    "    writer: &mut W,\n"
    "    profile: &ProtocolProfile,\n"
    "    center: ChunkPos,\n"
    "    delta: &ChunkViewDelta,\n"
    ") -> Result<()> {\n",
    "fn send_chunk_view_delta<W: Write>(\n"
    "    writer: &mut W,\n"
    "    profile: &ProtocolProfile,\n"
    "    shared_world: &SharedWorld,\n"
    "    center: ChunkPos,\n"
    "    delta: &ChunkViewDelta,\n"
    ") -> Result<()> {\n",
)
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "        for pos in &delta.newly_visible {\n"
    "            write_play_payload(\n"
    "                writer,\n"
    "                profile,\n"
    "                PacketKind::LevelChunkWithLight,\n"
    "                &encode_level_chunk_with_light(&flat_chunk(*pos)?)?,\n"
    "            )?;\n"
    "        }\n",
    "        for pos in &delta.newly_visible {\n"
    "            let chunk = shared_world.chunk_snapshot(*pos)?;\n"
    "            write_play_payload(\n"
    "                writer,\n"
    "                profile,\n"
    "                PacketKind::LevelChunkWithLight,\n"
    "                &encode_level_chunk_with_light(&chunk)?,\n"
    "            )?;\n"
    "        }\n",
)

# The first center chunk sent during Play bootstrap must use the same shared
# store, so a player joining later sees prior mutations immediately.
replace_once(
    "crates/ferrum-server/src/main.rs",
    "    let chunk = static_chunk()?;\n",
    "    let chunk = world.shared_world.chunk_snapshot(ChunkPos {\n"
    "        x: STATIC_CHUNK_X,\n"
    "        z: STATIC_CHUNK_Z,\n"
    "    })?;\n",
)

# Regression tests: authoritative snapshots retain mutations and independently
# cloned snapshots cannot mutate the shared store.
replace_once(
    "crates/ferrum-server/src/play_runtime.rs",
    "    #[test]\n    fn shared_world_applies_events_from_multiple_connections_to_one_store() {\n",
    "    #[test]\n"
    "    fn shared_world_chunk_snapshots_include_authoritative_mutations() {\n"
    "        let world = SharedWorld::static_flat();\n"
    "        let position = BlockPos { x: 3, y: 65, z: -4 };\n"
    "        let state = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);\n"
    "        world\n"
    "            .apply_event(\n"
    "                ConnectionId::new(1),\n"
    "                WorldEvent::BlockMutation(BlockMutation { position, state }),\n"
    "            )\n"
    "            .unwrap();\n\n"
    "        let mut snapshot = world.chunk_snapshot(ChunkPos { x: 0, z: -1 }).unwrap();\n"
    "        assert_eq!(snapshot.world_block(position).unwrap(), state);\n\n"
    "        snapshot\n"
    "            .apply_block_mutation(BlockMutation {\n"
    "                position,\n"
    "                state: BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),\n"
    "            })\n"
    "            .unwrap();\n"
    "        assert_eq!(world.world_block(position).unwrap(), state);\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn shared_world_chunk_snapshot_reports_missing_chunks() {\n"
    "        let world = SharedWorld::static_flat();\n"
    "        let error = world\n"
    "            .chunk_snapshot(ChunkPos { x: 100, z: 100 })\n"
    "            .unwrap_err();\n"
    "        assert!(error.to_string().contains(\"missing chunk (100, 100)\"));\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn shared_world_applies_events_from_multiple_connections_to_one_store() {\n",
)

# Documentation.
replace_once(
    "README.md",
    "- Shared in-memory world state for accepted block mutations across Play connections\n"
    "- Live simplified block breaking and adjacent-face stone placement\n",
    "- Shared in-memory world state for accepted block mutations across Play connections\n"
    "- Initial and dynamically streamed chunks serialized from authoritative shared-world snapshots\n"
    "- Live simplified block breaking and adjacent-face stone placement\n",
)
replace_once(
    "docs/SERVER_ROADMAP.md",
    "- Apply simplified adjacent-face stone placement while rejecting world-border hits.\n"
    "- Keep protocol serialization and version-specific numeric IDs outside the world crate.\n",
    "- Apply simplified adjacent-face stone placement while rejecting world-border hits.\n"
    "- Serialize bootstrap and dynamically streamed chunks from shared-world snapshots so accepted mutations survive new connections and chunk re-entry.\n"
    "- Keep protocol serialization and version-specific numeric IDs outside the world crate.\n",
)
