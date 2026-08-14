from pathlib import Path

path = Path(__file__).with_name("agent_integrate_combat_loop.py")
text = path.read_text(encoding="utf-8")
old = "        assert_ne!(velocity, Velocity::ZERO);"
new = "        assert_ne!(velocity, Velocity::default());"
if text.count(old) != 1:
    raise RuntimeError(f"expected one velocity assertion, found {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
