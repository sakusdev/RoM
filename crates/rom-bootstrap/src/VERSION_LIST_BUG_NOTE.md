# Bootstrap versions.list fallback investigation

`rom-bootstrap generate` currently trusts `META-INF/versions.list` whenever the verified official server bundle contains it. If that list is malformed, uses an unexpected delimiter, or points at an otherwise invalid record, generation fails before trying the safe fallback that scans for a single embedded game JAR under `META-INF/versions/`.

Because `prepare` verifies the outer `server.jar` by official SHA-1, a generate-time `versions.list` failure is not necessarily a local cache corruption. The extractor should treat `versions.list` as preferred metadata, but fall back to scanning the already verified bundle when exactly one embedded game JAR exists.

Proposed code change:

- In `resolve_game_jar`, replace the direct `read_versions_list(&mut archive)?` call with a helper that catches `versions.list` parse errors and falls back to `find_single_embedded_jar`.
- Keep ambiguity strict: if scanning finds zero or multiple embedded JARs, fail.
- Preserve SHA-256 verification when a valid digest is available from `versions.list`.
- Add tests for malformed `META-INF/versions.list` with a single embedded JAR.
