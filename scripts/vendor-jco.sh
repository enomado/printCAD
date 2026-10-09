#!/bin/sh
# Bundle what a browser page needs to run workbench packages into
# third_party/jco: the component transpiler (jco's, Apache-2.0 WITH
# LLVM-exception) as one module with its two core modules beside it, and
# the WASI shims (preview2-shim) as another. Run once when bumping the
# versions; the files are committed. Needs node and npm.
set -eu
TRANSPILE=0.18.0
SHIM=0.28.0
ESBUILD=0.25.10
dest="$(git rev-parse --show-toplevel)/third_party/jco"
tmp="$(mktemp -d "${TMPDIR:-/var/tmp}/vendor-jco.XXXX")"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"
npm init -y >/dev/null
npm install --silent --no-audit --no-fund \
  "@bytecodealliance/jco-transpile@$TRANSPILE" \
  "@bytecodealliance/preview2-shim@$SHIM" \
  "esbuild@$ESBUILD"

cat > transpile-entry.js <<'EOF'
import { $init, generate } from "@bytecodealliance/jco-transpile/component";

// A component's bytes as the files of a module the page imports: its
// `instantiate` takes the core modules and every import, so nothing is
// fetched by name.
export async function transpile(bytes, name) {
  await $init;
  const { files } = generate(bytes, {
    name,
    instantiation: { tag: "async" },
    noTypescript: true,
    noNodejsCompat: true,
    map: [],
  });
  return files;
}
EOF

cat > wasi-entry.js <<'EOF'
import { WASIShim } from "@bytecodealliance/preview2-shim/instantiation";

// The WASI a package's instance sees on a page: no files, no network, no
// environment.
export function wasiImports() {
  return new WASIShim({
    sandbox: { preopens: {}, env: {}, args: ["bench"], enableNetwork: false },
  }).getImportObject();
}
EOF

bundle() {
  npx esbuild "$1" --bundle --format=esm --platform=browser --conditions=browser \
    --external:node:* --log-level=warning --outfile="$2"
}
mkdir -p "$dest"
bundle transpile-entry.js "$dest/transpile.js"
bundle wasi-entry.js "$dest/wasi.js"
vendor=node_modules/@bytecodealliance/jco-transpile/vendor
cp "$vendor/js-component-bindgen-component.core.wasm" \
  "$vendor/js-component-bindgen-component.core2.wasm" "$dest/"
cp node_modules/@bytecodealliance/jco-transpile/LICENSE "$dest/LICENSE"
cat > "$dest/VENDORED.md" <<EOF
# jco

Bundled by \`scripts/vendor-jco.sh\` from npm:

- \`@bytecodealliance/jco-transpile\` $TRANSPILE: \`transpile.js\` and the
  two core modules beside it, which it loads by their names.
- \`@bytecodealliance/preview2-shim\` $SHIM: \`wasi.js\`.

Both are the Bytecode Alliance's, Apache-2.0 WITH LLVM-exception
(\`LICENSE\`). The browser build copies this folder to \`jco/\` beside the
page.
EOF
echo "vendored into $dest"
