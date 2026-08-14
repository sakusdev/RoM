from pathlib import Path

path = Path(__file__).with_name("agent_integrate_combat_loop.py")
text = path.read_text(encoding="utf-8")
marker = "# rom-protocol: make the 26.1.2 dedicated attack packet a first-class semantic kind.\n"
helper = '''def replace_first(path: Path, old: str, new: str) -> None:\n    text = path.read_text(encoding="utf-8")\n    if old not in text:\n        raise RuntimeError(f"missing marker in {path}: {old[:180]!r}")\n    path.write_text(text.replace(old, new, 1), encoding="utf-8")\n\n\n'''
if "def replace_first(" not in text:
    if marker not in text:
        raise RuntimeError("combat integration marker not found")
    text = text.replace(marker, helper + marker, 1)
needle = 'replace_once(protocol, "            | Self::ClientCommand\\n            | Self::MovePlayerPosition", "            | Self::ClientCommand\\n            | Self::Attack\\n            | Self::MovePlayerPosition")'
if text.count(needle) != 2:
    raise RuntimeError(f"expected two duplicate protocol replacements, found {text.count(needle)}")
text = text.replace(needle, needle.replace("replace_once", "replace_first", 1), 1)
path.write_text(text, encoding="utf-8")
