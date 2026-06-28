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
- Configuration-state Registry Data, Feature Flags, and Tags payload encoders

The socket runtime now uses `ProtocolProfile` and `ProtocolSession` from Handshake through Configuration. With Configuration explicitly enabled, it consumes Login Acknowledged, sends the configured Feature Flags and Tags payloads, sends Finish Configuration, validates the client acknowledgement, and reaches the Play state. It does not yet send Join Game or world data.

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

- Add a protocol-version mismatch disconnect before Login Success.
- Expand malformed-packet isolation tests as Play handlers grow.

## M11 — Minimal Configuration state

Status: wire-flow foundation implemented; version data remains.

Completed:

- Add a standalone `ferrum-configuration` payload-encoder crate.
- Encode Registry Data entries with optional anonymous NBT values.
- Encode Feature Flags and registry-grouped Tags.
- Consume Login Acknowledged in the live socket runtime.
- Send configured Feature Flags and an empty Tags payload.
- Send Finish Configuration and wait for the client acknowledgement.
- Transition the authoritative connection session to Play.
- Keep the new flow opt-in until a complete target-version registry set exists.

Remaining:

- Select exactly one documented Minecraft Java Edition protocol version.
- Add its required registry datasets and packet IDs.
- Send Registry Data packets in the required order.
- Implement known-packs negotiation when required by the selected version.
- Add captured-client integration fixtures for the complete Configuration exchange.

## M12 — Minimal Play state

- Send Login/Join Game data for one static dimension.
- Send spawn position and player-position synchronization.
- Send one static in-memory chunk.
- Implement Keep Alive on the live socket path.
- Implement system messages and graceful disconnect.
- Add integration fixtures that replay a real client packet sequence.

## M13 — Persistent world foundation

- Add region-independent world coordinates and block-state IDs.
- Implement chunk sections and palette containers.
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
