from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
text = path.read_text(encoding="utf-8")
old = '''            [configuration]
            enabled = true
            features = "minecraft:vanilla;minecraft:trade_rebalance"
            hide_online_players = false
            enforces_secure_chat = true
            previews_chat = false
            server_icon = "data:image/png;base64,iVBORw0KGgo="
            sample_players = "Steve:00000000-0000-0000-0000-000000000000;Alex:11111111-1111-1111-1111-111111111111"
'''
new = '''            hide_online_players = false
            enforces_secure_chat = true
            previews_chat = false
            server_icon = "data:image/png;base64,iVBORw0KGgo="
            sample_players = "Steve:00000000-0000-0000-0000-000000000000;Alex:11111111-1111-1111-1111-111111111111"

            [configuration]
            enabled = true
            features = "minecraft:vanilla;minecraft:trade_rebalance"
'''
if text.count(old) != 1:
    raise RuntimeError(f"expected one misplaced fixture block, found {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
