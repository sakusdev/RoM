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

The implementation supports `official_source_verified` and `version_pack_generated`:

1. Resolve Minecraft Java Edition 26.1.2 from the official version manifest.
2. Verify the version metadata SHA-1.
3. Download and verify the official server JAR from an approved HTTPS host.
4. Resolve the bundled game JAR without executing Java or bytecode, and verify its listed SHA-256 when valid bundled metadata provides one.
5. Scan only the 28 synchronized-registry resource directories.
6. Parse every selected resource as bounded JSON and derive its resource identifier.
7. Record each selected source path, size, and SHA-256 without copying the JSON payload into the pack.
8. Compare the resulting 28 registries and 382 identifiers with the built-in 26.1.2 manifest.
9. Write a deterministic `.rompack` with a container SHA-256 trailer and provenance metadata.
10. Revalidate the pack before `rom-bootstrap run`, then pass it to the native server for a second profile check.

The extractor does **not** decompile, translate, execute, or bytecode-patch the official server JAR. The generated pack is local-only provenance and derived runtime metadata.

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

## Generate the local version pack

```bash
./target/release/rom-bootstrap generate \
  --instance ./rom-instance
```

Use `--force` to regenerate an already valid pack. Generation is deterministic for the same verified source JAR and extractor version.

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
│       ├── 26.1.2.rompack
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

1. Package `rom-bootstrap` alongside `rom-server` in native release archives.
2. Move packet tables and additional version-specific runtime metadata into generated packs.
3. Add more independently testable extractors only when the server consumes their output.
4. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.
