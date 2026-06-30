# RoM Bootstrap

`rom-bootstrap` is the local instance manager for RoM. It follows a Fabric-installer-style distribution model without turning RoM into a patched Java server.

## Design goals

- RoM releases contain only original RoM code and documentation.
- Official Minecraft files are obtained directly by the user from an official Mojang/Microsoft endpoint.
- Every downloaded official artifact is checked against the SHA-1 and size in official version metadata.
- The official server JAR remains in a local cache and is never bundled into RoM release archives.
- The native Rust server remains the executable runtime; Java is not required to run RoM.
- Future version-pack generation must happen locally and record the exact source hash and patch-set identity.

## Current bootstrap stage

The initial implementation supports the `official_source_verified` stage:

1. Resolve Minecraft Java Edition 26.1.2 from the official version manifest.
2. Verify the version metadata SHA-1.
3. Download the official server JAR from an official HTTPS host.
4. Verify the JAR size and SHA-1.
5. Write a local `rom-bootstrap.json` and `versions/26.1.2/rompack.json` provenance record.
6. Create a local-only `eula.txt`, `NOTICE.txt`, and `server.toml`.
7. Build or install the native `ferrum-server` binary.
8. Run the native server from the prepared instance.

This stage does **not** decompile, translate, execute, or redistribute the official server JAR. Data extraction and deterministic version-pack generation remain future work and must preserve the same provenance and local-only boundaries.

## Build the tools

```bash
cargo build --locked --release -p rom-bootstrap -p ferrum-server
```

## Prepare an instance

Review the Minecraft EULA first, then explicitly acknowledge it:

```bash
./target/release/rom-bootstrap prepare \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula
```

The command refuses unsupported Minecraft versions and download URLs outside official Mojang/Microsoft HTTPS hosts.

## Install the local native server

Build `ferrum-server` from the current RoM checkout and copy it into the instance:

```bash
./target/release/rom-bootstrap install-local \
  --instance ./rom-instance \
  --workspace .
```

An already-built binary can be installed instead:

```bash
./target/release/rom-bootstrap install-local \
  --instance ./rom-instance \
  --server-binary ./target/release/ferrum-server
```

## Inspect the instance

```bash
./target/release/rom-bootstrap status --instance ./rom-instance
```

Machine-readable output:

```bash
./target/release/rom-bootstrap status --instance ./rom-instance --json
```

## Run the native server

```bash
./target/release/rom-bootstrap run --instance ./rom-instance
```

Arguments after `--` are forwarded to `ferrum-server`:

```bash
./target/release/rom-bootstrap run --instance ./rom-instance -- --help
```

## Instance layout

```text
rom-instance/
├── bin/
│   └── rom-server
├── cache/
│   └── official/
│       └── 26.1.2/
│           └── server.jar
├── versions/
│   └── 26.1.2/
│       └── rompack.json
├── eula.txt
├── NOTICE.txt
├── rom-bootstrap.json
└── server.toml
```

The file under `cache/official` is a user-local official artifact. Do not add instance directories, cached JARs, or generated proprietary data to RoM releases or source-control commits.

## Public server warning

RoM currently defaults to a loopback bind and offline-mode development login. That is suitable for local protocol testing, but it is not a substitute for Microsoft account authentication. Do not expose an offline-mode development instance to the public internet.

## Planned next stages

1. Add a deterministic local extractor for data that RoM actually needs at runtime.
2. Define a compact, versioned `.rompack` container with source hashes and patch-set metadata.
3. Validate every generated value against captured protocol fixtures and independent tests.
4. Load generated version packs without executing the official JAR.
5. Package `rom-bootstrap` alongside `rom-server` in native release archives.
