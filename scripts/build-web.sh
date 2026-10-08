#!/usr/bin/env bash
# Build the app for a browser page into a folder (default `web/dist`): the
# release build for wasm32-unknown-unknown, its JavaScript glue
# (wasm-bindgen, the version Cargo.lock pins), shrunk by wasm-opt, and the
# page that starts it (web/index.html). A second build with shared memory
# goes to `threads/`, which the page loads where its workers may share
# memory (cross-origin isolated): the kernel's parallel stages then run on
# web workers. It rebuilds the standard library with atomics, which stable
# Rust allows through RUSTC_BOOTSTRAP; WEB_THREADS=0 skips it.
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

# One build into a folder: `module <out> <target-dir> [cargo args…]`.
module() {
  local into="$1" target="$2"
  shift 2
  cargo build --release -p app_shell --target wasm32-unknown-unknown \
    --manifest-path "$root/Cargo.toml" --target-dir "$target" "$@"
  mkdir -p "$into"
  "$bindgen" --target web --no-typescript --out-dir "$into" \
    "$target/wasm32-unknown-unknown/release/printcad.wasm"
  # WASM_OPT=0 keeps the function names (and the size), for a panic's stack.
  [[ "${WASM_OPT:-1}" == 0 ]] || wasm-opt -Oz --enable-bulk-memory \
    --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
    --enable-reference-types --enable-multivalue --enable-threads \
    "$into/printcad_bg.wasm" -o "$into/printcad_bg.wasm"
}

rm -rf "$out"
module "$out" "$target_dir"
if [[ "${WEB_THREADS:-1}" != 0 ]]; then
  RUSTC_BOOTSTRAP=1 \
    RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals
      -C link-arg=--shared-memory -C link-arg=--import-memory
      -C link-arg=--max-memory=4294967296
      -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size
      -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base" \
    module "$out/threads" "$target_dir/threads" -Z build-std=panic_abort,std
fi
cp "$root/web/index.html" "$root/web/kernel-worker.js" "$root/web/lua-worker.js" \
  "$root/web/package-worker.js" "$out/"
mkdir -p "$out/jco"
cp "$root"/third_party/jco/*.js "$root"/third_party/jco/*.wasm "$out/jco/"
mkdir -p "$out/wasmoon"
cp "$root/third_party/wasmoon/index.js" "$root/third_party/wasmoon/glue.wasm" "$out/wasmoon/"
cp "$root/crates/app_shell/assets/icon/printcad.svg" "$root/third_party/coi-serviceworker/coi-serviceworker.js" "$out/"
echo "built $out ($(du -h "$out/printcad_bg.wasm" | cut -f1) of WebAssembly$([[ -d "$out/threads" ]] && echo ", and threads/"))"
