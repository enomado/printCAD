#!/usr/bin/env bash
# Build the app for a browser page into a folder (default `web/dist`): the
# release build for wasm32-unknown-unknown, its JavaScript glue
# (wasm-bindgen, the version Cargo.lock pins), shrunk by wasm-opt, and the
# page that starts it (web/index.html).
#
#   scripts/build-web.sh [out-dir]
#
# Needs the wasm32-unknown-unknown target, wasm-bindgen-cli at the
# wasm-bindgen version in Cargo.lock (WASM_BINDGEN names another binary),
# and wasm-opt (binaryen; WASM_OPT=0 skips it, keeping function names for
# a panic's stack).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/web/dist}"
bindgen="${WASM_BINDGEN:-wasm-bindgen}"
target_dir="${CARGO_TARGET_DIR:-$root/target}"

cargo build --release -p app_shell --target wasm32-unknown-unknown --manifest-path "$root/Cargo.toml"

rm -rf "$out"
mkdir -p "$out"
"$bindgen" --target web --no-typescript --out-dir "$out" \
  "$target_dir/wasm32-unknown-unknown/release/printcad.wasm"
# WASM_OPT=0 keeps the function names (and the size), for a panic's stack.
[[ "${WASM_OPT:-1}" == 0 ]] || wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  "$out/printcad_bg.wasm" -o "$out/printcad_bg.wasm"
cp "$root/web/index.html" "$out/"
cp "$root/crates/app_shell/assets/icon/printcad.svg" "$out/"
echo "built $out ($(du -h "$out/printcad_bg.wasm" | cut -f1) of WebAssembly)"
