#!/usr/bin/env sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
OUT="$ROOT/out"
rm -rf "$OUT"
mkdir -p "$OUT/classes"
javac -g -d "$OUT/classes" "$ROOT/src/m1/BytecodeFeatures.java"
jar --create --file "$OUT/m1-bytecode.jar" -C "$OUT/classes" .
