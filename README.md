# RoM — Reimplementation of Minecraft

**RoM is an experimental, native Minecraft Java Edition-compatible server written in Rust.**

> **NOT AN OFFICIAL MINECRAFT PRODUCT.**  
> **NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.**

RoM runs as a native executable rather than a JAR and does not require Java or a JVM at runtime. The current compatibility target is **Minecraft Java Edition 26.1.2**, protocol **775**.

> [!IMPORTANT]
> RoM is an active alpha. Bootstrap, login, world delivery, movement, block interaction, authoritative player inventory, and container transactions are implemented, but the server is not yet a production replacement for Vanilla, Paper, or Fabric servers.

## Install a release

The newest automatically numbered build is available on the [GitHub Releases page](https://github.com/sakusdev/RoM/releases/latest). Every successful `master` release publishes native `ferrum-server` and `rom-bootstrap` binaries, platform archives, checksums, `VERSION`, and `BUILD_INFO` metadata.

### Desktop and normal Linux

Download the archive for your platform, extract it, and run:

```bash
./rom-bootstrap setup \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula
./rom-bootstrap doctor --instance ./rom-instance
./rom-bootstrap run --instance ./rom-instance
```

On Windows, use `rom-bootstrap.exe` and `ferrum-server.exe`.

### Android Termux

Download `install-termux.sh` from the selected Release and run:

```bash
chmod +x install-termux.sh
./install-termux.sh
```

The installer selects the Android Bionic AArch64 build, verifies SHA-256 checksums, and installs `rom-bootstrap` and `ferrum-server` into the Termux prefix.

### Pixel Terminal

Download `install-pixel-terminal.sh` from the selected Release and run:

```bash
chmod +x install-pixel-terminal.sh
./install-pixel-terminal.sh
```

The installer selects the Linux AArch64 or x86_64 build from the detected CPU architecture, verifies checksums, and installs into `~/.local/bin` by default.

Set `ROM_INSTALL_DIR` to choose another installation directory. Set `ROM_VERSION` when using a source copy of either installer to pin a specific release.

### Server console

While `ferrum-server` is running interactively, enter commands without a leading slash:

```text
help
list
say Server maintenance starts soon
save-all
stop
```

`?` is an alias for `help`. `exit` and `quit` are safe aliases for `stop`; they request a final gameplay/world save before shutdown. A bare `say` is invalid because a message is required.

## What works now

### Bootstrap and version data

- One-command, idempotent `rom-bootstrap setup`
- Official Mojang/Microsoft version metadata resolution
- Direct official server JAR download on the user's machine
- Official size and SHA-1 verification
- Bounded extraction of synchronized registry resources
- Deterministic and integrity-protected schema-v7 `.rompack` generation
- Generated packet catalog, packet IDs, item IDs, data-component IDs, registries, world metadata, block states, and biome metadata
- Runtime validation of the generated pack against the built-in 26.1.2 profile
- `doctor`, `status`, JSON output, local install, and managed server launch

RoM does not redistribute the official server JAR. The JAR remains in the user's local cache and is used only as a verified source for generated compatibility metadata.

### Connection and protocol flow

- Handshake, status, ping, and pong
- Strict protocol-775 validation
- Development offline-mode login with Java-compatible offline UUIDs
- Login Acknowledged and Configuration transitions
- Known Packs negotiation for `minecraft/core/26.1.2`
- Feature Flags, all synchronized registry data, the client-synchronized subset of the official 26.1.2 network tag manifest, and Finish Configuration
- Join Game, initial chunks, player position, teleport acknowledgement, and dynamic chunk views
- Configurable Keep Alive and disconnect encoding
- Live online-player count in status responses
- Offline-mode player chat accepted from the generated `chat` packet and replicated through authoritative gameplay events

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
→ Bounded Chunk View
→ Gameplay
```

### “Network protocol error” during Configuration

A client that closes immediately after login can leave this server-side message:

```text
cannot read configuration acknowledged packet: failed to fill whole buffer
```

This means the client disconnected before acknowledging the end of Configuration; it is not an EULA or TCP bind failure. Builds containing the complete 26.1.2 network tag manifest fix the known empty-Tags cause. Update both `rom-bootstrap` and `ferrum-server`, reinstall the native server into the instance, and regenerate version metadata when moving from an older build:

```bash
rom-bootstrap setup \
  --instance ~/rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula \
  --force-generate
rom-bootstrap doctor --instance ~/rom-instance
rom-bootstrap run --instance ~/rom-instance
```

If it still disconnects, keep the exact client error and the final `connection closed:` server line together when reporting the issue.

### World and movement

- Authoritative shared in-memory world state
- Deterministic flat overworld
- Optional loading of Anvil region files and region directories
- Initial and dynamic chunk serialization
- Bounded per-player chunk loading and unloading
- All four serverbound movement packet forms
- Finite-coordinate, range, movement-flag, and exact-payload validation
- Bounded movement delta validation
- Flat-floor penetration rejection
- Simplified block breaking and adjacent-face stone placement
- Basic reach validation, bedrock protection, replaceability checks, Block Update, and prediction acknowledgement
- Bounded peer update queues with same-position coalescing

### Inventory and containers

- Authoritative 46-slot player inventory
- Full inventory snapshot after Play bootstrap
- Exact changed-slot replication for server-side mutations such as `/give`
- Version-aware ItemStack encoding from generated item and data-component palettes
- Serverbound container click decoding and transaction processing
- Pickup, quick move, swap, clone, throw, quick craft, and pickup-all click modes
- Cursor/carried stack state
- External container sessions and state IDs
- `SetContainerContent` and `SetContainerSlot` synchronization
- Creative slot updates with validation
- Stale or invalid transaction rejection followed by authoritative resynchronization
- Inventory mutation APIs for insert, remove, clear, swap, consume, drop, and death handling
- `keepInventory` handling
- Synchronization, rejection, snapshot, and drop metrics

### Admin web GUI

`ferrum-server` starts a lightweight local dashboard at
[`http://127.0.0.1:25575`](http://127.0.0.1:25575) by default. It is embedded in
the native binary and loads no external assets.

The dashboard displays CPU, memory, and instance-filesystem disk usage, server
uptime, online players, game ticks, and entity count. Its command box uses the
same authoritative parser as standard input and never invokes an operating-
system shell. Commands include `help`, `list`, `say hello`, `save-all`, and
`stop`.

Use `--no-admin-gui` to disable it and `--admin-bind` to choose another address.
A non-loopback bind is rejected unless `--admin-token` is supplied; remote
binds require at least 16 visible ASCII characters. The built-in endpoint is
plain HTTP, so expose it only on a trusted LAN or place it behind a TLS reverse
proxy. On Termux or Pixel Terminal, leave the loopback default and open
`http://127.0.0.1:25575` in the Android browser.

```bash
ferrum-server \
  --config ./rom-instance/server.toml \
  --version-pack ./rom-instance/versions/26.1.2/26.1.2.rompack \
  --admin-bind 127.0.0.1:25575
```

### Runtime architecture

- Version-neutral 20 TPS scheduling primitives
- Deterministic mutation ordering
- Bounded worker command channels
- Independently bounded non-blocking connection outputs
- Dedicated live Play writer workers
- Semantic Play outputs separated from version-specific packet IDs
- Cross-platform workspace tests on Ubuntu, Windows, and macOS
- Numbered native Releases for Windows, Linux, Android/Termux, and macOS

## Current boundaries

RoM is usable for local development and protocol/gameplay experimentation. The following areas are still incomplete:

- Microsoft account authentication, encryption, session-server verification, and secure online mode
- Full Vanilla collision shapes, movement-speed enforcement, reach rules, and anti-cheat behavior
- Entity spawning, metadata, movement, tracking, combat, mobs, and item entities
- Complete Vanilla block behavior, tools, mining rules, placement contexts, fluids, redstone, and scheduled block ticks
- Crafting, recipes, furnaces, enchanting, brewing, anvil, smithing, and the complete menu catalog
- Anvil region writing, transactional storage, and stronger crash recovery beyond the current validated JSON gameplay/world snapshots
- Procedural world generation and multiple dimensions
- Full data-component semantic validation for every Vanilla component type
- Commands, permissions, operators, bans, whitelist, and administration parity
- Multi-version compatibility
- Fabric, Bukkit, Spigot, Paper, or proxy plugin compatibility
- Production hardening, denial-of-service testing, long-running soak testing, and public-server security review

> [!CAUTION]
> The default configuration binds to loopback and uses development-only offline login. Do not expose it directly to the public internet. Offline mode does not prove that a connecting user owns Minecraft or the claimed username.

## Distribution model

RoM follows a native, installer-style bootstrap model:

1. Releases contain only RoM's original binaries, scripts, and documentation.
2. `rom-bootstrap` resolves the selected Minecraft version from official metadata.
3. The official server JAR is downloaded directly from an official HTTPS endpoint.
4. Its size and SHA-1 are checked against official metadata.
5. The JAR remains in the local cache and is never bundled into a RoM Release.
6. RoM reads only the bounded resources required to generate compatibility metadata.
7. The deterministic `.rompack` records source hashes, packet and registry metadata, item and component palettes, and patch-set identity.
8. `ferrum-server` validates the pack before accepting connections.

The Bootstrap process does not execute, decompile, translate, bytecode-patch, or redistribute the official server JAR. See [`docs/BOOTSTRAP.md`](docs/BOOTSTRAP.md), [`docs/RELEASES.md`](docs/RELEASES.md), and [`NOTICE.md`](NOTICE.md).

## Build from source

The workspace requires Rust **1.85 or newer**.

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

Run the full validation suite with:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Instance layout

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

Generated instance data and the official game cache are local artifacts. Do not commit or redistribute instance directories containing official Minecraft files.

## Release targets

Each numbered Release provides raw executables and archives for:

- Windows x86_64
- Linux x86_64
- Linux AArch64 using glibc
- Android/Termux AArch64 using Android Bionic
- macOS x86_64
- macOS AArch64

Release downloads include per-file `.sha256` files and a combined `SHA256SUMS`. See [`docs/RELEASES.md`](docs/RELEASES.md) for numbering and artifact policy.

## Workspace architecture

RoM separates version-independent gameplay/runtime logic from version-specific wire metadata.

- `rom-bootstrap` — official-source verification, `.rompack` generation, diagnostics, installation, and instance management
- `ferrum-rompack` — deterministic compatibility-pack encoding, integrity validation, and bounded decoding
- `ferrum-server` — native TCP server, connection lifecycle, replication, and gameplay integration
- `ferrum-runtime` — fixed-rate scheduling, bounded inputs, workers, and deterministic mutation ordering
- `ferrum-protocol` — state machine, packet tables, typed packet catalog, and framing
- `ferrum-configuration` — Configuration-state payload codecs
- `ferrum-play` — Play-state packet decoding and encoding, movement, ItemStack, inventory, and container codecs
- `ferrum-game` — authoritative players, inventory, container transactions, mutations, and game events
- `ferrum-world` — chunks, views, Anvil loading, and world primitives
- `ferrum-version-26-1-2` — generated and hand-audited compatibility profile for Minecraft 26.1.2

## License

RoM is licensed under the MIT License. See [`LICENSE`](LICENSE). Third-party and Minecraft-related notices are documented in [`NOTICE.md`](NOTICE.md).
