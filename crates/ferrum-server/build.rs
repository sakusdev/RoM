use std::{env, fs, path::PathBuf};

fn replace(path: &PathBuf, old: &str, new: &str) {
    let mut source = fs::read_to_string(path).expect("read server source");
    if source.contains(old) {
        source = source.replace(old, new);
        fs::write(path, source).expect("patch server source");
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let play_runtime = manifest.join("src/play_runtime.rs");
    replace(
        &play_runtime,
        "BlockPosition, DataComponentProtocolRegistry, ItemProtocolRegistry, PlayerMovement,",
        "BlockPosition, ItemProtocolRegistry, PlayerMovement,",
    );
    replace(
        &play_runtime,
        "    components: &'a DataComponentProtocolRegistry,\n",
        "",
    );
    replace(
        &play_runtime,
        "        components: &'a DataComponentProtocolRegistry,\n",
        "",
    );
    replace(&play_runtime, "            components,\n", "");
    replace(
        &play_runtime,
        "decode_container_click(payload, self.items, self.components)",
        "decode_container_click(payload, self.items)",
    );
    replace(
        &play_runtime,
        "decode_creative_slot_update(payload, self.items, self.components)",
        "decode_creative_slot_update(payload, self.items)",
    );

    let main = manifest.join("src/main.rs");
    replace(
        &main,
        r#"    let gameplay = play_runtime::GameplaySync::new(
        &context.state.game_runtime,
        online_player.player_uuid(),
        &config.item_protocol_ids,
        &config.data_component_protocol_ids,
    );
"#,
        r#"    let gameplay = play_runtime::GameplaySync::new(
        &context.state.game_runtime,
        online_player.player_uuid(),
        &config.item_protocol_ids,
    );
"#,
    );

    let replication = manifest.join("src/game_replication.rs");
    replace(
        &replication,
        r#"        for slot in 0..PLAYER_INVENTORY_SLOTS {
            assert_eq!(
                recv_output(&writer, &mut workers, &mut inputs),
                PlayOutput::SetPlayerInventory { slot, stack: None }
            );
        }
"#,
        r#"        assert!(matches!(
            recv_output(&writer, &mut workers, &mut inputs),
            PlayOutput::SetContainerContent {
                container_id: 0,
                state_id: 0,
                slots,
                carried: None,
            } if slots.len() == PLAYER_INVENTORY_SLOTS && slots.iter().all(Option::is_none)
        ));
"#,
    );
}
