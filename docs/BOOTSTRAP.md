# RoM Bootstrap

`rom-bootstrap` is the local instance manager for RoM. It follows a Fabric-installer-style distribution model without turning RoM into a patched Java server.

## Design goals

- RoM releases contain only original RoM code and documentation.
- Official Minecraft files are obtained directly by the user from an official Mojang/Microsoft endpoint.
- Every downloaded official artifact is checked against the SHA-1 and size in official version metadata.
- The official server JAR remains in a local cache and is never bundled into RoM release archives.
- The native Rust server remains the executable runtime; Java is not required to run RoM.
- Every version pack is generated locally and records the exact source hash and patch-set identity.

## Complete supported workflow

For Minecraft 26.1.2, Bootstrap supports the complete ready-to-run workflow:

1. Resolve Minecraft Java Edition 26.1.2 from the official version manifest.
2. Verify the version metadata SHA-1.
3. Download and verify the official server JAR from an approved HTTPS host.
4. Resolve the bundled game JAR without executing Java or bytecode, and verify its listed SHA-256 when valid bundled metadata provides one.
5. Scan only the 28 synchronized-registry resource directories.
6. Parse every selected resource as bounded JSON and derive its resource identifier.
7. Record each selected source path, size, and SHA-256 without copying the JSON payload into the pack.
8. Add the exact 26.1.2 semantic packet table and compare it with the built-in generation profile.
9. Compare the resulting 28 registries and 382 identifiers with the built-in 26.1.2 manifest.
10. Add the world data version, overworld section range, required flat-world block-state IDs, and plains biome ID.
11. Add the dimension ID, dimension-type ID, sea level, flat floor, and deterministic spawn coordinates consumed by Play bootstrap.
12. Write a deterministic schema-v7 `.rompack` with a container SHA-256 trailer and provenance metadata.
13. Install the native `rom-server` executable from a supplied binary, an adjacent release binary, or a local workspace build.
14. Diagnose the manifest, EULA marker, source artifact, version pack, executable, and configuration before launch.
15. Revalidate the pack before `rom-bootstrap run`; the native server then builds its `ProtocolProfile`, Configuration registry payloads, Join Game metadata, spawn packets, movement floor, and initial shared world from pack metadata.

The extractor does **not** decompile, translate, execute, or bytecode-patch the official server JAR. The generated pack is local-only provenance and derived runtime metadata.

## Build the tools

```bash
cargo build --locked --release -p rom-bootstrap -p rom-server
```

## Complete an instance in one command

From an extracted native release, the adjacent `rom-server` binary is detected automatically:

```bash
./rom-bootstrap setup \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula
```

From a source checkout, build and install from the workspace:

```bash
cargo build --locked --release -p rom-bootstrap -p rom-server
./target/release/rom-bootstrap setup \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula \
  --workspace .
```

`setup` executes `prepare`, `generate`, `install-local`, and the final readiness check. It reuses verified artifacts and valid packs by default. Use `--force-download` or `--force-generate` only when regeneration is intentional. An explicit `--server-binary` overrides adjacent-binary detection and workspace building.

Machine-readable setup output:

```bash
./rom-bootstrap setup --instance ./rom-instance --accept-minecraft-eula --json
```

## Manual stages

The individual commands remain available for troubleshooting, custom launchers, and controlled regeneration.

### Prepare an instance

Review the Minecraft EULA first, then explicitly acknowledge it:

```bash
./target/release/rom-bootstrap prepare \
  --instance ./rom-instance \
  --version 26.1.2 \
  --accept-minecraft-eula
```

The command refuses unsupported Minecraft versions and download URLs outside official Mojang/Microsoft HTTPS hosts.

### Generate the local version pack

```bash
./target/release/rom-bootstrap generate \
  --instance ./rom-instance
```

Use `--force` to regenerate an already valid pack. Generation is deterministic for the same verified source JAR and extractor version. Packs from earlier schemas are intentionally rejected after packet-catalog, world-metadata, item/component-palette, and inventory migrations and must be regenerated.

### Install the local native server

Build `rom-server` from the current RoM checkout and copy it into the instance:

```bash
./target/release/rom-bootstrap install-local \
  --instance ./rom-instance \
  --workspace .
```

An already-built binary can be installed instead:

```bash
./target/release/rom-bootstrap install-local \
  --instance ./rom-instance \
  --server-binary ./target/release/rom-server
```

## Inspect the instance

```bash
./target/release/rom-bootstrap status --instance ./rom-instance
```

Machine-readable output:

```bash
./target/release/rom-bootstrap status --instance ./rom-instance --json
```

## Diagnose readiness

```bash
./target/release/rom-bootstrap doctor --instance ./rom-instance
```

`doctor` checks the Bootstrap manifest, EULA marker, official source integrity, generated pack integrity, native executable, and `server.toml`. It lists every problem and returns a non-zero exit status until the instance is ready. Add `--json` for scripts and launchers.

## Run the native server

```bash
./target/release/rom-bootstrap run --instance ./rom-instance
```

Arguments after `--` are forwarded to `rom-server`:

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

## Native release archives

Platform release archives contain both `rom-bootstrap` and `rom-server`. After extracting an archive, the full first-run flow is:

```bash
./rom-bootstrap setup --instance ./rom-instance --version 26.1.2 --accept-minecraft-eula
./rom-bootstrap doctor --instance ./rom-instance
./rom-bootstrap run --instance ./rom-instance
```

The adjacent server executable is detected automatically. Existing development instances containing `bin/rom-server` remain readable, but new installations use `bin/rom-server`.

## Runtime Play policy

Bootstrap writes an explicit bounded Play policy into new `server.toml` files:

```toml
[play]
chunk_radius = 1
simulation_distance = 2
welcome_message = "RoM native Rust world loaded"
keep_alive_interval_seconds = 15
```

The chunk radius controls both initial in-memory chunk seeding and each player's visible chunk view. Existing instance configurations remain valid because the native server supplies the same defaults when `[play]` is absent.

## Runtime world source

By default, RoM starts from the deterministic in-memory flat world. To seed chunks from a Minecraft Anvil region file, configure one source in `server.toml`:

```toml
[world]
region_file = "world/region/r.0.0.mca"
```

The region coordinates are inferred from `r.X.Z.mca`. If the file name is not in that form, set both coordinates explicitly:

```toml
[world]
region_file = "world/custom/spawn.mca"
region_x = 0
region_z = 0
```

To load every region file in a directory, use `region_dir` instead:

```toml
[world]
region_dir = "world/region"
```

`region_file` and `region_dir` are mutually exclusive. `region_x` and `region_z` must be set together, and they are only valid with `region_file`. Bad chunks are skipped with startup warnings; bad files in a region directory are skipped if at least one region file loads successfully. Unknown block states currently fall back to stone and unknown biomes fall back to plains while registry-complete conversion remains in progress.

## Public server warning

RoM currently defaults to a loopback bind and offline-mode development login. That is suitable for local protocol testing, but it is not a substitute for Microsoft account authentication. Do not expose an offline-mode development instance to the public internet.

## Completion boundary

The Bootstrap workflow is complete for the supported Minecraft 26.1.2 profile: it can prepare, generate, install, diagnose, and run a local native instance without Java. New Minecraft versions remain separate profile work and must preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries.
