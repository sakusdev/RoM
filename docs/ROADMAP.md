# Roadmap

## M0 — JAR inventory (included)

- Read ZIP/JAR archives.
- Parse class metadata.
- Resolve class, field, and method names/descriptors.
- Keep per-entry failures instead of aborting.
- Emit a versioned JSON report.
- Prefix filtering and scan limits.

## M1 — Bytecode inventory (included)

- Parse `Code`, exception tables, and line tables.
- Parse local variable tables when available.
- Count opcodes and emit deterministic opcode sequences and histograms.
- Extract method, field, type, and useful string references.
- Detect native methods, synchronization, `invokedynamic`, lambda metafactory usage, reflection, class loaders, `Unsafe`, native library loading, switch bytecode, monitors, exception handlers, dynamic proxies, `java.lang.invoke`, runtime bytecode-generation markers, arrays, throws, and legacy `jsr`/`ret`.
- Generate Green/Yellow/Red porting classifications with machine-readable reason codes.
- Expose this heavier report through `ferrum bytecode` so `ferrum inspect` remains the M0 inventory command.

## M2 — Control-flow graph (included)

- Split bytecode into basic blocks.
- Branch, fallthrough, switch, return, throw, and exception-handler edges.
- Validate branch, switch, and exception-handler targets without aborting the whole JAR scan.
- Graphviz DOT and stable JSON output through `ferrum cfg`.
- Deterministic graph IDs for diffing across mappings/versions.

## Native server release foundation (included)

- Add `ferrum-server` as a native Rust binary target.
- Build release artifacts with Cargo's `release` profile.
- Package native binaries for Windows x86_64, Linux x86_64, Linux ARM64, macOS x86_64, and macOS ARM64.
- Publish release artifacts through GitHub Actions without producing or requiring a JAR.
- Keep Java/JVM requirements out of the server runtime path.

## M3 — Typed IR (included)

- Parse JVM field and method descriptors into structured Java types.
- Determine local types from method descriptors, `LocalVariableTable`, and store inference.
- Simulate the operand stack into explicit SSA-like value IDs for common bytecode forms.
- Retain `StackMapTable` frames for merge validation.
- Insert conservative phi/merge placeholders at CFG joins.
- Preserve exception handlers as typed IR metadata and recoverable errors.

## M4 — Rust skeleton generation (included)

- Generate deterministic Rust skeleton packages through `ferrum generate`.
- Map Java classes to structs and interfaces to traits.
- Map fields and method signatures from JVM descriptors.
- Use compatibility-layer placeholders for Java references, arrays, strings, and exceptions.
- Retain source provenance in generated Rust doc attributes.
- Emit structured generation warnings and `todo!()` method bodies without lowering bytecode bodies.

## M5 — Minecraft rules (included as planning/report foundation)

- Tiny mapping ingestion summary for classes, fields, and methods.
- Type, method, field, and manual override rewrite TOML ingestion.
- Registry, Codec, Packet, position, inventory, and NBT special-case reporting.
- Pilot-port readiness reporting for contained Minecraft types.

## M6 — Differential tests (included as replay comparator foundation)

- Versioned replay JSON model.
- Expected/actual outcome comparator.
- Passed, failed, and pending replay reports.
- External Java and Rust harness execution remains a later integration layer.

## M7 — Fabric and Mixin analyzer (included as compatibility report foundation)

- Fabric metadata inspection.
- Access widener and nested JAR discovery.
- Mixin configuration discovery.
- Conservative injection compatibility classification.

## M8 — Server integration pilots (documented boundary)

- `ferrum-server` remains a native Rust binary target.
- Native `ferrum-server` now implements the first Minecraft Java server surface: handshake, status response, and ping/pong.
- Login-intent handshakes derive Java-compatible offline-mode player UUIDs and can optionally receive a minimal Login Success packet; otherwise they receive an explicit disconnect message instead of silent connection close.
- Connections are handled independently so slow clients do not block status responses.
- Packet framing, VarInt, string, and packet-reader code is isolated in the server codec module.
- Status responses include vanilla-style server list metadata: favicon, sample players, secure-chat flags, and optional hidden player counts.
- Minecraft Java Edition `26.1.2` is the first concrete built-in profile, including strict protocol validation and vanilla Known Packs negotiation.
- The 26.1.2 profile synchronizes all 28 data-driven registries and 382 vanilla entry identifiers using references to the accepted `minecraft/core/26.1.2` pack.
- Manual packet tables remain available for protocol experiments outside built-in profiles.
- Generated reports and replay cases are ready to feed later Rust-native server subsystems.
- Actual gameplay subsystem implementation is intentionally outside the porting kit CLI.
