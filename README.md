# RoM

**RoM is an experimental Minecraft Java Edition-compatible server written in Rust.**

> **NOT AN OFFICIAL MINECRAFT PRODUCT.**  
> **NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.**

RoM is distributed as a native executable rather than a JAR and does not require Java or a JVM at runtime. The current target is **Minecraft Java Edition 26.1.2** using protocol **775**.

> [!WARNING]
> The supported 26.1.2 Bootstrap flow is complete and produces a runnable local development instance. RoM gameplay remains experimental and is not yet a replacement for a production Minecraft server.

## Distribution model

RoM follows a Fabric-installer-style bootstrap model while preserving its native Rust architecture:

1. RoM releases contain original RoM code and documentation only.
2. `rom-bootstrap` resolves the selected Minecraft version from official metadata.
3. The official server JAR is downloaded directly on the user's machine from an official Mojang/Microsoft HTTPS endpoint.
4. Its size and SHA-1 are checked against official metadata.
5. The JAR remains in a local cache and is not bundled into RoM releases.
6. RoM scans only synchronized-registry JSON resources from the locally obtained game JAR.
7. A deterministic, integrity-protected `.rompack` records registry IDs, source-resource hashes, source hashes, and patch-set identity.
8. The native `ferrum-server` validates that pack against its built-in 26.1.2 generation profile before starting.

The supported Bootstrap workflow is ready-to-run: `setup` performs source preparation, version-pack generation, native installation, and readiness validation. It does not decompile, translate, execute, bytecode-patch, or redistribute the official server JAR. The generated pack contains derived registry identifiers and source-resource hashes, not copied JSON payloads.

See [`docs/BOOTSTRAP.md`](docs/BOOTSTRAP.md) and [`NOTICE.md`](NOTICE.md).

## Current status

The native server currently supports:

- Handshake, server-list status, and ping/pong
- Strict protocol validation for Minecraft Java Edition 26.1.2
- Offline-mode login with Java-compatible offline UUIDs
- Login Acknowledged and Configuration-state transitions
- Vanilla `minecraft/core/26.1.2` Known Packs negotiation
- Packet IDs, world height, dimension/bootstrap metadata, flat-world block-state IDs, biome ID, and Configuration registry payloads loaded from the generated schema-v4 `.rompack` during Bootstrap startup
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
- Bounded configurable chunk views
- Dynamic chunk loading and unloading
- Live online-player count in status responses
- Configurable system chat and Keep Alive validation
- Version-neutral 20 TPS scheduling and bounded deterministic input primitives
- Generic bounded worker command channels and independently bounded non-blocking connection outputs
- Dedicated live Play writer workers
- Protocol-aware Keep Alive and disconnect encoding for live Play writer outputs
- Authoritative runtime Keep Alive requests routed through semantic Play output queues
- Optional loading of Minecraft Anvil region files and region directories

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
→ Configured Bounded Chunk View
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
- Full item, inventory, replaceability, collision, and game-mode validation
- Entities and entity tracking
- Inventory and container behavior
- Procedural world generation
- World persistence and Anvil region saving
- Multi-version gameplay compatibility
- Fabric, Bukkit, Spigot, or Paper plugin compatibility

> [!CAUTION]
> The default configuration binds to loopback and uses development-only offline login. Do not expose that configuration to the public internet. It does not prove that a connecting user owns Minecraft.

## Quick start with RoM Bootstrap

`setup` is the preferred path. It is idempotent: verified downloads and generated packs are reused unless their force flags are supplied.

### Extracted native release

The release archive places `rom-bootstrap` and `ferrum-server` beside each other, so the server binary is detected automatically:

```bash
./rom-bootstrap setup \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula
./rom-bootstrap doctor --instance ./rom-instance
./rom-bootstrap run --instance ./rom-instance
```

On Windows, use `rom-bootstrap.exe` and `ferrum-server.exe`.

### Source checkout

Build both native programs, then let Bootstrap install the server from the workspace:

```bash
cargo build --locked --release -p rom-bootstrap -p ferrum-server
./target/release/rom-bootstrap setup \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula \
  --workspace .
./target/release/rom-bootstrap doctor --instance ./rom-instance
./target/release/rom-bootstrap run --instance ./rom-instance
```

`doctor` prints every missing or invalid component and exits unsuccessfully until the instance is runnable. Use `--json` with `setup`, `doctor`, or `status` for automation. The individual `prepare`, `generate`, and `install-local` stages remain available for debugging and advanced workflows; see [`docs/BOOTSTRAP.md`](docs/BOOTSTRAP.md).

The generated default configuration binds to `127.0.0.1:25565`, uses offline-mode development login, a bounded chunk view, and the deterministic local world. Connect a matching Minecraft Java Edition 26.1.2 client for local testing.

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
./rom-bootstrap setup --instance ./rom-instance --version 26.1.2 --accept-minecraft-eula
./rom-bootstrap doctor --instance ./rom-instance
./rom-bootstrap run --instance ./rom-instance
```

`setup` automatically detects the adjacent `ferrum-server` executable. Pass `--server-binary` only when the binary is stored elsewhere.

Expected direct server usage:

```bash
ferrum-server.exe --config server.toml
./ferrum-server --config server.toml
```

## Architecture

RoM separates version-independent server behavior from version-specific wire metadata.

Core crates:

- `rom-bootstrap` — one-command setup, readiness diagnostics, official-source verification, bounded local extraction, and instance management
- `ferrum-rompack` — deterministic packet/profile metadata encoding, integrity validation, and bounded decoding
- `ferrum-server` — native TCP server and connection runtime
- `ferrum-runtime` — fixed-rate ticks, bounded inputs, deterministic mutation ordering, and bounded worker channels
- `ferrum-protocol` — packet tables, phases, and connection-state validation
- `ferrum-configuration` — Configuration-state payload codecs
- `ferrum-play` — bounded Play-state wire codecs and movement decoding
- `ferrum-world` — version-neutral chunks, player views, Anvil loading, and world primitives
