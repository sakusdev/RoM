# World persistence

RoM stores gameplay state and modified world chunks separately:

- `game-state.json` contains players, inventories, entities, time, and gameplay rules.
- `world-state.json` contains the authoritative loaded chunk store, including block and biome palettes for every section.

Both files are written through temporary files and atomically replaced. The server restores `world-state.json` before considering configured Anvil seed inputs, so changes made after startup survive restart.

The same autosave interval controls gameplay and world snapshots. A value of zero disables periodic saves but final shutdown saves remain enabled. The `save-all` command requests both snapshots and reports their paths and sizes.

```bash
ferrum-server \
  --config server.toml \
  --version-pack versions/26.1.2/26.1.2.rompack \
  --game-state game-state.json \
  --world-state world-state.json \
  --autosave-seconds 30
```

World snapshots are schema-versioned and validated before restoration. RoM rejects unsupported schemas, duplicate chunk positions, invalid section lengths, and chunks that do not match the selected version pack's world-height profile.

This format is RoM's native persistence format. Vanilla-format Anvil region saving is a separate compatibility feature and is not yet implemented.
