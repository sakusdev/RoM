use std::{env, fs, path::PathBuf};

fn replace(path: &PathBuf, old: &str, new: &str) {
    let mut source = fs::read_to_string(path).expect("read bootstrap source");
    if source.contains(old) {
        source = source.replace(old, new);
        fs::write(path, source).expect("patch bootstrap source");
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let extract = manifest.join("src/extract.rs");
    replace(
        &extract,
        "ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackDataComponent,\n    RomPackItem, RomPackMetadata,",
        "ROMPACK_SCHEMA_VERSION, RomPack, RomPackBiomes, RomPackBlockStates, RomPackMetadata,",
    );

    let registry = manifest.join("src/registry_report.rs");
    replace(
        &registry,
        "pub fn read_item_registry_report(",
        "#[allow(dead_code)]\npub fn read_item_registry_report(",
    );
    replace(
        &registry,
        "pub fn parse_item_registry_report(",
        "#[allow(dead_code)]\npub fn parse_item_registry_report(",
    );
}
