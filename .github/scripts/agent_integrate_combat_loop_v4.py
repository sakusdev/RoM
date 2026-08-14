from pathlib import Path

path = Path(__file__).with_name("agent_integrate_combat_loop.py")
text = path.read_text(encoding="utf-8")
old = "                    .map_err(|error| rom_game::GameStateError::Inventory(error).into())?;"
new = "                    .map_err(|error| GameRuntimeError::State(rom_game::GameStateError::Inventory(error)))?;"
if text.count(old) != 1:
    raise RuntimeError(f"expected one ambiguous error conversion, found {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
