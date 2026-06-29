# Architecture

```text
JAR/CLASS input
    │
    ▼
ferrum-importer          Fault-tolerant archive and class-file ingestion
    │
    ▼
ferrum-model             Stable serializable models and future IR contracts
    │
    ├── JSON reports      Human inspection, diffs, compatibility database
    ├── typed IR          Stack bytecode → CFG → explicit values
    │
    ▼
ferrum generate           Deterministic Rust skeleton packages
    │
    ├── ferrum map         Mapping/rewrite/Minecraft special-case planning
    ├── ferrum fabric      Fabric metadata and Mixin compatibility reports
    └── ferrum diff        Replay comparator for differential testing
```

## Design rules

1. A single malformed class must not abort a whole `server.jar` scan.
2. JVM internal names remain canonical; dotted names are presentation only.
3. Reports are versioned so later tools can consume them reliably.
4. Minecraft-specific mappings belong above the generic class-file importer.
5. Generated Rust must retain source class/method provenance.
6. Bytecode conversion will start from `Code` attributes, not decompiler text.

## Current IR and codegen path

Implementation order:

1. Decode `Code` attributes into bytecode instructions.
2. Find leaders and construct basic blocks.
3. Resolve branch and exception-handler edges.
4. Simulate JVM operand-stack types using descriptors and StackMapTable.
5. Convert stack operations to SSA-like values.
6. Generate Rust structs, traits, fields, signatures, provenance, and `todo!()` bodies.
7. Add mapping/rewrite reports, Fabric compatibility reports, and replay comparators.
8. Add Java-semantics lowering and Rust-native gameplay subsystem implementation in later server work.


## Native server path

```text
ferrum-version-26-1-2   Exact protocol IDs and version-specific numeric IDs
          │
          ├── ferrum-configuration   Known packs, registries, features, tags
          ├── ferrum-play            Play payload and palette/light encoding
          └── ferrum-world           Version-neutral coordinates and chunk state
                       │
                       ▼
                 ferrum-server        Socket runtime and protocol orchestration
```

The world crate does not know packet IDs or Minecraft-version numeric registries. The Play codec does not own authoritative world mutation. The server runtime orders Configuration, Join Game, chunk batches, acknowledgements, and Keep Alive while preserving deterministic state transitions.
