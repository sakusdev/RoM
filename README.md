# RoM

**RoM is an experimental Minecraft Java Edition server written in Rust.**

It is distributed as a native executable rather than a JAR and does not require Java or a JVM at runtime. The current target is **Minecraft Java Edition 26.1.2** using protocol **775**.

> [!WARNING]
> RoM is under active development. It can complete the initial vanilla connection flow and enter a small deterministic test world, but it is not yet a replacement for a production Minecraft server.

## Current status

The native server currently supports:

- Handshake, server-list status, and ping/pong
- Strict protocol validation for Minecraft Java Edition 26.1.2
- Offline-mode login with Java-compatible offline UUIDs
- Login Acknowledged and Configuration-state transitions
- Vanilla `minecraft/core/26.1.2` Known Packs negotiation
- All 28 synchronized 26.1.2 registries with 382 vanilla entries
- Feature Flags, Tags, and Finish Configuration
- Join Game for a deterministic flat overworld
- Default spawn and absolute player-position synchronization
- Teleport acknowledgement validation
- All four serverbound movement forms: position, rotation, position + rotation, and status-only
- Finite coordinate, coordinate-range, movement-flag, and exact-payload validation
- Bounded serverbound block interaction payload decoding and local deterministic mutation application
- Clientbound block-update payload encoding for accepted local mutations when the active protocol profile exposes that packet ID
- Per-connection authoritative `PlayerState`
- Deterministic 3×3 chunk views around the player's current chunk
- Chunk-cache-center updates only when the player crosses a chunk boundary
- Newly visible flat-chunk batches and Forget Level Chunk unloads
- Chunk-batch acknowledgement validation
- Live online-player count in server-list status responses
- System chat messages
- Live Keep Alive request/response validation while movement packets continue to be processed
- Graceful Play disconnects when bootstrap or movement validation fails
- Version-neutral 20 TPS scheduling, bounded input queues, and deterministic per-tick mutation primitives

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
→ Keep Alive
```

## Current limitations

The world is intentionally small and deterministic while the gameplay foundation is being built.

Not implemented yet:

- Collision or movement-speed enforcement
- Wiring network workers and shared world state into the authoritative runtime
- Verified 26.1.2 packet IDs for live block breaking and placement packets in the built-in profile
- Multi-client broadcasting of block breaking and placement results
- Entities and entity tracking
- Inventory and container behavior
- Procedural world generation
- World persistence and Anvil region saving
- Authentication and encrypted online mode
- Multi-version gameplay compatibility
- Fabric, Bukkit, Spigot, or Paper plugin compatibility

## Quick start

### 1. Build the server

Use the Rust toolchain selected by `rust-toolchain.toml`:

```bash
cargo build --release -p ferrum-server
```

The binary is created at:

```text
target/release/ferrum-server
```

On Windows:

```text
target\release\ferrum-server.exe
```

### 2. Create `server.toml`

```toml
[server]
profile = "26.1.2"
bind = "127.0.0.1:25565"
motd = "RoM native Rust server"
max_players = 20
online_players = 0
allow_offline_login = true
online_mode = false
hide_online_players = false
enforces_secure_chat = false
previews_chat = false

[configuration]
enabled = true
features = "minecraft:vanilla"
```

The same configuration is available at `examples/server-26.1.2.toml`.

A built-in profile owns its version name, protocol number, and packet IDs. Do not add a manual `[protocol]` section when using:

```toml
profile = "26.1.2"
```

### 3. Start the server

Linux and macOS:

```bash
./target/release/ferrum-server --config server.toml
```

Windows PowerShell:

```powershell
.\target\release\ferrum-server.exe --config server.toml
```

Then connect with a matching Minecraft Java Edition 26.1.2 client to:

```text
127.0.0.1:25565
```

## Native releases

`ferrum-server` is intended to be released as a platform-native executable.

Supported release targets:

- Windows x86_64: `ferrum-server.exe`
- Linux x86_64: `ferrum-server`
- Linux ARM64: `ferrum-server`
- macOS x86_64: `ferrum-server`
- macOS ARM64: `ferrum-server`

Tagged releases matching `v*` run `.github/workflows/release.yml`, build the server with Cargo's release profile, and attach packaged binaries to the GitHub Release.

## Architecture

RoM separates version-independent server behavior from version-specific wire metadata.

Core server crates:

- `ferrum-server` — native TCP server and connection runtime
- `ferrum-runtime` — fixed-rate ticks, bounded connection inputs, and deterministic mutation ordering
- `ferrum-protocol` — packet tables, protocol phases, and connection state validation
- `ferrum-configuration` — Configuration-state payload codecs
- `ferrum-play` — bounded Play-state wire codecs and movement decoding
- `ferrum-world` — version-neutral chunks, player chunk views, and world primitives
- `ferrum-nbt` — bounded binary NBT encoding and decoding
- `ferrum-version-26-1-2` — exact protocol 775 metadata and registry manifests

Design rules:

- No JVM dependency in the released server
- Version-specific packet IDs stay outside gameplay code
- Untrusted lengths are bounded before allocation
- NaN, infinity, invalid movement flags, and out-of-range coordinates are rejected
- Connection inputs are globally bounded and drained in stable fair rounds
- Tick catch-up is capped so overload cannot create an unbounded catch-up spiral
- Authoritative state transitions and loaded-chunk set differences remain deterministic
- Wire codecs are tested with exact-byte fixtures
- Unsupported protocol input fails explicitly instead of being guessed

## Roadmap

The next server milestones are:

1. Wire network workers and shared world state into the authoritative 20 TPS runtime
2. Add mutable block storage and block breaking/placement
3. Broadcast block and player changes across connections
4. Add entities and entity tracking
5. Add inventory and container protocols
6. Add persistent Anvil region loading and saving
7. Add online-mode authentication and encryption
8. Add additional Minecraft version profiles

See [`docs/SERVER_ROADMAP.md`](docs/SERVER_ROADMAP.md) for the detailed server plan.

## Development

Run the full validation suite with:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The repository also contains JVM/JAR analysis and porting-assistance tools that were used to study and plan the Rust implementation. They are developer tooling, not a runtime dependency of the server.

Build the analysis CLI with:

```bash
cargo build --release -p ferrum
```

Example:

```bash
./target/release/ferrum inspect server.jar -o server-report.json
```

## Legal boundary

RoM is an independently written server implementation. Do not publish Mojang-owned source, converted source, or proprietary binaries merely because tooling can inspect them. Use only software and mappings you are authorized to inspect.

This project is not affiliated with or endorsed by Mojang Studios or Microsoft.
