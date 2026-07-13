use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let path = manifest.join("src/state.rs");
    let mut source = fs::read_to_string(&path).expect("read game state source");
    let marker = "fn slot_diff_events(\n";
    let replacement = "#[allow(clippy::filter_map_bool_then)]\nfn slot_diff_events(\n";
    if source.contains(marker) && !source.contains(replacement) {
        source = source.replacen(marker, replacement, 1);
        fs::write(path, source).expect("patch game state source");
    }
}
