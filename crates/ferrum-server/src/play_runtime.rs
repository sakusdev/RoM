use super::{
    KEEP_ALIVE_INTERVAL, MAX_IGNORED_PLAY_PACKETS, STATIC_CHUNK_RADIUS, STATIC_CHUNK_X,
    STATIC_CHUNK_Z, STATIC_FLOOR_Y, is_connection_eof, version_26_1_2, write_play_payload,
};
use crate::codec::{PacketReader, read_packet};
use anyhow::{Context, Result, bail};
use ferrum_play::{
    BlockPosition, PlayerMovement, PlayerState, decode_move_player_position,
    decode_move_player_position_rotation, decode_move_player_rotation, decode_move_player_status,
    decode_player_action, decode_use_item_on_block, encode_block_update,
    encode_chunk_batch_finished, encode_chunk_batch_start, encode_forget_level_chunk,
    encode_keep_alive, encode_level_chunk_with_light, encode_set_chunk_cache_center,
    player_action_to_world_event, use_item_on_block_to_world_event,
};
use ferrum_protocol::{
    PacketDirection, PacketKind, ProtocolPhase, ProtocolProfile, ProtocolSession,
};
use ferrum_runtime::{ConnectionId, DeterministicRuntime, Tick};
use ferrum_world::{
    AppliedWorldEvent, BiomeId, BlockPos, BlockStateId, ChunkPos, ChunkStore, ChunkView,
    ChunkViewDelta, FlatWorldSpec, StaticChunk, WorldEvent,
};
use std::{
    io::{Read, Write},
    num::NonZeroUsize,
    sync::Mutex,
};

const CLIENT_TICKS_PER_SECOND: usize = 20;
#[cfg(test)]
const LOCAL_WORLD_CONNECTION_ID: ConnectionId = ConnectionId::new(1);
const LOCAL_WORLD_QUEUE_CAPACITY: usize = 128;
const LOCAL_WORLD_EVENTS_PER_TICK: usize = 16;

type LocalWorldRuntime = DeterministicRuntime<ChunkStore, WorldEvent>;

#[derive(Debug)]
pub(super) struct SharedWorld {
    inner: Mutex<SharedWorldInner>,
}

#[derive(Debug)]
struct SharedWorldInner {
    runtime: LocalWorldRuntime,
    tick: Tick,
}

impl SharedWorld {
    pub(super) fn new(center: ChunkPos) -> Result<Self> {
        Ok(Self {
            inner: Mutex::new(SharedWorldInner {
                runtime: new_local_world_runtime(center)?,
                tick: Tick::ZERO,
            }),
        })
    }

    pub(super) fn static_flat() -> Self {
        Self::new(ChunkPos {
            x: STATIC_CHUNK_X,
            z: STATIC_CHUNK_Z,
        })
        .expect("static flat world constants must build a valid chunk store")
    }

    fn ensure_chunks_loaded(&self, positions: &[ChunkPos]) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        ensure_chunks_loaded(inner.runtime.state_mut(), positions)
    }

    fn apply_event(
        &self,
        connection: ConnectionId,
        event: WorldEvent,
    ) -> Result<Vec<AppliedWorldEvent>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        inner.tick = next_tick(inner.tick)?;
        let tick = inner.tick;
        apply_world_event(&mut inner.runtime, connection, tick, event)
    }

    #[cfg(test)]
    fn world_block(&self, position: BlockPos) -> Result<BlockStateId> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        Ok(inner.runtime.state().world_block(position)?)
    }
}

pub(super) fn is_movement_packet_id(profile: &ProtocolProfile, packet_id: i32) -> bool {
    matches!(
        profile
            .packets()
            .resolve(ProtocolPhase::Play, PacketDirection::Serverbound, packet_id,),
        Some(
            PacketKind::MovePlayerPosition
                | PacketKind::MovePlayerPositionRotation
                | PacketKind::MovePlayerRotation
                | PacketKind::MovePlayerStatusOnly
        )
    )
}

pub(super) fn run_play_loop<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    profile: &ProtocolProfile,
    session: &mut ProtocolSession,
    shared_world: &SharedWorld,
    connection: ConnectionId,
    play_round_limit: Option<usize>,
) -> Result<()> {
    if play_round_limit == Some(0) {
        return Ok(());
    }

    let mut player = PlayerState::new([0.5, 65.0, 0.5], 0.0, 0.0, false, false)?;
    let mut view = ChunkView::new(
        ChunkPos {
            x: STATIC_CHUNK_X,
            z: STATIC_CHUNK_Z,
        },
        STATIC_CHUNK_RADIUS,
    )?;
    view.mark_loaded(player.chunk_pos());
    if play_round_limit.is_none() {
        let initial_delta = view.synchronize()?;
        shared_world.ensure_chunks_loaded(&initial_delta.newly_visible)?;
        send_chunk_view_delta(writer, profile, view.center(), &initial_delta)?;
    }

    let tick_interval = usize::try_from(KEEP_ALIVE_INTERVAL.as_secs())
        .context("keep alive interval exceeds usize")?
        .checked_mul(CLIENT_TICKS_PER_SECOND)
        .context("keep alive tick interval overflow")?;
    let mut keep_alive_id = 1_i64;
    let mut completed_rounds = 0_usize;
    let mut ignored_packets = 0_usize;

    loop {
        write_play_payload(
            writer,
            profile,
            PacketKind::KeepAliveRequest,
            &encode_keep_alive(keep_alive_id),
        )?;
        session.keep_alive_sent(keep_alive_id)?;
        writer.flush()?;

        let mut keep_alive_acknowledged = false;
        let mut ticks_since_request = 0_usize;
        loop {
            let packet = match read_packet(reader).context("cannot read Play packet") {
                Ok(packet) => packet,
                Err(error) if is_connection_eof(&error) => {
                    session.disconnect();
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            let mut packet_reader = PacketReader::new(&packet);
            let packet_id = packet_reader.read_varint()?;
            let kind = profile.packets().resolve(
                ProtocolPhase::Play,
                PacketDirection::Serverbound,
                packet_id,
            );

            match kind {
                Some(PacketKind::KeepAliveResponse) => {
                    let received_id = packet_reader.read_i64()?;
                    if received_id != keep_alive_id {
                        bail!("expected keep alive id {keep_alive_id}, got {received_id}");
                    }
                    require_empty(&mut packet_reader, "keep alive response")?;
                    session.keep_alive_response(keep_alive_id)?;
                    keep_alive_acknowledged = true;
                    completed_rounds += 1;
                    if play_round_limit.is_some_and(|limit| completed_rounds >= limit) {
                        return Ok(());
                    }
                }
                Some(PacketKind::ClientTickEnd) => {
                    require_empty(&mut packet_reader, "client tick end")?;
                    ticks_since_request = ticks_since_request.saturating_add(1);
                }
                Some(PacketKind::ChunkBatchReceived) => {
                    let desired_chunks_per_tick = packet_reader.read_f32()?;
                    if !desired_chunks_per_tick.is_finite() || desired_chunks_per_tick <= 0.0 {
                        bail!(
                            "chunk batch acknowledgement contains invalid desired chunks per tick {desired_chunks_per_tick}"
                        );
                    }
                    require_empty(&mut packet_reader, "chunk batch acknowledgement")?;
                }
                Some(
                    kind @ (PacketKind::MovePlayerPosition
                    | PacketKind::MovePlayerPositionRotation
                    | PacketKind::MovePlayerRotation
                    | PacketKind::MovePlayerStatusOnly),
                ) => {
                    let movement = decode_movement(kind, packet_reader.take_remaining())?;
                    let previous_chunk = player.chunk_pos();
                    player.apply(movement);
                    let current_chunk = player.chunk_pos();
                    if current_chunk != previous_chunk {
                        let delta = view.recenter(current_chunk)?;
                        shared_world.ensure_chunks_loaded(&delta.newly_visible)?;
                        send_chunk_view_delta(writer, profile, current_chunk, &delta)?;
                    }
                }
                Some(PacketKind::PlayerAction) => {
                    let action = decode_player_action(packet_reader.take_remaining())?;
                    if let Some(event) = player_action_to_world_event(
                        action,
                        BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),
                    ) {
                        let applied = shared_world.apply_event(connection, event)?;
                        send_world_updates(writer, profile, &applied)?;
                    }
                }
                Some(PacketKind::UseItemOn) => {
                    let interaction = decode_use_item_on_block(packet_reader.take_remaining())?;
                    let event = use_item_on_block_to_world_event(
                        interaction,
                        BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),
                    );
                    let applied = shared_world.apply_event(connection, event)?;
                    send_world_updates(writer, profile, &applied)?;
                }
                _ => {
                    ignored_packets = ignored_packets
                        .checked_add(1)
                        .context("ignored Play packet count overflow")?;
                    if ignored_packets > MAX_IGNORED_PLAY_PACKETS {
                        bail!("ignored Play packet limit exceeded");
                    }
                }
            }

            if keep_alive_acknowledged && ticks_since_request >= tick_interval {
                break;
            }
        }

        keep_alive_id = keep_alive_id
            .checked_add(1)
            .context("keep alive id overflow")?;
    }
}

fn new_local_world_runtime(center: ChunkPos) -> Result<LocalWorldRuntime> {
    let mut store = ChunkStore::new();
    seed_chunk_square(&mut store, center, STATIC_CHUNK_RADIUS)?;
    Ok(DeterministicRuntime::new(
        store,
        non_zero_usize(LOCAL_WORLD_QUEUE_CAPACITY),
        non_zero_usize(LOCAL_WORLD_EVENTS_PER_TICK),
    ))
}

fn non_zero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("local world runtime constants must be non-zero")
}

fn seed_chunk_square(store: &mut ChunkStore, center: ChunkPos, radius: i32) -> Result<()> {
    for z in center
        .z
        .checked_sub(radius)
        .context("visible chunk z minimum overflow")?
        ..=center
            .z
            .checked_add(radius)
            .context("visible chunk z maximum overflow")?
    {
        for x in center
            .x
            .checked_sub(radius)
            .context("visible chunk x minimum overflow")?
            ..=center
                .x
                .checked_add(radius)
                .context("visible chunk x maximum overflow")?
        {
            store.insert(flat_chunk(ChunkPos { x, z })?);
        }
    }
    Ok(())
}

fn ensure_chunks_loaded(store: &mut ChunkStore, positions: &[ChunkPos]) -> Result<()> {
    for pos in positions {
        if store.chunk(*pos).is_none() {
            store.insert(flat_chunk(*pos)?);
        }
    }
    Ok(())
}

fn next_tick(tick: Tick) -> Result<Tick> {
    let next = tick
        .get()
        .checked_add(1)
        .context("local world tick overflow")?;
    Ok(Tick::new(next))
}

fn apply_world_event(
    runtime: &mut LocalWorldRuntime,
    connection: ConnectionId,
    tick: Tick,
    event: WorldEvent,
) -> Result<Vec<AppliedWorldEvent>> {
    runtime
        .push_input(connection, event)
        .context("local world input queue is full")?;

    let mut apply_error = None;
    let mut applied_events = Vec::new();
    runtime.execute_tick(tick, |store, _tick, envelope| {
        if apply_error.is_none() {
            match store.apply_event(envelope.payload) {
                Ok(applied) => applied_events.push(applied),
                Err(error) => apply_error = Some(error),
            }
        }
    });

    if let Some(error) = apply_error {
        bail!("cannot apply local world event: {error}");
    }
    Ok(applied_events)
}

#[cfg(test)]
fn apply_local_world_event(
    runtime: &mut LocalWorldRuntime,
    tick: Tick,
    event: WorldEvent,
) -> Result<Vec<AppliedWorldEvent>> {
    apply_world_event(runtime, LOCAL_WORLD_CONNECTION_ID, tick, event)
}

fn send_world_updates<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    applied_events: &[AppliedWorldEvent],
) -> Result<()> {
    if profile.packets().id(PacketKind::BlockUpdate).is_none() {
        return Ok(());
    }

    for event in applied_events {
        let AppliedWorldEvent::BlockMutation(mutation) = event;
        write_play_payload(
            writer,
            profile,
            PacketKind::BlockUpdate,
            &encode_block_update(
                block_position_from_world(mutation.position),
                mutation.current,
            )?,
        )?;
    }
    writer.flush()?;
    Ok(())
}

fn block_position_from_world(position: BlockPos) -> BlockPosition {
    BlockPosition {
        x: position.x,
        y: position.y,
        z: position.z,
    }
}

fn decode_movement(kind: PacketKind, payload: &[u8]) -> Result<PlayerMovement> {
    Ok(match kind {
        PacketKind::MovePlayerPosition => decode_move_player_position(payload)?,
        PacketKind::MovePlayerPositionRotation => decode_move_player_position_rotation(payload)?,
        PacketKind::MovePlayerRotation => decode_move_player_rotation(payload)?,
        PacketKind::MovePlayerStatusOnly => decode_move_player_status(payload)?,
        _ => bail!("packet {kind:?} is not a movement packet"),
    })
}

fn require_empty(reader: &mut PacketReader<'_>, label: &str) -> Result<()> {
    if !reader.take_remaining().is_empty() {
        bail!("{label} contains trailing bytes");
    }
    Ok(())
}

fn send_chunk_view_delta<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    center: ChunkPos,
    delta: &ChunkViewDelta,
) -> Result<()> {
    if delta.center_changed {
        write_play_payload(
            writer,
            profile,
            PacketKind::SetChunkCacheCenter,
            &encode_set_chunk_cache_center(center.x, center.z),
        )?;
    }

    for pos in &delta.no_longer_visible {
        write_play_payload(
            writer,
            profile,
            PacketKind::ForgetLevelChunk,
            &encode_forget_level_chunk(*pos),
        )?;
    }

    if !delta.newly_visible.is_empty() {
        write_play_payload(
            writer,
            profile,
            PacketKind::ChunkBatchStart,
            &encode_chunk_batch_start(),
        )?;
        for pos in &delta.newly_visible {
            write_play_payload(
                writer,
                profile,
                PacketKind::LevelChunkWithLight,
                &encode_level_chunk_with_light(&flat_chunk(*pos)?)?,
            )?;
        }
        let batch_size =
            i32::try_from(delta.newly_visible.len()).context("visible chunk batch exceeds i32")?;
        write_play_payload(
            writer,
            profile,
            PacketKind::ChunkBatchFinished,
            &encode_chunk_batch_finished(batch_size)?,
        )?;
    }
    writer.flush()?;
    Ok(())
}

fn flat_chunk(pos: ChunkPos) -> Result<StaticChunk> {
    Ok(StaticChunk::flat_overworld(
        pos,
        version_26_1_2::OVERWORLD_MIN_SECTION_Y,
        version_26_1_2::OVERWORLD_SECTION_COUNT,
        FlatWorldSpec {
            floor_y: STATIC_FLOOR_Y,
            air: BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),
            bedrock: BlockStateId::new(version_26_1_2::BEDROCK_BLOCK_STATE_ID),
            stone: BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),
            dirt: BlockStateId::new(version_26_1_2::DIRT_BLOCK_STATE_ID),
            grass: BlockStateId::new(version_26_1_2::GRASS_BLOCK_STATE_ID),
            biome: BiomeId::new(version_26_1_2::PLAINS_BIOME_ID),
        },
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrum_protocol::PacketTable;
    use ferrum_world::{BlockMutation, BlockPos};

    #[test]
    fn movement_packet_classifier_is_phase_and_direction_aware() {
        let profile = version_26_1_2::protocol_profile().unwrap();
        assert!(is_movement_packet_id(&profile, 0x1e));
        assert!(is_movement_packet_id(&profile, 0x1f));
        assert!(is_movement_packet_id(&profile, 0x20));
        assert!(is_movement_packet_id(&profile, 0x21));
        assert!(!is_movement_packet_id(&profile, 0x48));
    }

    #[test]
    fn local_world_runtime_applies_block_events_through_authoritative_ticks() {
        let mut runtime = new_local_world_runtime(ChunkPos { x: 0, z: 0 }).unwrap();
        let position = BlockPos { x: 0, y: 65, z: 0 };
        let state = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);

        let applied = apply_local_world_event(
            &mut runtime,
            Tick::new(1),
            WorldEvent::BlockMutation(BlockMutation { position, state }),
        )
        .unwrap();

        assert_eq!(applied.len(), 1);
        assert_eq!(runtime.pending_inputs(), 0);
        assert_eq!(runtime.state().world_block(position).unwrap(), state);
    }

    #[test]
    fn sends_block_update_when_profile_exposes_packet_id() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::BlockUpdate, 0x22).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let mut runtime = new_local_world_runtime(ChunkPos { x: 0, z: 0 }).unwrap();
        let position = BlockPos { x: 1, y: 65, z: -2 };
        let state = BlockStateId::new(1);
        let applied = apply_local_world_event(
            &mut runtime,
            Tick::new(1),
            WorldEvent::BlockMutation(BlockMutation { position, state }),
        )
        .unwrap();
        let mut output = Vec::new();

        send_world_updates(&mut output, &profile, &applied).unwrap();

        let mut expected = vec![10, 0x22];
        expected.extend_from_slice(
            &BlockPosition { x: 1, y: 65, z: -2 }
                .pack_for_test()
                .to_be_bytes(),
        );
        expected.push(1);
        assert_eq!(output, expected);
    }

    #[test]
    fn shared_world_applies_events_from_multiple_connections_to_one_store() {
        let world = SharedWorld::static_flat();
        let position = BlockPos { x: 2, y: 65, z: 2 };
        let stone = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);
        let air = BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID);

        let first = world
            .apply_event(
                ConnectionId::new(1),
                WorldEvent::BlockMutation(BlockMutation {
                    position,
                    state: stone,
                }),
            )
            .unwrap();
        let second = world
            .apply_event(
                ConnectionId::new(2),
                WorldEvent::BlockMutation(BlockMutation {
                    position,
                    state: air,
                }),
            )
            .unwrap();

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(world.world_block(position).unwrap(), air);
    }

    trait TestBlockPositionPack {
        fn pack_for_test(self) -> i64;
    }

    impl TestBlockPositionPack for BlockPosition {
        fn pack_for_test(self) -> i64 {
            let x = i64::from(self.x) & 0x3ff_ffff;
            let y = i64::from(self.y) & 0xfff;
            let z = i64::from(self.z) & 0x3ff_ffff;
            (x << 38) | (z << 12) | y
        }
    }
}
