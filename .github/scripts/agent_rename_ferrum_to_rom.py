from pathlib import Path
import os

ROOT = Path(__file__).resolve().parents[2]

DIR_RENAMES = {
    "ferrum-model": "rom-model",
    "ferrum-importer": "rom-importer",
    "ferrum-cli": "rom-cli",
    "ferrum-server": "rom-server",
    "ferrum-runtime": "rom-runtime",
    "ferrum-nbt": "rom-nbt",
    "ferrum-protocol": "rom-protocol",
    "ferrum-configuration": "rom-configuration",
    "ferrum-play": "rom-play",
    "ferrum-world": "rom-world",
    "ferrum-game": "rom-game",
    "ferrum-version-26-1-2": "rom-version-26-1-2",
    "ferrum-rompack": "rom-pack",
}

TEXT_SUFFIXES = {
    ".rs", ".toml", ".md", ".yml", ".yaml", ".sh", ".cmd", ".ps1", ".json", ".txt", ".lock"
}
TEXT_NAMES = {"Dockerfile", "Justfile", "Makefile"}

# Preserve references to the separate FerrumC project while renaming this project's
# own Ferrum branding and crate identifiers.
PLACEHOLDER = "__ROM_PRESERVE_FERRUMC__"


def should_edit(path: Path) -> bool:
    if ".git" in path.parts or "target" in path.parts:
        return False
    return path.suffix in TEXT_SUFFIXES or path.name in TEXT_NAMES


def rewrite(text: str) -> str:
    text = text.replace("FerrumC", PLACEHOLDER)
    text = text.replace("ferrum-rompack", "rom-pack")
    text = text.replace("ferrum_rompack", "rom_pack")
    text = text.replace("FERRUM_ROMPACK", "ROM_PACK")
    text = text.replace("ferrum-", "rom-")
    text = text.replace("ferrum_", "rom_")
    text = text.replace("FERRUM_", "ROM_")
    text = text.replace("Ferrum", "RoM")
    text = text.replace("FERRUM", "ROM")
    text = text.replace(PLACEHOLDER, "FerrumC")
    return text


# Rename crate directories first. os.rename preserves all files and makes Git see
# these as normal renames instead of delete/recreate churn.
crates = ROOT / "crates"
for old, new in DIR_RENAMES.items():
    src = crates / old
    dst = crates / new
    if src.exists():
        if dst.exists():
            raise RuntimeError(f"destination already exists: {dst}")
        os.rename(src, dst)

# Rename the top-level design document as part of the public branding cleanup.
old_doc = ROOT / "FERRUM_PORTING_KIT_CONCEPT.md"
new_doc = ROOT / "ROM_PORTING_KIT_CONCEPT.md"
if old_doc.exists():
    if new_doc.exists():
        raise RuntimeError(f"destination already exists: {new_doc}")
    os.rename(old_doc, new_doc)

for path in ROOT.rglob("*"):
    if not path.is_file() or not should_edit(path):
        continue
    try:
        original = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        continue
    updated = rewrite(original)
    if updated != original:
        path.write_text(updated, encoding="utf-8")

# The public server executable is the product name itself, while the package stays
# rom-server to keep workspace naming regular.
server_manifest = ROOT / "crates" / "rom-server" / "Cargo.toml"
manifest = server_manifest.read_text(encoding="utf-8")
manifest = manifest.replace('[[bin]]\nname = "rom-server"\npath = "src/main.rs"', '[[bin]]\nname = "rom"\npath = "src/main.rs"')
server_manifest.write_text(manifest, encoding="utf-8")

# Guardrails: old internal crate/package identifiers and paths must be gone.
allowed = {
    ROOT / ".github" / "scripts" / "agent_rename_ferrum_to_rom.py",
}
violations = []
for path in ROOT.rglob("*"):
    if not path.is_file() or path in allowed or not should_edit(path):
        continue
    try:
        text = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        continue
    if "ferrum-" in text or "ferrum_" in text:
        violations.append(str(path.relative_to(ROOT)))
if violations:
    raise RuntimeError("legacy Ferrum crate identifiers remain in: " + ", ".join(violations))

print("Renamed Ferrum internal crates and branding to RoM.")
