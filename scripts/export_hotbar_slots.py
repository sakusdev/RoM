from pathlib import Path

path = Path("crates/ferrum-game/src/lib.rs")
text = path.read_text(encoding="utf-8")
old = '''pub use inventory::{
    EquipmentSlot, Inventory, InventoryError, ItemStack, MAX_VANILLA_STACK_SIZE,
    PLAYER_INVENTORY_SLOTS,
};'''
new = '''pub use inventory::{
    EquipmentSlot, HOTBAR_SLOTS, Inventory, InventoryError, ItemStack, MAX_VANILLA_STACK_SIZE,
    PLAYER_INVENTORY_SLOTS,
};'''
if text.count(old) != 1:
    raise SystemExit("ferrum-game inventory re-export target not found exactly once")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
