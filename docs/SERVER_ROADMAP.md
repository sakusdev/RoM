# RoM Server Roadmap

RoM is a native Rust implementation of a Minecraft Java Edition-compatible server. It is distributed as platform-native binaries and must not require Java or a JVM at runtime.

## Current implementation

The current server milestone provides:

- Handshake parsing
- Server-list status response
- Ping/pong latency response
- Configurable packet IDs and protocol metadata
- Offline-mode UUID derivation
- Minimal Login Success support
- Explicit login disconnect fallback
- Independent connection workers
- VarInt, framed-packet, string, and packet-reader primitives
- Deterministic binary NBT encoding and decoding
- Protocol-anonymous NBT encoding and decoding
- Versioned protocol profiles and deterministic connection state validation
- Configuration-state Registry Data, Feature Flags, Tags, and Known Packs codecs
- Deterministic Play-state payload codecs
- A built-in Minecraft Java Edition 26.1.2 profile (protocol 775)
- Static-overworld Join Game, default spawn, player-position synchronization, and Keep Alive
- Palette-encoded in-memory flat chunks with full skylight and chunk-batch negotiation
- All four serverbound movement packet forms with bounded decoding
- Bounded serverbound player-action and use-item-on-block payload decoding
- Shared in-memory world-runtime state for decoded block mutation events
- Clientbound block-update payload encoding and optional local mutation writeback for profiles that expose that packet ID
- Per-connection player position, rotation, movement flags, and current-chunk state
- Deterministic 3×3 chunk views with cache-center updates, new chunk batches, and unloads
- Version-neutral 20 TPS tick scheduling with capped catch-up
- Globally bounded, per-connection sequenced input queues
- Deterministic fair per-tick input draining and mutation budgets
- Welcome system chat and graceful Play disconnects
- Live online-player count in server-list status responses
- Version-neutral in-memory world coordinates, sections, block states, biome IDs, and chunk views

The socket runtime now uses `ProtocolProfile` and `ProtocolSession` from Handshake through Play. The 26.1.2 profile validates the handshake protocol, negotiates the vanilla core known pack, sends the full synchronized registry set, completes Configuration, sends the flat-overworld Play bootstrap, validates teleport and chunk-batch acknowledgements, and enters a movement-aware Play loop. The server tracks each player's authoritative position and rotation, updates the chunk-cache center only after crossing a chunk boundary, sends newly visible chunks, unloads chunks that leave the 3×3 view, keeps the server-list online-player count synchronized with Play connections, and continues Keep Alive validation while processing movement. The standalone `ferrum-runtime` crate now provides deterministic scheduling and bounded input-ordering primitives, and the Play loop can route decoded block mutation events through shared in-memory world state. If the active profile exposes a clientbound Block Update packet ID, accepted mutations are written back to the acting client. Entities, dedicated network-worker queues, verified 26.1.2 block interaction packet IDs, multi-client block update broadcasting, and persistence are not implemented yet.

## M9 — Binary NBT foundation

Status: complete.

- Add a standalone `ferrum-nbt` workspace crate.
- Support all standard binary NBT tag types.
- Encode named roots, empty-name roots, and protocol-anonymous roots.
- Decode untrusted input with depth, string, and collection limits.
- Keep compound encoding deterministic with `BTreeMap` ordering.
- Reject heterogeneous lists, invalid tag IDs, negative lengths, invalid `TAG_End` roots, truncated input, and limit violations.
- Include exact-byte and round-trip tests.

## M10 — Protocol state machine

Status: complete for the implemented wire states.

- Model Handshake, Status, Login, Configuration, Play, and Closed phases.
- Keep version-specific packet IDs in `ProtocolProfile` and `PacketTable`.
- Resolve packet IDs by phase and direction.
- Reject packet-ID collisions inside the same phase and direction.
- Validate Login Acknowledged and Configuration Acknowledged ordering.
- Track pending Keep Alive requests and validate responses.
- Reject invalid phase transitions without mutating the connection state.
- Build a protocol profile from the current `server.toml` packet table at startup.
- Drive Handshake, Status, Ping/Pong, Login, and Configuration through `ProtocolSession`.
- Preserve custom packet-ID support through the profile layer.

Remaining cross-cutting work:

- Expand malformed-packet isolation tests as Play handlers grow.

## M11 — Minimal Configuration state

Status: complete for the selected 26.1.2 configuration flow.

Completed:

- Add a standalone `ferrum-configuration` payload-encoder crate.
- Encode Registry Data entries with optional anonymous NBT values.
- Encode Feature Flags and registry-grouped Tags.
- Consume Login Acknowledged in the live socket runtime.
- Send configured Feature Flags and an empty Tags payload.
- Send Finish Configuration and wait for the client acknowledgement.
- Transition the authoritative connection session to Play.
- Keep the manual flow opt-in until a complete target-version registry set exists.
- Select Minecraft Java Edition 26.1.2 / protocol 775 as the first concrete target.
- Add an exact built-in packet table for the implemented states.
- Disconnect mismatched clients before Login Start is consumed.
- Negotiate the vanilla `minecraft/core/26.1.2` known pack with bounded response decoding.
- Accept the vanilla `minecraft:brand` Configuration custom payload before Known Packs.
- Add all 28 required 26.1.2 synchronized registries and 382 entry identifiers.
- Send Registry Data packets in deterministic order after Feature Flags and before Tags.
- Disconnect during Configuration when the required core pack is declined.

Remaining cross-version validation:

- Add captured-client integration fixtures for the complete Configuration exchange.

## M12 — Minimal Play state

Status: complete for the first 26.1.2 Play bootstrap milestone.

Completed:

- Add a standalone deterministic `ferrum-play` payload-codec crate.
- Send Login/Join Game data for one static overworld dimension.
- Send default spawn and absolute player-position synchronization.
- Validate the client's teleport acknowledgement and reject the wrong teleport ID.
- Implement Keep Alive on the live socket path with exact response validation.
- Keep test execution finite while the real TCP path continues Keep Alive rounds.
- Send the initial chunk cache center and deterministic flat in-memory chunk.
- Encode block-state and biome palette containers in vanilla section order.
- Send full skylight data and negotiate Chunk Batch Start/Finished/Received.
- Send a welcome system message and a graceful Play disconnect on bootstrap failure.

Remaining cross-client validation:

- Add integration fixtures captured from a real 26.1.2 client.
- Confirm the complete join sequence against the vanilla client.
- Verify initial chunk rendering, skylight, and spawn stability in a manual vanilla-client smoke test.

## M13 — Player movement and chunk-view streaming

Status: complete for the first per-connection 26.1.2 movement milestone.

Completed:

- Add exact protocol-775 packet IDs for Client Tick End and all four serverbound movement packets.
- Decode position, rotation, position-plus-rotation, and status-only movement payloads with exact lengths.
- Reject NaN, infinity, coordinates outside the supported world range, unknown movement-flag bits, and trailing bytes.
- Decode and validate serverbound block interaction payloads once a profile exposes their packet IDs.
- Convert validated block break/place interactions into version-neutral `WorldEvent` mutations.
- Reject movement received before the initial teleport acknowledgement.
- Store authoritative per-connection position, yaw, pitch, on-ground state, and horizontal-collision state.
- Convert player coordinates to chunk coordinates correctly across negative boundaries.
- Add deterministic `ChunkView` reconciliation backed by ordered sets.
- Keep a 3×3 visible chunk set centered on the player's current chunk.
- Update the client chunk-cache center only when the player crosses a chunk boundary.
- Batch only newly visible flat chunks and send Forget Level Chunk for chunks leaving the view.
- Continue accepting movement and chunk acknowledgements while Keep Alive is pending.
- Use Client Tick End packets to advance the per-connection Keep Alive cadence without blocking movement handling.
- Add exact-byte, malformed-input, boundary-crossing, and deterministic-set tests.

Remaining movement work:

- Add collision, movement-speed, and fall-state validation.
- Broadcast player state to other connected clients.

## M14 — Authoritative tick/runtime foundation

Status: complete for version-neutral scheduling and input-ordering primitives.

Completed:

- Add a standalone `ferrum-runtime` crate with no Minecraft wire-format dependency.
- Add stable `Tick`, `ConnectionId`, and per-connection `InputSequence` identifiers.
- Add a fixed-rate clock with a standard 20 TPS server constructor.
- Cap catch-up work and explicitly report skipped overdue ticks to prevent an unbounded catch-up spiral.
- Add a globally bounded input queue with explicit overflow errors.
- Assign independent monotonic sequence numbers to each connection.
- Drain queued input in deterministic fair rounds ordered by connection ID.
- Enforce a configurable maximum number of mutations per tick.
- Remove queued input and reset sequence state when a connection is removed.
- Add a generic deterministic state runner for applying envelopes at an authoritative tick.
- Cover timing, overload, fairness, queue limits, disconnect cleanup, and mutation order with tests.

Remaining integration work:

- Move packet readers into independent network workers that publish bounded input envelopes.
- Run one shared 20 TPS world loop instead of per-connection timing.
- Route resulting chunk, movement, Keep Alive, and disconnect output back to connection writers.
- Ensure a slow reader or writer cannot block unrelated players.
- Add integration tests for deterministic ordering across multiple simulated connections.

## M15 — Persistent world foundation

Status: started with deterministic in-memory primitives.

Completed foundation:

- Add a standalone `ferrum-world` crate.
- Add region-independent chunk coordinates and version-neutral block-state and biome IDs.
- Implement mutable 16×16×16 chunk sections and 4×4×4 biome containers.
- Track non-air block counts incrementally.
- Build deterministic four-layer flat overworld chunks at arbitrary chunk positions.
- Apply deterministic world-coordinate block mutations with chunk/local coordinate reporting.
- Store loaded chunks in deterministic coordinate order and route world-coordinate mutations to the owning chunk.
- Expose runtime-compatible world events and verify loaded chunks mutate through `DeterministicRuntime` in authoritative order.
- Enqueue decoded block break/place events into the live Play path's deterministic world runtime.
- Move accepted block mutations from per-connection local stores into shared server world state.
- Encode clientbound Block Update packets and send accepted mutations when the profile exposes `BlockUpdate`.
- Keep protocol serialization and version-specific numeric IDs outside the world crate.

Remaining:

- Verify exact 26.1.2 packet IDs for Player Action, Use Item On, and Block Update before adding them to the built-in profile.
- Replace the current shared mutex world path with dedicated bounded network-worker queues and one authoritative tick owner.
- Broadcast accepted block mutations back to affected clients.
- Wire live network workers into the shared authoritative world runtime.
- Add NBT-backed Anvil region reading.
- Add safe asynchronous loading and saving.
- Preserve deterministic world mutation through the authoritative tick loop.

## Development policy

- Do not attempt all Minecraft versions at once.
- Do not scatter packet IDs throughout gameplay code.
- Do not treat generated Mojang code as publishable original source.
- Network I/O may be concurrent, but authoritative world mutations must be ordered and deterministic.
- Bound total queued input and per-tick work so one connection cannot starve the server.
- Cap tick catch-up rather than allowing overload to grow without limit.
- Every externally supplied length must have a limit before allocation.
- Reject non-finite or unreasonable movement input before mutating player state.
- Add tests for exact wire bytes in addition to round-trip tests.
