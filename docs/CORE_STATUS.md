# RoM core completion status

This document tracks the boundary between implemented core infrastructure and production-parity work.

## Implemented foundations

- Native protocol-775 connection lifecycle for Minecraft Java Edition 26.1.2
- Official-source Bootstrap and schema-v7 compatibility packs
- Deterministic 20 TPS runtime and bounded worker queues
- Shared world state, chunk views, movement, and basic block interaction
- Authoritative player inventory and container transactions
- Entity storage and gameplay-state snapshots
- Atomic gameplay autosave and shutdown save
- Deterministic world chunk snapshots with validation
- World startup restore, periodic autosave, `save-all`, and final shutdown save
- Native numbered Releases for Windows, Linux, Android/Termux, and macOS

## Current completion work

Durable world persistence is connected to the server lifecycle. Client-visible entity replication is the next protocol-level subsystem; packet layouts must be derived from the exact 26.1.2 implementation rather than guessed.

## Production boundary

Production parity still requires secure online-mode authentication, complete entity and combat behavior, full collision and block semantics, Vanilla menu/recipe systems, procedural generation, Vanilla-format world saving, administration parity, multi-version support, and sustained security/soak testing.
