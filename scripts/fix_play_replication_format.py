from pathlib import Path

path = Path("crates/ferrum-server/src/play_writer.rs")
text = path.read_text(encoding="utf-8")
old = "| PlayOutput::PlayerTeleport { .. } => Ok(PlayWriterDirective::Continue)\n"
new = "| PlayOutput::PlayerTeleport { .. } => Ok(PlayWriterDirective::Continue),\n"
if old not in text:
    raise SystemExit("semantic writer match arm target not found")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
