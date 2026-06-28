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
- Versioned protocol profiles and deterministic connection state validation

The socket runtime now uses `ProtocolProfile` and `ProtocolSession` for Handshake, Status, and Login. It does not yet consume Login Acknowledged or enter Configuration and Play on the wire.

## M9 — Binary NBT foundation

Status: merged into `master`.

- Add a standalone `ferrum-nbt` workspace crate.
- Support all standard binary NBT tag types.
- Encode named and unnamed roots.
- Decode untrusted input with depth, string, and collection limits.
- Keep compound encoding deterministic with `BTreeMap` ordering.
- Reject heterogeneous lists, invalid tag IDs, negative lengths, invalid named `TAG_End`, truncated input, and limit violations.
- Include exact-byte and round-trip tests.

This milestone is required before registry data, dimension data, chunk data, and Anvil world compatibility can be implemented cleanly.

## M10 — Protocol state machine

Status: core model and existing socket-flow integration implemented.

Completed:

- Model Handshake, Status, Login, Configuration, Play, and Closed phases.
- Keep version-specific packet IDs in `ProtocolProfile` and `PacketTable`.
- Resolve packet IDs by phase and direction.
- Reject packet-ID collisions inside the same phase and direction.
- Validate Login Acknowledged and Configuration Acknowledged ordering.
- Track pending Keep Alive requests and validate responses.
- Reject invalid phase transitions without mutating the connection state.
- Cover the complete Login → Configuration → Play sequence with unit tests.
- Build a protocol profile from the current `server.toml` packet table at startup.
- Drive Handshake, Status, Ping/Pong, Login Start, Login Success, and Login Disconnect through `ProtocolSession`.
- Preserve configured packet-ID tests through the new profile layer.

Remaining:

- Add a deterministic disconnect path for unsupported protocol versions.
- Consume Login Acknowledged in the socket runtime.
- Keep malformed packets isolated to the originating connection as Configuration and Play handlers grow.

## M11 — Minimal Configuration state

- Send known-packs negotiation where required by the selected protocol version.
- Send registry data backed by `ferrum-nbt`.
- Send enabled features and tags.
- Finish Configuration and wait for the client acknowledgement.
- Target exactly one documented Minecraft Java Edition version before adding adapters for other versions.

## M12 — Minimal Play state

- Send Login/Join Game data for one static dimension.
- Send spawn position and player-position synchronization.
- Send one static in-memory chunk.
- Implement Keep Alive.
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
