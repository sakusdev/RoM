from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[2]
runpy.run_path(str(ROOT / ".github/scripts/agent_integrate_experience_sync_v2.py"), run_name="__main__")

path = ROOT / "crates/rom-server/src/game_replication.rs"
text = path.read_text(encoding="utf-8")
old = '''            if matches!(\n                output,\n                PlayOutput::ProtocolPacket {\n                    kind: PacketKind::SetHealth,\n                    ..\n                }\n            ) {\n                continue;\n            }'''
new = '''            if matches!(\n                output,\n                PlayOutput::ProtocolPacket {\n                    kind: PacketKind::SetHealth | PacketKind::SetExperience,\n                    ..\n                }\n            ) {\n                continue;\n            }'''
if text.count(old) != 1:
    raise RuntimeError(f"expected one replication helper marker, found {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Updated replication test helper for initial experience synchronization.")
