# Generated packet catalog

RoM version packs can carry both the typed packet table understood by the current runtime and a complete packet catalog generated from Mojang's `reports/packets.json`.

Use `rom-bootstrap generate --packet-report <path>` to supply a report explicitly. When omitted, the bootstrapper checks these instance-relative locations in order:

- `generated/reports/packets.json`
- `generated-reports/reports/packets.json`
- `reports/packets.json`

If no generated report is present, RoM builds a canonical catalog from the built-in typed core so existing local setup remains usable.

The generated catalog is validated for canonical names, non-negative IDs, and uniqueness by protocol phase and direction. Typed packet records stored in the `.rompack` must agree with the corresponding entries in the complete catalog. The runtime continues to use the typed table while retaining unknown catalog entries for future packet implementations without requiring another extraction.
