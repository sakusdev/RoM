# RoM

**RoM is an experimental Minecraft Java Edition-compatible server written in Rust.**

> **NOT AN OFFICIAL MINECRAFT PRODUCT.**  
> **NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.**

RoM is distributed as a native executable rather than a JAR and does not require Java or a JVM at runtime. The current target is **Minecraft Java Edition 26.1.2** using protocol **775**.

> [!WARNING]
> RoM is under active development. It can complete the initial vanilla connection flow and enter a small deterministic test world, but it is not yet a replacement for a production Minecraft server.

## Distribution model

RoM follows a Fabric-installer-style bootstrap model while preserving its native Rust architecture:

1. RoM releases contain original RoM code and documentation only.
2. `rom-bootstrap` resolves the selected Minecraft version from official metadata.
3. The official server JAR is downloaded directly on the user's machine from an official Mojang/Microsoft HTTPS endpoint.
4. Its size and SHA-1 are checked against official metadata.
5. The JAR remains in a local cache and is not bundled into RoM releases.
6. RoM scans only synchronized-registry JSON resources from the locally obtained game JAR.
7. A deterministic, integrity-protected `.rompack` records registry IDs, source-resource hashes, source hashes, and patch-set identity.
8. The native `rom-server` validates that pack against its built-in 26.1.2 profile before starting.

The current bootstrap implementation supports the **version pack generated** stage. It does not decompile, translate, execute, bytecode-patch, or redistribute the official server JAR. The generated pack contains derived registry identifiers and source-resource hashes, not copied JSON payloads.

See [`docs/BOOTSTRAP.md`](docs/BOOTSTRAP.md) and [`NOTICE.md`](NOTICE.md).

## Current status

The native server currently supports:

- Handshake, server-list status, and ping/pong
- Strict protocol validation for Minecraft Java Edition 26.1.2
- Offline-mode login with Java-compatible offline UUIDs
- Login Acknowledged and Configuration-state transitions
- Vanilla `minecraft/core/26.1.2` Known Packs negotiation
- Packet IDs, world height, flat-world block-state IDs, and biome ID loaded from the generated schema-v3 `.rompack` during Bootstrap startup
- All 28 synchronized 26.1.2 registries with 382 vanilla entries
- Feature Flags, Tags, and Finish Configuration
- Join Game for a deterministic flat overworld
- Default spawn and absolute player-position synchronization
- Teleport acknowledgement validation
- All four serverbound movement forms
- Finite coordinate, coordinate-range, movement-flag, and exact-payload validation
- Bounded single-packet player movement delta validation
- Flat-floor penetration rejection for position movement
- Bounded protocol-775 Player Action and Use Item On decoding
- Use Item On cursor finite and in-block range validation
- Basic player-eye-to-block-center reach validation for block interactions
- Non-air validation for simplified block breaking
- Bedrock protection for simplified block breaking
- Air-only validation for simplified block placement
- Ack-only handling for world-height-outside block interactions
- Shared in-memory world state across Play connections
- Initial and dynamic chunks serialized from authoritative world snapshots
- Simplified block breaking and adjacent-face stone placement
- Block Update and Block Changed Ack responses
- Bounded per-connection peer update queues with same-position coalescing
- Peer block-update draining after inbound Play packets and transient read timeouts
- Deterministic 3×3 chunk views
- Dynamic chunk loading and unloading
- Live online-player count in status responses
- System chat and Keep Alive validation
- Version-neutral 20 TPS scheduling and bounded deterministic input primitives

The implemented connection flow is:

```text
Handshake
→ Status or Login
→ Login Success
→ Login Acknowledged
→ Known Packs
→ Feature Flags
→ Registry Data
→ Tags
→ Finish Configuration
→ Join Game
→ Initial Chunk Batch
→ Player Position
→ Teleport Acknowledgement
→ 3×3 Chunk View
→ Player Movement
→ Dynamic Chunk Load / Unload
→ Block Break / Simplified Placement
→ Block Update / Prediction Ack
→ Keep Alive
```

## Current limitations

The world is intentionally small and deterministic while the gameplay foundation is being built.

Not implemented yet:

- Microsoft account authentication and encrypted online mode
- Full collision, full movement-speed, and full reach enforcement
- Dedicated network-worker to authoritative-world-runtime queues
- Full item, inventory, replaceability, reach, collision, and game-mode validation
- Dedicated outbound writer workers
- Entities and entity tracking
- Inventory and container behavior
- Procedural world generation
- World persistence and Anvil region saving
- Multi-version gameplay compatibility
- Fabric, Bukkit, Spigot, or Paper plugin compatibility
- Runtime replacement of remaining dimension registry payloads and other gameplay constants with generated pack data

> [!CAUTION]
> The default configuration binds to loopback and uses development-only offline login. Do not expose that configuration to the public internet. It does not prove that a connecting user owns Minecraft.

## Quick start with RoM Bootstrap

### 1. Build the bootstrapper and native server

Use the Rust toolchain selected by `rust-toolchain.toml`:

```bash
cargo build --locked --release -p rom-bootstrap -p ferrum-server
```

### 2. Prepare a local instance

Review the Minecraft EULA, then explicitly acknowledge it:

```bash
./target/release/rom-bootstrap prepare \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula
```

This downloads the official server JAR directly from an official endpoint, validates the official version metadata and JAR SHA-1, and writes local provenance records. RoM does not execute the JAR.

### 3. Generate the local version pack

```bash
./target/release/rom-bootstrap generate \
  --instance ./rom-instance
```

The extractor opens the verified local JAR, resolves the bundled game JAR when present, validates all selected JSON resources, derives the synchronized-registry identifiers, adds the exact semantic packet table, compares both with the built-in 26.1.2 profile, and writes an integrity-protected schema-v3 `.rompack`. Existing schema-v1/v2 packs must be regenerated with `generate --force`.

### 4. Install the native server

```bash
./target/release/rom-bootstrap install-local \
  --instance ./rom-instance \
  --workspace .
```

### 5. Inspect and run

```bash
./target/release/rom-bootstrap status --instance ./rom-instance
./target/release/rom-bootstrap run --instance ./rom-instance
```

The instance defaults to:

```toml
bind = "127.0.0.1:25565"
online_mode = false
```

Connect a matching Minecraft Java Edition 26.1.2 client to `127.0.0.1:25565` for local development testing.

## Direct developer start

The native server can still be built and run directly when working on RoM itself:

```bash
cargo build --locked --release -p ferrum-server
./target/release/ferrum-server --config examples/server-26.1.2.toml
```

On Windows:

```powershell
cargo build --locked --release -p ferrum-server
.\target\release\ferrum-server.exe --config examples\server-26.1.2.toml
```

## Bootstrap instance layout

```text
rom-instance/
├── bin/
│   └── ferrum-server
├── cache/
│   └── official/
│       └── 26.1.2/
│           └── server.jar
├── versions/
│   └── 26.1.2/
│       ├── 26.1.2.rompack
│       └── rompack.json
├── eula.txt
├── NOTICE.txt
├── rom-bootstrap.json
└── server.toml
```

The cache and generated instance data are local artifacts. Do not commit or redistribute instance directories containing official Minecraft files.

## Native releases

`ferrum-server` and `rom-bootstrap` are released as platform-native executables for:

- Windows x86_64
- Linux x86_64
- Linux ARM64
- macOS x86_64
- macOS ARM64

Each release archive contains both binaries, `server.toml`, the Bootstrap guide, README, NOTICE, LICENSE, and VERSION. Standalone binaries are also published separately. Official Minecraft files and locally generated `.rompack` files are never bundled.

Preferred first-run usage from an extracted release archive:

```bash
./rom-bootstrap prepare --instance ./rom-instance --version 26.1.2 --accept-minecraft-eula
./rom-bootstrap generate --instance ./rom-instance
./rom-bootstrap install-local --instance ./rom-instance --server-binary ./ferrum-server
./rom-bootstrap run --instance ./rom-instance
```

Expected direct server usage:

```bash
ferrum-server.exe --config server.toml
./ferrum-server --config server.toml
```

## Architecture

RoM separates version-independent server behavior from version-specific wire metadata.

Core crates:

- `rom-bootstrap` — official-source verification, bounded local extraction, and instance management
- `ferrum-rompack` — deterministic packet/profile metadata encoding, integrity validation, and bounded decoding
- `ferrum-server` — native TCP server and connection runtime
- `ferrum-runtime` — fixed-rate ticks, bounded inputs, and deterministic mutation ordering
- `ferrum-protocol` — packet tables, phases, and connection-state validation
- `ferrum-configuration` — Configuration-state payload codecs
- `ferrum-play` — bounded Play-state wire codecs and movement decoding
- `ferrum-world` — version-neutral chunks, player views, and world primitives
- `ferrum-nbt` — bounded binary NBT encoding and decoding
- `ferrum-version-26-1-2` — exact protocol 775 metadata and registry manifests

Design rules:

- No JVM dependency in the released server runtime
- No official Minecraft files in RoM source or release archives
- Official local source artifacts must be downloaded from approved HTTPS hosts and hash-verified
- Every generated version pack must record source hashes and patch-set identity
- Version-specific packet IDs stay outside gameplay code
- Untrusted lengths are bounded before allocation
- NaN, infinity, invalid movement flags, and unreasonable coordinates are rejected
- Connection inputs and peer updates are bounded
- Authoritative state transitions remain deterministic
- Wire codecs use exact-byte fixtures
- Unsupported input fails explicitly instead of being guessed

## Roadmap

The next server and bootstrap milestones are:

1. Move remaining dimension registry payloads and gameplay constants into generated packs
2. Wire dedicated network workers into the authoritative 20 TPS runtime
3. Add full block interaction and inventory validation
4. Add entities and entity tracking
5. Add persistent Anvil region loading and saving
6. Add Microsoft account authentication and encrypted online mode
7. Add additional Minecraft version profiles

See [`docs/SERVER_ROADMAP.md`](docs/SERVER_ROADMAP.md) for the detailed server plan.

## Development

Run the full validation suite with:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The repository also contains JVM/JAR analysis tooling used for development research. Those tools are not part of the server runtime and must not be used to publish copied game source or proprietary binaries.

## Legal boundary

RoM is independently written and is not affiliated with Mojang Studios or Microsoft. The project does not grant rights to Minecraft software, data, names, or assets. Operators and contributors are responsible for reviewing the Minecraft EULA, Usage Guidelines, applicable platform terms, and local law.
