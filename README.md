# Ferrum Porting Kit

An experimental Rust toolchain for analyzing JVM JARs and accelerating a clean, testable port of Minecraft server behavior to Rust.

This repository **does not promise automatic, semantically perfect JAR → Rust conversion**. The practical goal is to automate inventory, dependency analysis, control-flow recovery, IR construction, skeleton generation, and differential testing so humans spend time on Minecraft-specific semantics rather than repetitive transcription.

## Included milestones

`ferrum inspect` scans a JAR and emits versioned JSON containing:

- Class names, superclass, and interfaces
- Class-file/Java versions
- Access flags
- Fields and JVM descriptors
- Methods and JVM descriptors
- Constant-pool and attribute counts
- Per-entry parse/verification errors
- JAR manifest and aggregate statistics

`ferrum bytecode` extends the same report with opt-in method-body inventory:

- `Code` attribute size, max stack, max locals, exception handlers, line numbers, and local variables
- Decoded opcode sequence and deterministic opcode histogram
- Branch, switch, return, and throw instruction inventories
- Referenced methods, fields, types, and loaded string constants
- Difficult JVM feature detection for native methods, synchronization, `invokedynamic`, reflection, class loading, `Unsafe`, native library loading, dynamic proxies, `java.lang.invoke`, runtime bytecode generation markers, switches, exceptions, arrays, and legacy `jsr`/`ret`
- Green/Yellow/Red porting classifications with machine-readable reason codes

`ferrum cfg` extends the bytecode report with method-level control-flow graphs:

- Deterministic basic blocks and block IDs
- Branch, fallthrough, switch, return, throw, and exception-handler edges
- Branch and switch target validation
- Stable JSON plus Graphviz DOT output for a selected method

`ferrum ir` extends the CFG report with typed intermediate representation data:

- Parsed method and field descriptors
- Local-variable types from descriptors, `LocalVariableTable`, and store inference
- Explicit SSA-like values produced by operand-stack simulation
- Structured `StackMapTable` frames when present
- Conservative phi/merge placeholders at CFG joins
- Exception-handler metadata and recoverable IR lowering errors

`ferrum generate` emits deterministic Rust skeleton packages for selected classes:

- Rust structs for Java classes and traits for Java interfaces
- Field and method signatures mapped from JVM descriptors
- Source provenance retained as Rust doc attributes
- `todo!()` placeholders for every unsupported method body
- Structured generation warnings in `ferrum-generation-report.json`

`ferrum map` adds the Minecraft-specific planning layer:

- Tiny mapping inventory for classes, fields, and methods
- Rewrite TOML ingestion for Java type/member replacements
- Minecraft special-case detection for registries, codecs, packets, positions, NBT, and inventory types
- Pilot-port readiness reporting for contained early targets such as `Identifier`, `BlockPos`, and `ChunkPos`

`ferrum fabric inspect` produces Fabric compatibility reports:

- `fabric.mod.json` metadata, entrypoints, dependencies, nested JARs, and access wideners
- Mixin configuration discovery
- Conservative Mixin compatibility classifications

`ferrum diff run` reads deterministic replay JSON and compares expected versus actual outcomes. It is the differential-test report format and comparator foundation; external Java/Rust runners are intentionally not hidden inside this command yet.

The parser library currently supports class files through Java 25. Newer class versions are preserved as per-entry errors with best-effort major/minor header values.

## Build

```bash
cargo build --release -p ferrum
cargo build --release -p ferrum-server
```

## Native server releases

`ferrum-server` is distributed as native Rust binaries, not as a JAR. Release binaries must not require Java or a JVM at runtime.

Supported release targets:

- Windows x86_64: `ferrum-server.exe`
- Linux x86_64: `ferrum-server`
- Linux ARM64: `ferrum-server`
- macOS x86_64: `ferrum-server`
- macOS ARM64: `ferrum-server`

Expected usage:

```powershell
ferrum-server.exe --config server.toml
```

```bash
./ferrum-server --config server.toml
```

Current server runtime scope:

- Native Rust binary only; no Java or JVM at runtime
- Minecraft Java status handshake
- Status response JSON
- Ping/pong latency packet
- Login intent handling with offline-mode player identity generation, optional Login Success, and an explicit JSON disconnect fallback
- Per-connection worker threads so status pings do not block each other
- Shared Minecraft packet codec module for VarInt, strings, framed packets, and packet readers
- Vanilla-style server list metadata: favicon, sample players, secure-chat flags, and hidden player counts
- Configurable `version.name`, protocol number, and packet IDs for Minecraft Java Edition `26.*.*` experiments

Minimal `server.toml`:

```toml
[server]
bind = "127.0.0.1:25565"
version_name = "Minecraft Java Edition 26.*.*"
protocol = 0
motd = "Ferrum native Rust server"
max_players = 20
online_players = 0
login_disconnect_message = "Ferrum native server currently implements status ping only"
allow_offline_login = false
online_mode = false
hide_online_players = false
enforces_secure_chat = false
previews_chat = false
server_icon = "data:image/png;base64,iVBORw0KGgo="
sample_players = "Ferrum:00000000-0000-0000-0000-000000000000"

[protocol]
handshake_serverbound = 0
status_request_serverbound = 0
status_response_clientbound = 0
ping_request_serverbound = 1
pong_response_clientbound = 1
login_start_serverbound = 0
login_disconnect_clientbound = 0
login_success_clientbound = 2
```

Set `allow_offline_login = true` to answer login intent with a Java-compatible offline UUID and a minimal Login Success packet. This only admits the client past the login gate; configuration/play-state packets are still intentionally not implemented.

Tagged releases matching `v*` run `.github/workflows/release.yml`, build `ferrum-server` with Cargo's `release` profile on each supported native runner, package the binary with README and LICENSE, and attach the archives to the GitHub Release.

## Use

```bash
# Full report
./target/release/ferrum inspect server.jar -o server-report.json

# Start with Minecraft classes only
./target/release/ferrum inspect server.jar \
  --prefix net.minecraft \
  --limit 500 \
  --verify \
  -o minecraft-report.json

# Fabric mod
./target/release/ferrum inspect lithium.jar -o lithium-report.json

# Bytecode inventory and porting difficulty classification
./target/release/ferrum bytecode server.jar \
  --prefix net.minecraft \
  -o server-bytecode-report.json

# CFG JSON plus DOT for one method
./target/release/ferrum cfg server.jar \
  --class net.minecraft.Example \
  --method tick \
  --dot-output tick.dot \
  -o tick-cfg.json

# Typed IR for one method
./target/release/ferrum ir server.jar \
  --class net.minecraft.Example \
  --method tick \
  -o tick-ir.json

# Rust skeleton package for one class
./target/release/ferrum generate server.jar \
  --class net.minecraft.Example \
  --output generated/example

# Mapping and Minecraft rewrite planning
./target/release/ferrum map server.jar \
  --mappings yarn.tiny \
  --rewrites mappings \
  -o mapping-report.json

# Fabric metadata and Mixin compatibility
./target/release/ferrum fabric inspect example-mod.jar \
  -o fabric-report.json

# Differential replay comparator
./target/release/ferrum diff run replay.json \
  -o diff-report.json
```

The JSON document uses `schema_version: 1`. A bad class is added to `errors`; it does not normally abort the scan. Add `--fail-on-class-error` in CI when strict behavior is desired.

M1 classification policy is intentionally conservative: simple primitive/local/field bytecode is Green, features that need CFG or Java object semantics review are Yellow, and native methods, reflection-heavy code, custom class loading, `Unsafe`, native library loading, dynamic proxies, runtime bytecode generation, and legacy subroutines are Red.

## Test fixture

```bash
./fixtures/sample/build.sh
./fixtures/m1-bytecode/build.sh
cargo test --workspace
python3 tools/reference_scan.py crates/ferrum-importer/tests/fixtures/sample.jar
```

## Workspace

- `ferrum-model`: stable report/IR-facing data contracts
- `ferrum-importer`: JAR and JVM Class File importer
- `ferrum`: CLI
- `ferrum-server`: native Rust server binary target
- `docs/ROADMAP.md`: bytecode, CFG, IR, code-generation milestones
- `tools/reference_scan.py`: small independent fixture validator

## Legal boundary

Use only software and mappings you are authorized to inspect. Do not publish Mojang-owned converted source or binaries merely because a tool can generate them. A safer public architecture is to publish original tooling, independently written runtime/server code, mapping/rewrite rules, and tests while requiring users to supply legitimately obtained inputs locally. This is not legal advice.
