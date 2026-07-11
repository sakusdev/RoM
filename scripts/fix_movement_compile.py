from pathlib import Path

main = Path("crates/ferrum-server/src/main.rs")
text = main.read_text(encoding="utf-8")
old = "                return Err(error);"
new = "                return Err(error.into());"
if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit("main.rs WorkerControlError conversion target not found")
main.write_text(text, encoding="utf-8")

runtime = Path("crates/ferrum-server/src/play_runtime.rs")
text = runtime.read_text(encoding="utf-8")
old = '''            Some(&reader),
            Some(1),
        )'''
new = '''            Some(&reader),
            None,
            Some(1),
        )'''
if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit("play_runtime.rs bridge test target not found")
runtime.write_text(text, encoding="utf-8")
