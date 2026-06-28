#!/usr/bin/env sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
rm -rf "$ROOT/out"
mkdir -p "$ROOT/out/classes"
javac --release 21 -d "$ROOT/out/classes" "$ROOT/src/Sample.java"
printf 'Manifest-Version: 1.0\nMain-Class: example.Sample\n\n' > "$ROOT/out/MANIFEST.MF"
jar --create --file "$ROOT/out/sample.jar" --manifest "$ROOT/out/MANIFEST.MF" -C "$ROOT/out/classes" .
cp "$ROOT/out/sample.jar" "$ROOT/../../crates/ferrum-importer/tests/fixtures/sample.jar"
