# Ferrum Server Roadmap

Ferrum Server is a native Rust implementation of a Minecraft Java Edition-compatible server. It is distributed as platform-native binaries and must not require Java or a JVM at runtime.

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
- One palette-encoded in-memory flat chunk with full skylight and chunk-batch negotiation
- Welcome system chat and graceful Play disconnects
- Version-neutral in-memory world coordinates, sections, block states, and biome IDs

The socket runtime now uses `ProtocolProfile` and `ProtocolSession` from Handshake through Play. The 26.1.2 profile validates the handshake protocol, negotiates the vanilla core known pack, sends the full synchronized registry set, completes Configuration, sends a static-overworld Play bootstrap and one flat chunk, negotiates the chunk batch, validates the teleport acknowledgement, emits a welcome system message, and runs Keep Alive exchanges. The world remains a single immutable in-memory chunk; movement, interactions, entities, streaming, and persistence are not implemented yet.

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
- Add all 28 required 26.1.2 synchronized registries and 382 entry identifiers.
- Send Registry Data packets in deterministic order after Feature Flags and before Tags.
- Disconnect during Configuration when the required core pack is declined.

Remaining cross-version validation:

- Add captured-client integration fixtures for the complete Configuration exchange.

## M12 — Minimal Play state

Status: complete for the first single-chunk 26.1.2 Play milestone.

Completed:

- Add a standalone deterministic `ferrum-play` payload-codec crate.
- Send Login/Join Game data for one static overworld dimension.
- Send default spawn and absolute player-position synchronization.
- Validate the client's teleport acknowledgement and reject the wrong teleport ID.
- Implement Keep Alive on the live socket path with exact response validation.
- Keep test execution finite while the real TCP path continues periodic Keep Alive rounds.
- Send the chunk cache center and one deterministic flat in-memory chunk.
- Encode block-state and biome palette containers in vanilla section order.
- Send full skylight data and negotiate Chunk Batch Start/Finished/Received.
- Send a welcome system message and a graceful Play disconnect on bootstrap failure.

Remaining cross-client validation:

- Add integration fixtures captured from a real 26.1.2 client.
- Confirm the complete single-chunk join sequence against the vanilla client.
- Verify initial chunk rendering, skylight, and spawn stability in a manual vanilla-client smoke test.

## M13 — Persistent world foundation

Status: started with deterministic in-memory primitives.

Completed foundation:

- Add a standalone `ferrum-world` crate.
- Add region-independent chunk coordinates and version-neutral block-state and biome IDs.
- Implement mutable 16×16×16 chunk sections and 4×4×4 biome containers.
- Track non-air block counts incrementally.
- Build a deterministic four-layer flat overworld chunk.
- Keep protocol serialization and version-specific numeric IDs outside the world crate.

Remaining:

- Add authoritative player movement and chunk subscription state.
- Stream and unload chunks as the view center changes.
- Add block mutation and interaction handling.
- Add NBT-backed Anvil region reading.
- Add safe asynchronous loading and saving.
- Preserve deterministic world mutation through the authoritative tick loop.

## Development policy

- Do not attempt all Minecraft versions at once.
- Do not scatter packet IDs throughout gameplay code.
- Do not treat generated Mojang code as publishable original source.
- Network I/O may be asynchronous, but authoritative world mutations must be ordered and deterministic.
- Every externally supplied length must have a limit before allocation.
- Add tests for exact wire bytes in addition to round-trip tests.
