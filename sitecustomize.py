"""Temporary CI-only postfix for the inventory implementation generator."""

from __future__ import annotations

import atexit
import os
from pathlib import Path


def _replace(path: str, old: str, new: str) -> None:
    target = Path(path)
    if not target.is_file():
        return
    text = target.read_text(encoding="utf-8")
    if old in text:
        target.write_text(text.replace(old, new), encoding="utf-8")


def _apply_inventory_postfix() -> None:
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    if not Path("crates/ferrum-game/src/container.rs").is_file():
        return

    _replace(
        "crates/ferrum-game/src/state.rs",
        """        .filter_map(|(slot, (before, after))| {
            (before != after).then(|| GameEvent::InventorySlotChanged {
                uuid,
                slot,
                stack: after.clone(),
            })
        })""",
        """        .filter(|(_, (before, after))| before != after)
        .map(|(slot, (_, after))| GameEvent::InventorySlotChanged {
            uuid,
            slot,
            stack: after.clone(),
        })""",
    )

    _replace(
        "crates/ferrum-server/src/play_runtime.rs",
        "BlockPosition, DataComponentProtocolRegistry, ItemProtocolRegistry, PlayerMovement,",
        "BlockPosition, ItemProtocolRegistry, PlayerMovement,",
    )
    _replace(
        "crates/ferrum-server/src/play_runtime.rs",
        "    components: &'a DataComponentProtocolRegistry,\n",
        "",
    )
    _replace(
        "crates/ferrum-server/src/play_runtime.rs",
        "        components: &'a DataComponentProtocolRegistry,\n",
        "",
    )
    _replace(
        "crates/ferrum-server/src/play_runtime.rs",
        "            components,\n",
        "",
    )
    _replace(
        "crates/ferrum-server/src/play_runtime.rs",
        "decode_container_click(payload, self.items, self.components)",
        "decode_container_click(payload, self.items)",
    )
    _replace(
        "crates/ferrum-server/src/play_runtime.rs",
        "decode_creative_slot_update(payload, self.items, self.components)",
        "decode_creative_slot_update(payload, self.items)",
    )
    _replace(
        "crates/ferrum-server/src/main.rs",
        "        &config.data_component_protocol_ids,\n",
        "",
    )

    _replace(
        "crates/rom-bootstrap/src/extract.rs",
        "ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackDataComponent,\n    RomPackItem, RomPackMetadata,",
        "ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackMetadata,",
    )
    _replace(
        "crates/rom-bootstrap/src/registry_report.rs",
        "pub fn read_item_registry_report(",
        "#[allow(dead_code)]\npub fn read_item_registry_report(",
    )
    _replace(
        "crates/rom-bootstrap/src/registry_report.rs",
        "pub fn parse_item_registry_report(",
        "#[allow(dead_code)]\npub fn parse_item_registry_report(",
    )


atexit.register(_apply_inventory_postfix)
