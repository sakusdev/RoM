from pathlib import Path

path = Path("crates/ferrum-server/src/game_service.rs")
text = path.read_text(encoding="utf-8")
old = "if config.autosave_interval.is_some_and(Duration::is_zero) {"
new = "if config.autosave_interval.is_some_and(|interval| interval.is_zero()) {"
if old not in text:
    raise SystemExit("autosave validation target not found")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
