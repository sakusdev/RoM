from pathlib import Path

path = Path("crates/ferrum-server/src/main.rs")
source = path.read_text()
for signature in [
    "fn run_static_play_session<R: Read, W: Write>(",
    "fn run_keep_alive_loop<R: Read, W: Write>(",
]:
    marker = "#[allow(dead_code)]\n" + signature
    if marker in source:
        continue
    if signature not in source:
        raise SystemExit(f"missing compatibility wrapper: {signature}")
    source = source.replace(signature, marker, 1)
path.write_text(source)
