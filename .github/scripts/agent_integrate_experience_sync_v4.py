from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[2]
runpy.run_path(str(ROOT / ".github/scripts/agent_integrate_experience_sync_v3.py"), run_name="__main__")

path = ROOT / "crates/rom-server/src/game_replication.rs"
text = path.read_text(encoding="utf-8")
old = '''        assert!(matches!(\n            recv_raw_output(&alex_writer, &mut workers, &mut inputs),\n            PlayOutput::ProtocolPacket {\n                kind: PacketKind::SetHealth,\n                ..\n            }\n        ));\n        assert!(matches!(\n            recv_output(&steve_writer, &mut workers, &mut inputs),'''
new = '''        assert!(matches!(\n            recv_raw_output(&alex_writer, &mut workers, &mut inputs),\n            PlayOutput::ProtocolPacket {\n                kind: PacketKind::SetHealth,\n                ..\n            }\n        ));\n        assert!(matches!(\n            recv_raw_output(&alex_writer, &mut workers, &mut inputs),\n            PlayOutput::ProtocolPacket {\n                kind: PacketKind::SetExperience,\n                ..\n            }\n        ));\n        assert!(matches!(\n            recv_output(&steve_writer, &mut workers, &mut inputs),'''
count = text.count(old)
if count != 1:
    raise RuntimeError(f"expected one health-test initialization marker, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Drained Alex initial experience packet before subject-only health assertion.")
