use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let path = manifest.join("src/lib.rs");
    let mut source = fs::read_to_string(&path).expect("read rompack source");
    let old = "            ],\n            packet_catalog: vec![";
    let new = "            ],\n            data_components: Vec::new(),\n            packet_catalog: vec![";
    if source.contains(old) && !source.contains(new) {
        source = source.replacen(old, new, 1);
        fs::write(path, source).expect("patch rompack test fixture");
    }
}
