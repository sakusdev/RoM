use super::{
    MAX_IGNORED_PLAY_PACKETS, PlayPolicy, is_connection_eof, is_transient_read_timeout,
    version_26_1_2, write_play_payload,
};
use crate::codec::{PacketReader, read_packet};
use anyhow::{Context, Result, bail};
use ferrum_play::{
    BlockPosition, PlayerMovement, PlayerState, decode_player_action, decode_use_item_on_block,
    encode_block_changed_ack, encode_block_update, encode_chunk_batch_finished,
    encode_chunk_batch_start, encode_forget_level_chunk, encode_keep_alive,
    encode_level_chunk_with_light, encode_set_chunk_cache_center, player_action_to_world_event,
    use_item_on_block_to_world_event,
};
use ferrum_protocol::{
    PacketDirection, PacketKind, ProtocolPhase, ProtocolProfile, ProtocolSession,
};
use ferrum_rompack::{RomPackBiomes, RomPackBlockStates, RomPackWorld};
use ferrum_runtime::{ConnectionId, DeterministicRuntime, Tick};
use ferrum_server::{authoritative_runtime::PlayInput, play_input::decode_play_input};
use ferrum_world::{
    AppliedWorldEvent, BiomeId, BlockPos, BlockStateId, ChunkPos, ChunkStore, ChunkView,
    ChunkViewDelta, FlatWorldSpec, StaticChunk, WorldError, WorldEvent,
};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    num::NonZeroUsize,
    sync::Mutex,
};

const CLIENT_TICKS_PER_SECOND: usize = 20;
#[cfg(test)]
const LOCAL_WORLD_CONNECTION_ID: ConnectionId = ConnectionId::new(1);
const LOCAL_WORLD_QUEUE_CAPACITY: usize = 128;
const LOCAL_WORLD_EVENTS_PER_TICK: usize = 16;
const MAX_PENDING_WORLD_UPDATES_PER_CONNECTION: usize = 256;
const MAX_WORLD_UPDATES_PER_DRAIN: usize = 64;
const MAX_PLAYER_MOVE_DELTA: f64 = 100.0;
const MAX_PLAYER_MOVE_DELTA_SQUARED: f64 = MAX_PLAYER_MOVE_DELTA * MAX_PLAYER_MOVE_DELTA;
const PLAYER_EYE_HEIGHT: f64 = 1.62;
const MAX_BLOCK_INTERACTION_REACH: f64 = 6.0;
const MAX_BLOCK_INTERACTION_REACH_SQUARED: f64 =
    MAX_BLOCK_INTERACTION_REACH * MAX_BLOCK_INTERACTION_REACH;

type LocalWorldRuntime = DeterministicRuntime<ChunkStore, WorldEvent>;

#[derive(Debug)]
pub(super) struct SharedWorld {
    profile: RomPackWorld,
    play_policy: PlayPolicy,
    inner: Mutex<SharedWorldInner>,
}

#[derive(Debug)]
struct SharedWorldInner {
    runtime: LocalWorldRuntime,
    tick: Tick,
    subscribers: BTreeMap<ConnectionId, PendingWorldUpdates>,
}

#[derive(Debug, Default)]
struct PendingWorldUpdates {
    updates: VecDeque<AppliedWorldEvent>,
}

impl PendingWorldUpdates {
    fn push(&mut self, event: AppliedWorldEvent) {
        let position = applied_world_event_position(&event);
        if let Some(existing) = self
            .updates
            .iter_mut()
            .find(|existing| applied_world_event_position(existing) == position)
        {
            *existing = event;
            return;
        }

        if self.updates.len() == MAX_PENDING_WORLD_UPDATES_PER_CONNECTION {
            self.updates.pop_front();
        }
        self.updates.push_back(event);
    }

    fn drain(&mut self, limit: usize) -> Vec<AppliedWorldEvent> {
        let count = limit.min(self.updates.len());
        self.updates.drain(..count).collect()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.updates.len()
    }
}

#[derive(Debug)]
pub(super) struct SharedWorldSubscription<'a> {
    world: &'a SharedWorld,
    connection: ConnectionId,
}

impl Drop for SharedWorldSubscription<'_> {
    fn drop(&mut self) {
        self.world.unsubscribe(self.connection);
    }
}

pub(super) fn builtin_world_profile() -> RomPackWorld {
    RomPackWorld {
        data_version: version_26_1_2::WORLD_VERSION,
        overworld_min_section_y: version_26_1_2::OVERWORLD_MIN_SECTION_Y,
        overworld_section_count: version_26_1_2::OVERWORLD_SECTION_COUNT,
        dimension: version_26_1_2::OVERWORLD_DIMENSION.to_owned(),
        dimension_type_id: version_26_1_2::OVERWORLD_DIMENSION_TYPE_ID,
        sea_level: version_26_1_2::OVERWORLD_SEA_LEVEL,
        floor_y: version_26_1_2::FLAT_WORLD_FLOOR_Y,
        spawn_x: version_26_1_2::FLAT_WORLD_SPAWN_X,
        spawn_z: version_26_1_2::FLAT_WORLD_SPAWN_Z,
        block_states: RomPackBlockStates {
            air: version_26_1_2::AIR_BLOCK_STATE_ID,
            stone: version_26_1_2::STONE_BLOCK_STATE_ID,
            grass: version_26_1_2::GRASS_BLOCK_STATE_ID,
            dirt: version_26_1_2::DIRT_BLOCK_STATE_ID,
            bedrock: version_26_1_2::BEDROCK_BLOCK_STATE_ID,
        },
        biomes: RomPackBiomes {
            plains: version_26_1_2::PLAINS_BIOME_ID,
        },
    }
}

impl SharedWorld {
    #[cfg(test)]
    pub(super) fn new(center: ChunkPos, profile: RomPackWorld) -> Result<Self> {
        Self::new_with_policy(center, profile, PlayPolicy::default())
    }

    pub(super) fn new_with_policy(
        center: ChunkPos,
        profile: RomPackWorld,
        play_policy: PlayPolicy,
    ) -> Result<Self> {
        let runtime =
            new_local_world_runtime_with_radius(center, &profile, play_policy.chunk_radius)?;
        Ok(Self {
            profile,
            play_policy,
            inner: Mutex::new(SharedWorldInner {
                runtime,
                tick: Tick::ZERO,
                subscribers: BTreeMap::new(),
            }),
        })
    }

    #[cfg(test)]
    pub(super) fn static_flat() -> Self {
        let profile = builtin_world_profile();
        let center = spawn_chunk(&profile);
        Self::new(center, profile)
            .expect("static flat world constants must build a valid chunk store")
    }

    fn ensure_chunks_loaded(&self, positions: &[ChunkPos]) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        ensure_chunks_loaded(inner.runtime.state_mut(), positions, &self.profile)
    }

    pub(super) fn subscribe(
        &self,
        connection: ConnectionId,
    ) -> Result<SharedWorldSubscription<'_>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        if inner.subscribers.contains_key(&connection) {
            bail!(
                "connection {} is already subscribed to the shared world",
                connection.get()
            );
        }
        inner
            .subscribers
            .insert(connection, PendingWorldUpdates::default());
        Ok(SharedWorldSubscription {
            world: self,
            connection,
        })
    }

    fn unsubscribe(&self, connection: ConnectionId) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.subscribers.remove(&connection);
        }
    }

    fn drain_updates(
        &self,
        connection: ConnectionId,
        limit: usize,
    ) -> Result<Vec<AppliedWorldEvent>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        let pending = inner.subscribers.get_mut(&connection).with_context(|| {
            format!(
                "connection {} is not subscribed to the shared world",
                connection.get()
            )
        })?;
        Ok(pending.drain(limit))
    }

    #[must_use]
    pub(super) fn world_profile(&self) -> &RomPackWorld {
        &self.profile
    }

    pub(super) fn play_policy(&self) -> &PlayPolicy {
        &self.play_policy
    }

    pub(super) fn chunk_snapshot(&self, pos: ChunkPos) -> Result<StaticChunk> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        inner
            .runtime
            .state()
            .chunk(pos)
            .cloned()
            .with_context(|| format!("shared world is missing chunk ({}, {})", pos.x, pos.z))
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
        let applied = apply_world_event(&mut inner.runtime, connection, tick, event)?;
        for applied_event in applied.iter().copied() {
            for (subscriber, pending) in &mut inner.subscribers {
                if *subscriber != connection {
                    pending.push(applied_event);
                }
            }
        }
        Ok(applied)
    }

    fn interaction_block_state(&self, position: BlockPos) -> Result<Option<BlockStateId>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("shared world lock poisoned"))?;
        match inner.runtime.state().world_block(position) {
            Ok(state) => Ok(Some(state)),
            Err(WorldError::SectionOutOfRange { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    #[cfg(test)]
    fn subscriber_count(&self) -> usize {
        self.inner
            .lock()
            .expect("shared world lock must not be poisoned in tests")
            .subscribers
            .len()
    }

    #[cfg(test)]
    fn pending_update_count(&self, connection: ConnectionId) -> usize {
        self.inner
            .lock()
            .expect("shared world lock must not be poisoned in tests")
            .subscribers
            .get(&connection)
            .map_or(0, PendingWorldUpdates::len)
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

    let world_profile = shared_world.world_profile();
    let play_policy = shared_world.play_policy();
    let mut player =
        PlayerState::new(player_spawn_position(world_profile), 0.0, 0.0, false, false)?;
    let mut view = ChunkView::new(spawn_chunk(world_profile), play_policy.chunk_radius)?;
    view.mark_loaded(player.chunk_pos());
    if play_round_limit.is_none() {
        let initial_delta = view.synchronize()?;
        shared_world.ensure_chunks_loaded(&initial_delta.newly_visible)?;
        send_chunk_view_delta(writer, profile, shared_world, view.center(), &initial_delta)?;
    }

    let tick_interval = keep_alive_tick_interval(play_policy)?;
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
                Err(error) if is_transient_read_timeout(&error) => {
                    drain_and_send_pending_world_updates(
                        writer,
                        profile,
                        shared_world,
                        connection,
                    )?;
                    continue;
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
                Some(
                    kind @ (PacketKind::KeepAliveResponse
                    | PacketKind::ClientTickEnd
                    | PacketKind::ChunkBatchReceived
                    | PacketKind::MovePlayerPosition
                    | PacketKind::MovePlayerPositionRotation
                    | PacketKind::MovePlayerRotation
                    | PacketKind::MovePlayerStatusOnly),
                ) => {
                    let input = decode_play_input(kind, packet_reader.take_remaining())?
                        .context("resolved migrated Play packet did not decode")?;
                    match input {
                        PlayInput::KeepAliveResponse(received_id) => {
                            if received_id != keep_alive_id {
                                bail!("expected keep alive id {keep_alive_id}, got {received_id}");
                            }
                            session.keep_alive_response(keep_alive_id)?;
                            keep_alive_acknowledged = true;
                            completed_rounds += 1;
                            if play_round_limit.is_some_and(|limit| completed_rounds >= limit) {
                                return Ok(());
                            }
                        }
                        PlayInput::ClientTickEnd => {
                            ticks_since_request = ticks_since_request.saturating_add(1);
                        }
                        PlayInput::ChunkBatchReceived(_) => {}
                        PlayInput::Movement(movement) => {
                            validate_movement_delta(&player, movement)?;
                            validate_movement_floor(movement, world_profile.floor_y)?;
                            let previous_chunk = player.chunk_pos();
                            player.apply(movement);
                            let current_chunk = player.chunk_pos();
                            if current_chunk != previous_chunk {
                                let delta = view.recenter(current_chunk)?;
                                shared_world.ensure_chunks_loaded(&delta.newly_visible)?;
                                send_chunk_view_delta(
                                    writer,
                                    profile,
                                    shared_world,
                                    current_chunk,
                                    &delta,
                                )?;
                            }
                        }
                        PlayInput::Disconnected => {
                            unreachable!("socket disconnect is not decoded from a Play packet")
                        }
                    }
                }
                Some(PacketKind::PlayerAction) => {
                    let action = decode_player_action(packet_reader.take_remaining())?;
                    let sequence = action.sequence;
                    if is_block_interaction_within_reach(&player, action.position)
                        && let Some(event) = player_action_to_world_event(
                            action,
                            BlockStateId::new(shared_world.world_profile().block_states.air),
                        )
                        && is_break_target_mutable(shared_world, event)?
                    {
                        let applied = shared_world.apply_event(connection, event)?;
                        send_world_updates(writer, profile, &applied)?;
                    }
                    send_block_changed_ack(writer, profile, sequence)?;
                }
                Some(PacketKind::UseItemOn) => {
                    let interaction = decode_use_item_on_block(packet_reader.take_remaining())?;
                    let sequence = interaction.sequence;
                    if is_block_interaction_within_reach(&player, interaction.position)
                        && let Some(event) = use_item_on_block_to_world_event(
                            interaction,
                            BlockStateId::new(shared_world.world_profile().block_states.stone),
                        )
                        && is_placement_target_air(shared_world, event)?
                    {
                        let applied = shared_world.apply_event(connection, event)?;
                        send_world_updates(writer, profile, &applied)?;
                    }
                    send_block_changed_ack(writer, profile, sequence)?;
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

            drain_and_send_pending_world_updates(writer, profile, shared_world, connection)?;

            if keep_alive_acknowledged && ticks_since_request >= tick_interval {
                break;
            }
        }

        keep_alive_id = keep_alive_id
            .checked_add(1)
            .context("keep alive id overflow")?;
    }
}

fn keep_alive_tick_interval(play_policy: &PlayPolicy) -> Result<usize> {
    usize::try_from(play_policy.keep_alive_interval_seconds)
        .context("keep alive interval exceeds usize")?
        .checked_mul(CLIENT_TICKS_PER_SECOND)
        .context("keep alive tick interval overflow")
}

#[cfg(test)]
fn new_local_world_runtime(center: ChunkPos, profile: &RomPackWorld) -> Result<LocalWorldRuntime> {
    new_local_world_runtime_with_radius(center, profile, PlayPolicy::default().chunk_radius)
}

fn new_local_world_runtime_with_radius(
    center: ChunkPos,
    profile: &RomPackWorld,
    chunk_radius: i32,
) -> Result<LocalWorldRuntime> {
    let mut store = ChunkStore::new();
    seed_chunk_square(&mut store, center, chunk_radius, profile)?;
    Ok(DeterministicRuntime::new(
        store,
        non_zero_usize(LOCAL_WORLD_QUEUE_CAPACITY),
        non_zero_usize(LOCAL_WORLD_EVENTS_PER_TICK),
    ))
}

fn non_zero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("local world runtime constants must be non-zero")
}

fn seed_chunk_square(
    store: &mut ChunkStore,
    center: ChunkPos,
    radius: i32,
    profile: &RomPackWorld,
) -> Result<()> {
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
            store.insert(flat_chunk(ChunkPos { x, z }, profile)?);
        }
    }
    Ok(())
}

fn ensure_chunks_loaded(
    store: &mut ChunkStore,
    positions: &[ChunkPos],
    profile: &RomPackWorld,
) -> Result<()> {
    for pos in positions {
        if store.chunk(*pos).is_none() {
            store.insert(flat_chunk(*pos, profile)?);
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

fn send_block_changed_ack<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    sequence: i32,
) -> Result<()> {
    write_play_payload(
        writer,
        profile,
        PacketKind::BlockChangedAck,
        &encode_block_changed_ack(sequence)?,
    )?;
    writer.flush()?;
    Ok(())
}

fn send_world_updates<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    applied_events: &[AppliedWorldEvent],
) -> Result<()> {
    if applied_events.is_empty() {
        return Ok(());
    }
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

fn drain_and_send_pending_world_updates<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    shared_world: &SharedWorld,
    connection: ConnectionId,
) -> Result<()> {
    let pending_updates = shared_world.drain_updates(connection, MAX_WORLD_UPDATES_PER_DRAIN)?;
    send_world_updates(writer, profile, &pending_updates)
}

fn applied_world_event_position(event: &AppliedWorldEvent) -> BlockPos {
    match event {
        AppliedWorldEvent::BlockMutation(mutation) => mutation.position,
    }
}

fn block_position_from_world(position: BlockPos) -> BlockPosition {
    BlockPosition {
        x: position.x,
        y: position.y,
        z: position.z,
    }
}

fn validate_movement_delta(player: &PlayerState, movement: PlayerMovement) -> Result<()> {
    let Some(next_position) = movement_position(movement) else {
        return Ok(());
    };
    let distance_squared = player
        .position
        .into_iter()
        .zip(next_position)
        .map(|(from, to)| {
            let delta = from - to;
            delta * delta
        })
        .sum::<f64>();
    if distance_squared > MAX_PLAYER_MOVE_DELTA_SQUARED {
        bail!(
            "player movement delta exceeds {MAX_PLAYER_MOVE_DELTA} blocks: squared distance {distance_squared}"
        );
    }
    Ok(())
}

fn validate_movement_floor(movement: PlayerMovement, floor_y: i32) -> Result<()> {
    let Some(next_position) = movement_position(movement) else {
        return Ok(());
    };
    let minimum_feet_y = f64::from(floor_y) + 1.0;
    if next_position[1] < minimum_feet_y {
        bail!(
            "player movement feet y {} is below flat-world floor {}",
            next_position[1],
            minimum_feet_y
        );
    }
    Ok(())
}

fn movement_position(movement: PlayerMovement) -> Option<[f64; 3]> {
    match movement {
        PlayerMovement::Position { position, .. }
        | PlayerMovement::PositionRotation { position, .. } => Some(position),
        PlayerMovement::Rotation { .. } | PlayerMovement::StatusOnly { .. } => None,
    }
}

fn is_block_interaction_within_reach(player: &PlayerState, position: BlockPosition) -> bool {
    let eye = [
        player.position[0],
        player.position[1] + PLAYER_EYE_HEIGHT,
        player.position[2],
    ];
    let block_center = [
        f64::from(position.x) + 0.5,
        f64::from(position.y) + 0.5,
        f64::from(position.z) + 0.5,
    ];
    let distance_squared = eye
        .into_iter()
        .zip(block_center)
        .map(|(from, to)| {
            let delta = from - to;
            delta * delta
        })
        .sum::<f64>();
    distance_squared <= MAX_BLOCK_INTERACTION_REACH_SQUARED
}

fn is_placement_target_air(shared_world: &SharedWorld, event: WorldEvent) -> Result<bool> {
    let WorldEvent::BlockMutation(mutation) = event;
    Ok(matches!(
        shared_world.interaction_block_state(mutation.position)?,
        Some(state) if state == BlockStateId::new(shared_world.world_profile().block_states.air)
    ))
}

fn is_break_target_mutable(shared_world: &SharedWorld, event: WorldEvent) -> Result<bool> {
    let WorldEvent::BlockMutation(mutation) = event;
    Ok(matches!(
        shared_world.interaction_block_state(mutation.position)?,
        Some(state)
            if state != BlockStateId::new(shared_world.world_profile().block_states.air)
                && state != BlockStateId::new(shared_world.world_profile().block_states.bedrock)
    ))
}

fn send_chunk_view_delta<W: Write>(
    writer: &mut W,
    profile: &ProtocolProfile,
    shared_world: &SharedWorld,
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
            let chunk = shared_world.chunk_snapshot(*pos)?;
            write_play_payload(
                writer,
                profile,
                PacketKind::LevelChunkWithLight,
                &encode_level_chunk_with_light(&chunk)?,
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

fn flat_chunk(pos: ChunkPos, profile: &RomPackWorld) -> Result<StaticChunk> {
    Ok(StaticChunk::flat_overworld(
        pos,
        profile.overworld_min_section_y,
        profile.overworld_section_count,
        FlatWorldSpec {
            floor_y: profile.floor_y,
            air: BlockStateId::new(profile.block_states.air),
            bedrock: BlockStateId::new(profile.block_states.bedrock),
            stone: BlockStateId::new(profile.block_states.stone),
            dirt: BlockStateId::new(profile.block_states.dirt),
            grass: BlockStateId::new(profile.block_states.grass),
            biome: BiomeId::new(profile.biomes.plains),
        },
    )?)
}

pub(super) fn spawn_chunk(profile: &RomPackWorld) -> ChunkPos {
    ChunkPos {
        x: profile.spawn_x.div_euclid(16),
        z: profile.spawn_z.div_euclid(16),
    }
}

pub(super) fn player_spawn_position(profile: &RomPackWorld) -> [f64; 3] {
    [
        f64::from(profile.spawn_x) + 0.5,
        f64::from(profile.floor_y) + 2.0,
        f64::from(profile.spawn_z) + 0.5,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{build_packet, write_packet, write_varint_vec};
    use ferrum_protocol::PacketTable;
    use ferrum_world::{BlockMutation, BlockPos};
    use std::io::{self, Cursor, Read};

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
    fn configured_play_policy_controls_loaded_radius_and_keep_alive_cadence() {
        let policy = PlayPolicy {
            chunk_radius: 2,
            keep_alive_interval_seconds: 3,
            ..PlayPolicy::default()
        };
        let profile = builtin_world_profile();
        let world =
            SharedWorld::new_with_policy(ChunkPos { x: 0, z: 0 }, profile, policy.clone()).unwrap();
        assert!(world.chunk_snapshot(ChunkPos { x: 2, z: 2 }).is_ok());
        assert!(world.chunk_snapshot(ChunkPos { x: 3, z: 0 }).is_err());
        assert_eq!(world.play_policy(), &policy);
        assert_eq!(keep_alive_tick_interval(&policy).unwrap(), 60);
    }

    #[test]
    fn generated_world_profile_drives_chunk_layout_and_block_states() {
        let mut profile = builtin_world_profile();
        profile.overworld_min_section_y = -2;
        profile.overworld_section_count = 8;
        profile.block_states.stone = 123;
        let world = SharedWorld::new(ChunkPos { x: 0, z: 0 }, profile).unwrap();
        let chunk = world.chunk_snapshot(ChunkPos { x: 0, z: 0 }).unwrap();
        assert_eq!(chunk.min_section_y(), -2);
        assert_eq!(chunk.sections().len(), 8);
        assert_eq!(
            world.world_block(BlockPos { x: 0, y: 61, z: 0 }).unwrap(),
            BlockStateId::new(123)
        );
    }

    #[test]
    fn local_world_runtime_applies_block_events_through_authoritative_ticks() {
        let mut runtime =
            new_local_world_runtime(ChunkPos { x: 0, z: 0 }, &builtin_world_profile()).unwrap();
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
        let mut runtime =
            new_local_world_runtime(ChunkPos { x: 0, z: 0 }, &builtin_world_profile()).unwrap();
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
    fn sends_block_change_prediction_acknowledgement() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::BlockChangedAck, 0x04).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let mut output = Vec::new();
        send_block_changed_ack(&mut output, &profile, 300).unwrap();
        assert_eq!(output, [3, 0x04, 0xac, 0x02]);
    }

    #[test]
    fn block_interaction_reach_uses_player_eye_to_block_center_distance() {
        let player = PlayerState::new([0.5, 65.0, 0.5], 0.0, 0.0, false, false).unwrap();

        assert!(is_block_interaction_within_reach(
            &player,
            BlockPosition { x: 0, y: 65, z: 0 }
        ));
        assert!(is_block_interaction_within_reach(
            &player,
            BlockPosition { x: 5, y: 65, z: 0 }
        ));
        assert!(!is_block_interaction_within_reach(
            &player,
            BlockPosition { x: 7, y: 65, z: 0 }
        ));
    }

    #[test]
    fn movement_delta_validation_allows_normal_steps_and_rejects_large_jumps() {
        let player = PlayerState::new([0.5, 65.0, 0.5], 0.0, 0.0, false, false).unwrap();

        validate_movement_delta(
            &player,
            PlayerMovement::Position {
                position: [1.0, 65.0, 1.0],
                flags: ferrum_play::MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                },
            },
        )
        .unwrap();
        validate_movement_delta(
            &player,
            PlayerMovement::Rotation {
                yaw: 90.0,
                pitch: 0.0,
                flags: ferrum_play::MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                },
            },
        )
        .unwrap();

        let error = validate_movement_delta(
            &player,
            PlayerMovement::Position {
                position: [200.5, 65.0, 0.5],
                flags: ferrum_play::MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                },
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("movement delta exceeds"));
    }

    #[test]
    fn movement_floor_validation_rejects_flat_floor_penetration() {
        let floor_y = builtin_world_profile().floor_y;
        let minimum_feet_y = f64::from(floor_y) + 1.0;
        validate_movement_floor(
            PlayerMovement::Position {
                position: [0.5, minimum_feet_y, 0.5],
                flags: ferrum_play::MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                },
            },
            floor_y,
        )
        .unwrap();
        validate_movement_floor(
            PlayerMovement::StatusOnly {
                flags: ferrum_play::MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                },
            },
            floor_y,
        )
        .unwrap();

        let error = validate_movement_floor(
            PlayerMovement::Position {
                position: [0.5, minimum_feet_y - 0.01, 0.5],
                flags: ferrum_play::MovementFlags {
                    on_ground: true,
                    horizontal_collision: false,
                },
            },
            floor_y,
        )
        .unwrap_err();
        assert!(error.to_string().contains("below flat-world floor"));
    }

    #[test]
    fn block_interaction_state_checks_treat_world_height_outside_as_unavailable() {
        let world = SharedWorld::static_flat();
        let too_high = BlockPos { x: 0, y: 320, z: 0 };
        let event = WorldEvent::BlockMutation(BlockMutation {
            position: too_high,
            state: BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID),
        });

        assert!(!is_placement_target_air(&world, event).unwrap());
        assert!(!is_break_target_mutable(&world, event).unwrap());
    }

    #[test]
    fn shared_world_broadcasts_mutations_to_other_subscribers_only() {
        let world = SharedWorld::static_flat();
        let first = ConnectionId::new(1);
        let second = ConnectionId::new(2);
        let first_subscription = world.subscribe(first).unwrap();
        let second_subscription = world.subscribe(second).unwrap();
        assert_eq!(world.subscriber_count(), 2);

        let position = BlockPos { x: 2, y: 65, z: 3 };
        let state = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);
        let applied = world
            .apply_event(
                first,
                WorldEvent::BlockMutation(BlockMutation { position, state }),
            )
            .unwrap();

        assert!(world.drain_updates(first, usize::MAX).unwrap().is_empty());
        assert_eq!(world.drain_updates(second, usize::MAX).unwrap(), applied);

        drop(second_subscription);
        assert_eq!(world.subscriber_count(), 1);
        drop(first_subscription);
        assert_eq!(world.subscriber_count(), 0);
    }

    #[test]
    fn shared_world_coalesces_repeated_updates_for_the_same_block() {
        let world = SharedWorld::static_flat();
        let source = ConnectionId::new(1);
        let receiver = ConnectionId::new(2);
        let _receiver_subscription = world.subscribe(receiver).unwrap();
        let position = BlockPos { x: 4, y: 65, z: 4 };
        let stone = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);
        let air = BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID);

        world
            .apply_event(
                source,
                WorldEvent::BlockMutation(BlockMutation {
                    position,
                    state: stone,
                }),
            )
            .unwrap();
        let latest = world
            .apply_event(
                source,
                WorldEvent::BlockMutation(BlockMutation {
                    position,
                    state: air,
                }),
            )
            .unwrap();

        assert_eq!(world.pending_update_count(receiver), 1);
        assert_eq!(world.drain_updates(receiver, 1).unwrap(), latest);
    }

    #[test]
    fn shared_world_bounds_pending_peer_updates() {
        let world = SharedWorld::static_flat();
        let source = ConnectionId::new(1);
        let receiver = ConnectionId::new(2);
        let _receiver_subscription = world.subscribe(receiver).unwrap();
        let stone = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);

        for index in 0..MAX_PENDING_WORLD_UPDATES_PER_CONNECTION + 5 {
            let position = BlockPos {
                x: -16 + i32::try_from(index % 48).unwrap(),
                y: 65,
                z: -16 + i32::try_from(index / 48).unwrap(),
            };
            world
                .apply_event(
                    source,
                    WorldEvent::BlockMutation(BlockMutation {
                        position,
                        state: stone,
                    }),
                )
                .unwrap();
        }

        assert_eq!(
            world.pending_update_count(receiver),
            MAX_PENDING_WORLD_UPDATES_PER_CONNECTION
        );
    }

    #[test]
    fn shared_world_chunk_snapshots_include_authoritative_mutations() {
        let world = SharedWorld::static_flat();
        let position = BlockPos { x: 3, y: 65, z: -4 };
        let state = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);
        world
            .apply_event(
                ConnectionId::new(1),
                WorldEvent::BlockMutation(BlockMutation { position, state }),
            )
            .unwrap();

        let mut snapshot = world.chunk_snapshot(ChunkPos { x: 0, z: -1 }).unwrap();
        assert_eq!(snapshot.world_block(position).unwrap(), state);

        snapshot
            .apply_block_mutation(BlockMutation {
                position,
                state: BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID),
            })
            .unwrap();
        assert_eq!(world.world_block(position).unwrap(), state);
    }

    #[test]
    fn shared_world_chunk_snapshot_reports_missing_chunks() {
        let world = SharedWorld::static_flat();
        let error = world
            .chunk_snapshot(ChunkPos { x: 100, z: 100 })
            .unwrap_err();
        assert!(error.to_string().contains("missing chunk (100, 100)"));
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

    #[test]
    fn play_loop_rejects_large_position_jump_before_chunk_streaming() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets
            .insert(PacketKind::MovePlayerPosition, 0x1e)
            .unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(1);
        let _subscription = world.subscribe(connection).unwrap();

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0x1e, |body| {
                for value in [500.5_f64, 65.0, 0.5] {
                    body.extend_from_slice(&value.to_be_bytes());
                }
                body.push(1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut session = play_session();

        let error = run_play_loop(
            &mut Cursor::new(input),
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap_err();

        assert!(error.to_string().contains("movement delta exceeds"));
        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        assert!(crate::codec::read_packet(&mut output).is_err());
    }

    #[test]
    fn play_loop_rejects_position_below_flat_floor() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets
            .insert(PacketKind::MovePlayerPosition, 0x1e)
            .unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(1);
        let _subscription = world.subscribe(connection).unwrap();

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0x1e, |body| {
                for value in [
                    0.5_f64,
                    f64::from(world.world_profile().floor_y) + 1.0 - 0.01,
                    0.5,
                ] {
                    body.extend_from_slice(&value.to_be_bytes());
                }
                body.push(1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut session = play_session();

        let error = run_play_loop(
            &mut Cursor::new(input),
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap_err();

        assert!(error.to_string().contains("below flat-world floor"));
        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        assert!(crate::codec::read_packet(&mut output).is_err());
    }

    #[test]
    fn play_loop_drains_peer_updates_after_read_timeout() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets.insert(PacketKind::KeepAliveResponse, 0x1c).unwrap();
        packets.insert(PacketKind::BlockUpdate, 0x22).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(2);
        let _subscription = world.subscribe(connection).unwrap();
        let position = BlockPos { x: 1, y: 65, z: 1 };
        let state = BlockStateId::new(version_26_1_2::STONE_BLOCK_STATE_ID);
        world
            .apply_event(
                ConnectionId::new(1),
                WorldEvent::BlockMutation(BlockMutation { position, state }),
            )
            .unwrap();

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0x1c, |body| {
                body.extend_from_slice(&1_i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut reader = TimeoutThenCursor::new(input);
        let mut output = Vec::new();
        let mut session = play_session();

        run_play_loop(
            &mut reader,
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap();

        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        let block_update = crate::codec::read_packet(&mut output).unwrap();
        let mut block_update_reader = PacketReader::new(&block_update);
        assert_eq!(block_update_reader.read_varint().unwrap(), 0x22);
        assert_eq!(
            block_update_reader.read_i64().unwrap(),
            BlockPosition { x: 1, y: 65, z: 1 }.pack_for_test()
        );
        assert_eq!(
            block_update_reader.read_varint().unwrap(),
            i32::try_from(state.get()).unwrap()
        );
    }

    #[test]
    fn play_loop_acknowledges_far_placement_without_mutating_world() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets.insert(PacketKind::KeepAliveResponse, 0x1c).unwrap();
        packets.insert(PacketKind::UseItemOn, 0x42).unwrap();
        packets.insert(PacketKind::BlockChangedAck, 0x04).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(1);
        let _subscription = world.subscribe(connection).unwrap();
        let placement = BlockPos { x: 8, y: 65, z: 0 };

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0x42, |body| {
                write_varint_vec(body, 0);
                body.extend_from_slice(
                    &BlockPosition { x: 7, y: 65, z: 0 }
                        .pack_for_test()
                        .to_be_bytes(),
                );
                write_varint_vec(body, 5);
                for value in [0.5_f32, 0.5, 0.5] {
                    body.extend_from_slice(&value.to_be_bytes());
                }
                body.push(0);
                body.push(0);
                write_varint_vec(body, 9);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x1c, |body| {
                body.extend_from_slice(&1_i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut session = play_session();

        run_play_loop(
            &mut Cursor::new(input),
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap();

        assert_eq!(
            world.world_block(placement).unwrap(),
            BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID)
        );
        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        let acknowledgement = crate::codec::read_packet(&mut output).unwrap();
        let mut acknowledgement_reader = PacketReader::new(&acknowledgement);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 0x04);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 9);
        assert!(crate::codec::read_packet(&mut output).is_err());
    }

    #[test]
    fn play_loop_acknowledges_air_break_without_sending_block_update() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets.insert(PacketKind::KeepAliveResponse, 0x1c).unwrap();
        packets.insert(PacketKind::PlayerAction, 0x29).unwrap();
        packets.insert(PacketKind::BlockChangedAck, 0x04).unwrap();
        packets.insert(PacketKind::BlockUpdate, 0x22).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(1);
        let _subscription = world.subscribe(connection).unwrap();
        let air_position = BlockPos { x: 0, y: 65, z: 0 };

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0x29, |body| {
                write_varint_vec(body, 2);
                body.extend_from_slice(
                    &BlockPosition { x: 0, y: 65, z: 0 }
                        .pack_for_test()
                        .to_be_bytes(),
                );
                body.push(1);
                write_varint_vec(body, 11);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x1c, |body| {
                body.extend_from_slice(&1_i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut session = play_session();

        run_play_loop(
            &mut Cursor::new(input),
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap();

        assert_eq!(
            world.world_block(air_position).unwrap(),
            BlockStateId::new(version_26_1_2::AIR_BLOCK_STATE_ID)
        );
        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        let acknowledgement = crate::codec::read_packet(&mut output).unwrap();
        let mut acknowledgement_reader = PacketReader::new(&acknowledgement);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 0x04);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 11);
        assert!(crate::codec::read_packet(&mut output).is_err());
    }

    #[test]
    fn play_loop_acknowledges_bedrock_break_without_mutating_world() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets.insert(PacketKind::KeepAliveResponse, 0x1c).unwrap();
        packets
            .insert(PacketKind::MovePlayerPosition, 0x1e)
            .unwrap();
        packets.insert(PacketKind::PlayerAction, 0x29).unwrap();
        packets.insert(PacketKind::BlockChangedAck, 0x04).unwrap();
        packets.insert(PacketKind::BlockUpdate, 0x22).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(1);
        let _subscription = world.subscribe(connection).unwrap();
        let bedrock_position = BlockPos { x: 0, y: 60, z: 0 };

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0x1e, |body| {
                for value in [0.5_f64, f64::from(world.world_profile().floor_y) + 1.0, 0.5] {
                    body.extend_from_slice(&value.to_be_bytes());
                }
                body.push(1);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x29, |body| {
                write_varint_vec(body, 2);
                body.extend_from_slice(
                    &BlockPosition { x: 0, y: 60, z: 0 }
                        .pack_for_test()
                        .to_be_bytes(),
                );
                body.push(1);
                write_varint_vec(body, 12);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x1c, |body| {
                body.extend_from_slice(&1_i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut session = play_session();

        run_play_loop(
            &mut Cursor::new(input),
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap();

        assert_eq!(
            world.world_block(bedrock_position).unwrap(),
            BlockStateId::new(version_26_1_2::BEDROCK_BLOCK_STATE_ID)
        );
        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        let acknowledgement = crate::codec::read_packet(&mut output).unwrap();
        let mut acknowledgement_reader = PacketReader::new(&acknowledgement);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 0x04);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 12);
        assert!(crate::codec::read_packet(&mut output).is_err());
    }

    #[test]
    fn play_loop_acknowledges_occupied_placement_without_replacing_block() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets.insert(PacketKind::KeepAliveResponse, 0x1c).unwrap();
        packets.insert(PacketKind::UseItemOn, 0x42).unwrap();
        packets.insert(PacketKind::BlockChangedAck, 0x04).unwrap();
        packets.insert(PacketKind::BlockUpdate, 0x22).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(1);
        let _subscription = world.subscribe(connection).unwrap();
        let occupied = BlockPos { x: 0, y: 63, z: 0 };
        let previous = world.world_block(occupied).unwrap();

        let mut input = Vec::new();
        write_packet(
            &mut input,
            &build_packet(0x42, |body| {
                write_varint_vec(body, 0);
                body.extend_from_slice(
                    &BlockPosition { x: 0, y: 64, z: 0 }
                        .pack_for_test()
                        .to_be_bytes(),
                );
                write_varint_vec(body, 0);
                for value in [0.5_f32, 0.0, 0.5] {
                    body.extend_from_slice(&value.to_be_bytes());
                }
                body.push(0);
                body.push(0);
                write_varint_vec(body, 10);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x1c, |body| {
                body.extend_from_slice(&1_i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut session = play_session();

        run_play_loop(
            &mut Cursor::new(input),
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap();

        assert_eq!(world.world_block(occupied).unwrap(), previous);
        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        let acknowledgement = crate::codec::read_packet(&mut output).unwrap();
        let mut acknowledgement_reader = PacketReader::new(&acknowledgement);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 0x04);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 10);
        assert!(crate::codec::read_packet(&mut output).is_err());
    }

    #[test]
    fn play_loop_acknowledges_height_outside_placement_without_error() {
        let mut packets = PacketTable::new();
        packets.insert(PacketKind::KeepAliveRequest, 0x2c).unwrap();
        packets.insert(PacketKind::KeepAliveResponse, 0x1c).unwrap();
        packets
            .insert(PacketKind::MovePlayerPosition, 0x1e)
            .unwrap();
        packets.insert(PacketKind::UseItemOn, 0x42).unwrap();
        packets.insert(PacketKind::BlockChangedAck, 0x04).unwrap();
        packets.insert(PacketKind::BlockUpdate, 0x22).unwrap();
        let profile = ProtocolProfile::new("Test", 1, packets).unwrap();
        let world = SharedWorld::static_flat();
        let connection = ConnectionId::new(1);
        let _subscription = world.subscribe(connection).unwrap();

        let mut input = Vec::new();
        for y in [160.0_f64, 250.0, 318.0] {
            write_packet(
                &mut input,
                &build_packet(0x1e, |body| {
                    for value in [0.5_f64, y, 0.5] {
                        body.extend_from_slice(&value.to_be_bytes());
                    }
                    body.push(1);
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();
        }
        write_packet(
            &mut input,
            &build_packet(0x42, |body| {
                write_varint_vec(body, 0);
                body.extend_from_slice(
                    &BlockPosition { x: 0, y: 319, z: 0 }
                        .pack_for_test()
                        .to_be_bytes(),
                );
                write_varint_vec(body, 1);
                for value in [0.5_f32, 1.0, 0.5] {
                    body.extend_from_slice(&value.to_be_bytes());
                }
                body.push(0);
                body.push(0);
                write_varint_vec(body, 13);
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        write_packet(
            &mut input,
            &build_packet(0x1c, |body| {
                body.extend_from_slice(&1_i64.to_be_bytes());
                Ok(())
            })
            .unwrap(),
        )
        .unwrap();
        let mut output = Vec::new();
        let mut session = play_session();

        run_play_loop(
            &mut Cursor::new(input),
            &mut output,
            &profile,
            &mut session,
            &world,
            connection,
            Some(1),
        )
        .unwrap();

        let mut output = Cursor::new(output);
        let keep_alive = crate::codec::read_packet(&mut output).unwrap();
        assert_eq!(PacketReader::new(&keep_alive).read_varint().unwrap(), 0x2c);
        let acknowledgement = crate::codec::read_packet(&mut output).unwrap();
        let mut acknowledgement_reader = PacketReader::new(&acknowledgement);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 0x04);
        assert_eq!(acknowledgement_reader.read_varint().unwrap(), 13);
        assert!(crate::codec::read_packet(&mut output).is_err());
    }

    struct TimeoutThenCursor {
        timed_out: bool,
        cursor: Cursor<Vec<u8>>,
    }

    impl TimeoutThenCursor {
        fn new(input: Vec<u8>) -> Self {
            Self {
                timed_out: false,
                cursor: Cursor::new(input),
            }
        }
    }

    impl Read for TimeoutThenCursor {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if !self.timed_out {
                self.timed_out = true;
                return Err(io::Error::new(io::ErrorKind::TimedOut, "test timeout"));
            }
            self.cursor.read(output)
        }
    }

    fn play_session() -> ProtocolSession {
        let mut session = ProtocolSession::new();
        session
            .handshake(1, ferrum_protocol::HandshakeIntent::Login)
            .unwrap();
        session.login_start("Steve").unwrap();
        session.login_success_sent().unwrap();
        session.login_acknowledged().unwrap();
        session.finish_configuration_sent().unwrap();
        session.configuration_acknowledged().unwrap();
        session
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
