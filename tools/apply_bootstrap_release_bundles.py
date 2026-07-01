from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}: {old[:180]!r}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


release = ".github/workflows/release.yml"
replace_once(release, "name: Release RoM server binaries\n", "name: Release RoM native binaries\n")

matrix_replacements = [
    (
        "            source_binary: ferrum-server.exe\n"
        "            package_binary: ferrum-server.exe\n"
        "            raw_asset: ferrum-server-windows-x86_64.exe\n",
        "            source_binary: ferrum-server.exe\n"
        "            package_binary: ferrum-server.exe\n"
        "            raw_asset: ferrum-server-windows-x86_64.exe\n"
        "            bootstrap_binary: rom-bootstrap.exe\n"
        "            bootstrap_raw_asset: rom-bootstrap-windows-x86_64.exe\n",
    ),
    (
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-linux-x86_64\n",
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-linux-x86_64\n"
        "            bootstrap_binary: rom-bootstrap\n"
        "            bootstrap_raw_asset: rom-bootstrap-linux-x86_64\n",
    ),
    (
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-linux-aarch64\n",
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-linux-aarch64\n"
        "            bootstrap_binary: rom-bootstrap\n"
        "            bootstrap_raw_asset: rom-bootstrap-linux-aarch64\n",
    ),
    (
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-macos-x86_64\n",
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-macos-x86_64\n"
        "            bootstrap_binary: rom-bootstrap\n"
        "            bootstrap_raw_asset: rom-bootstrap-macos-x86_64\n",
    ),
    (
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-macos-aarch64\n",
        "            source_binary: ferrum-server\n"
        "            package_binary: ferrum-server\n"
        "            raw_asset: ferrum-server-macos-aarch64\n"
        "            bootstrap_binary: rom-bootstrap\n"
        "            bootstrap_raw_asset: rom-bootstrap-macos-aarch64\n",
    ),
]
for old, new in matrix_replacements:
    replace_once(release, old, new)

replace_once(
    release,
    "      - name: Build native server\n"
    "        if: matrix.target != 'aarch64-unknown-linux-gnu'\n"
    "        run: cargo build --locked --release -p ferrum-server --target ${{ matrix.target }}\n",
    "      - name: Build native server and Bootstrap\n"
    "        if: matrix.target != 'aarch64-unknown-linux-gnu'\n"
    "        run: cargo build --locked --release -p ferrum-server -p rom-bootstrap --target ${{ matrix.target }}\n",
)
replace_once(
    release,
    "      - name: Build Linux ARM64 server\n"
    "        if: matrix.target == 'aarch64-unknown-linux-gnu'\n"
    "        shell: bash\n"
    "        env:\n"
    "          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc\n"
    "        run: cargo build --locked --release -p ferrum-server --target ${{ matrix.target }}\n",
    "      - name: Build Linux ARM64 server and Bootstrap\n"
    "        if: matrix.target == 'aarch64-unknown-linux-gnu'\n"
    "        shell: bash\n"
    "        env:\n"
    "          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc\n"
    "        run: cargo build --locked --release -p ferrum-server -p rom-bootstrap --target ${{ matrix.target }}\n",
)
replace_once(
    release,
    "          & \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" --help | Out-Null\n"
    "          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n",
    "          & \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" --help | Out-Null\n"
    "          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n"
    "          & \"target/${{ matrix.target }}/release/${{ matrix.bootstrap_binary }}\" --help | Out-Null\n"
    "          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n",
)
replace_once(
    release,
    "          \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" --help >/dev/null\n",
    "          \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" --help >/dev/null\n"
    "          \"target/${{ matrix.target }}/release/${{ matrix.bootstrap_binary }}\" --help >/dev/null\n",
)
replace_once(
    release,
    "          Copy-Item \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"$stage/${{ matrix.package_binary }}\"\n"
    "          Copy-Item \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"dist/${{ matrix.raw_asset }}\"\n"
    "          Copy-Item \"examples/server-26.1.2.toml\" \"$stage/server.toml\"\n"
    "          Copy-Item README.md \"$stage/README.md\"\n"
    "          Copy-Item LICENSE \"$stage/LICENSE\"\n",
    "          Copy-Item \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"$stage/${{ matrix.package_binary }}\"\n"
    "          Copy-Item \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"dist/${{ matrix.raw_asset }}\"\n"
    "          Copy-Item \"target/${{ matrix.target }}/release/${{ matrix.bootstrap_binary }}\" \"$stage/${{ matrix.bootstrap_binary }}\"\n"
    "          Copy-Item \"target/${{ matrix.target }}/release/${{ matrix.bootstrap_binary }}\" \"dist/${{ matrix.bootstrap_raw_asset }}\"\n"
    "          Copy-Item \"examples/server-26.1.2.toml\" \"$stage/server.toml\"\n"
    "          Copy-Item README.md \"$stage/README.md\"\n"
    "          Copy-Item NOTICE.md \"$stage/NOTICE.md\"\n"
    "          Copy-Item LICENSE \"$stage/LICENSE\"\n"
    "          New-Item -ItemType Directory -Force -Path \"$stage/docs\" | Out-Null\n"
    "          Copy-Item \"docs/BOOTSTRAP.md\" \"$stage/docs/BOOTSTRAP.md\"\n",
)
replace_once(
    release,
    "          install -m 0755 \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"$stage/${{ matrix.package_binary }}\"\n"
    "          install -m 0755 \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"dist/${{ matrix.raw_asset }}\"\n"
    "          cp examples/server-26.1.2.toml \"$stage/server.toml\"\n"
    "          cp README.md \"$stage/README.md\"\n"
    "          cp LICENSE \"$stage/LICENSE\"\n",
    "          install -m 0755 \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"$stage/${{ matrix.package_binary }}\"\n"
    "          install -m 0755 \"target/${{ matrix.target }}/release/${{ matrix.source_binary }}\" \"dist/${{ matrix.raw_asset }}\"\n"
    "          install -m 0755 \"target/${{ matrix.target }}/release/${{ matrix.bootstrap_binary }}\" \"$stage/${{ matrix.bootstrap_binary }}\"\n"
    "          install -m 0755 \"target/${{ matrix.target }}/release/${{ matrix.bootstrap_binary }}\" \"dist/${{ matrix.bootstrap_raw_asset }}\"\n"
    "          cp examples/server-26.1.2.toml \"$stage/server.toml\"\n"
    "          cp README.md \"$stage/README.md\"\n"
    "          cp NOTICE.md \"$stage/NOTICE.md\"\n"
    "          cp LICENSE \"$stage/LICENSE\"\n"
    "          mkdir -p \"$stage/docs\"\n"
    "          cp docs/BOOTSTRAP.md \"$stage/docs/BOOTSTRAP.md\"\n",
)
replace_once(
    release,
    "          for asset in \"${{ matrix.raw_asset }}\" *.tar.gz; do\n",
    "          for asset in \"${{ matrix.raw_asset }}\" \"${{ matrix.bootstrap_raw_asset }}\" *.tar.gz; do\n",
)
replace_once(
    release,
    "            dist/ferrum-server-*\n"
    "            dist/*.sha256\n",
    "            dist/ferrum-server-*\n"
    "            dist/rom-bootstrap-*\n"
    "            dist/*.sha256\n",
)
replace_once(
    release,
    "          test \"$(wc -l < SHA256SUMS)\" -eq 10\n",
    "          test \"$(wc -l < SHA256SUMS)\" -eq 15\n",
)
replace_once(
    release,
    "          Each platform provides a standalone ferrum-server executable and an archive containing the executable, server.toml, README.md, LICENSE, and VERSION. Verify downloads with SHA256SUMS.\n",
    "          Each platform provides standalone ferrum-server and rom-bootstrap executables. The archive contains both binaries, server.toml, README.md, NOTICE.md, LICENSE, Bootstrap documentation, and VERSION. Verify downloads with SHA256SUMS.\n",
)
replace_once(
    release,
    "          test \"$(gh release view \"$TAG\" --json assets --jq '.assets | length')\" -ge 21\n",
    "          test \"$(gh release view \"$TAG\" --json assets --jq '.assets | length')\" -ge 31\n",
)

bootstrap = "crates/rom-bootstrap/src/lib.rs"
replace_once(
    bootstrap,
    "    let native_binary = instance.join(\"bin\").join(native_server_file_name());\n"
    "    let installed = native_binary.is_file();\n\n"
    "    Ok(StatusReport {\n",
    "    let native_binary = installed_native_server(&instance);\n"
    "    let installed = native_binary.is_some();\n\n"
    "    Ok(StatusReport {\n",
)
replace_once(
    bootstrap,
    "        native_server_binary: installed.then_some(native_binary),\n",
    "        native_server_binary: native_binary,\n",
)
replace_once(
    bootstrap,
    "fn native_server_file_name() -> &'static str {\n"
    "    if cfg!(windows) {\n"
    "        \"rom-server.exe\"\n"
    "    } else {\n"
    "        \"rom-server\"\n"
    "    }\n"
    "}\n",
    "fn installed_native_server(instance: &Path) -> Option<PathBuf> {\n"
    "    let current = instance.join(\"bin\").join(native_server_file_name());\n"
    "    if current.is_file() {\n"
    "        return Some(current);\n"
    "    }\n"
    "    let legacy = instance\n"
    "        .join(\"bin\")\n"
    "        .join(legacy_native_server_file_name());\n"
    "    legacy.is_file().then_some(legacy)\n"
    "}\n\n"
    "fn native_server_file_name() -> &'static str {\n"
    "    if cfg!(windows) {\n"
    "        \"ferrum-server.exe\"\n"
    "    } else {\n"
    "        \"ferrum-server\"\n"
    "    }\n"
    "}\n\n"
    "fn legacy_native_server_file_name() -> &'static str {\n"
    "    if cfg!(windows) {\n"
    "        \"rom-server.exe\"\n"
    "    } else {\n"
    "        \"rom-server\"\n"
    "    }\n"
    "}\n",
)
replace_once(
    bootstrap,
    "    #[test]\n    fn existing_server_configuration_is_preserved() {\n",
    "    #[test]\n"
    "    fn native_server_uses_release_name_and_detects_legacy_instances() {\n"
    "        let directory = tempdir().unwrap();\n"
    "        let instance = directory.path();\n"
    "        fs::create_dir_all(instance.join(\"bin\")).unwrap();\n"
    "        let legacy = instance.join(\"bin\").join(legacy_native_server_file_name());\n"
    "        fs::write(&legacy, b\"legacy\").unwrap();\n"
    "        assert_eq!(installed_native_server(instance), Some(legacy));\n"
    "        assert!(native_server_file_name().starts_with(\"ferrum-server\"));\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn existing_server_configuration_is_preserved() {\n",
)

readme = "README.md"
replace_once(readme, "│   └── rom-server\n", "│   └── ferrum-server\n")
replace_once(
    readme,
    "`ferrum-server` is released as a platform-native server executable for:\n",
    "`ferrum-server` and `rom-bootstrap` are released as platform-native executables for:\n",
)
replace_once(
    readme,
    "Release archives must contain RoM binaries, configuration, documentation, license, notice, and version information only. They must not contain the official Minecraft server JAR or generated data copied from it.\n\n"
    "Expected server usage:\n",
    "Each release archive contains both binaries, `server.toml`, the Bootstrap guide, README, NOTICE, LICENSE, and VERSION. Standalone binaries are also published separately. Official Minecraft files and locally generated `.rompack` files are never bundled.\n\n"
    "Preferred first-run usage from an extracted release archive:\n\n"
    "```bash\n"
    "./rom-bootstrap prepare --instance ./rom-instance --version 26.1.2 --accept-minecraft-eula\n"
    "./rom-bootstrap generate --instance ./rom-instance\n"
    "./rom-bootstrap install-local --instance ./rom-instance --server-binary ./ferrum-server\n"
    "./rom-bootstrap run --instance ./rom-instance\n"
    "```\n\n"
    "Expected direct server usage:\n",
)

docs = "docs/BOOTSTRAP.md"
replace_once(docs, "│   └── rom-server\n", "│   └── ferrum-server\n")
replace_once(
    docs,
    "## Public server warning\n",
    "## Native release archives\n\n"
    "Platform release archives contain both `rom-bootstrap` and `ferrum-server`. After extracting an archive, prepare and generate the local instance, then install the adjacent server binary:\n\n"
    "```bash\n"
    "./rom-bootstrap prepare --instance ./rom-instance --version 26.1.2 --accept-minecraft-eula\n"
    "./rom-bootstrap generate --instance ./rom-instance\n"
    "./rom-bootstrap install-local --instance ./rom-instance --server-binary ./ferrum-server\n"
    "./rom-bootstrap run --instance ./rom-instance\n"
    "```\n\n"
    "Existing development instances containing `bin/rom-server` remain readable, but new installations use `bin/ferrum-server`.\n\n"
    "## Public server warning\n",
)
replace_once(
    docs,
    "1. Package `rom-bootstrap` alongside `rom-server` in native release archives.\n"
    "2. Move packet tables and additional version-specific runtime metadata into generated packs.\n"
    "3. Add more independently testable extractors only when the server consumes their output.\n"
    "4. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.\n",
    "1. Move packet tables and additional version-specific runtime metadata into generated packs.\n"
    "2. Add more independently testable extractors only when the server consumes their output.\n"
    "3. Preserve bounded decoding, deterministic ordering, source hashes, and local-only artifact boundaries for every new section.\n",
)
