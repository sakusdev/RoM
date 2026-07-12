from pathlib import Path

path = Path("scripts/apply_packet_catalog_protocol.py")
text = path.read_text(encoding="utf-8")
text = text.replace("\n             |", "\n            |")
text = text.replace("\n             _ =>", "\n            _ =>")
path.write_text(text, encoding="utf-8")
