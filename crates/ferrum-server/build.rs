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
        "        &config.data_component_protocol_ids,\n",
        "",
    );
}
