from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")
old = '''            PlayOutputPacketIds {
                keep_alive_request: 0x43,
                disconnect: 0x44,
            },'''
new = '''            PlayOutputPacketIds {
                keep_alive_request: 0x43,
                disconnect: 0x44,
                system_chat: 0x45,
                player_position: 0x46,
            },'''
count = text.count(old)
if count != 3:
    raise SystemExit(f"expected 3 PlayOutputPacketIds test initializers, found {count}")
path.write_text(text.replace(old, new), encoding="utf-8")
