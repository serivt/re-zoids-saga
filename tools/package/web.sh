#!/usr/bin/env bash
# Builds the web version: dist/web/, a folder any static web server can
# serve (the page, its scripts and style, the game's WebAssembly and its
# JavaScript glue, the manifest and the service worker that caches them
# under this version, so the page installs and plays offline), and the same
# folder as dist/re-zoids-saga-<version>-web.zip.
# Usage: tools/package/web.sh [version]
#
# Needs the wasm32-unknown-unknown Rust target and wasm-bindgen-cli at the
# version of the wasm-bindgen crate in Cargo.lock. With SUPABASE_URL and
# SUPABASE_ANON_KEY (the project's public key) the page offers the cloud
# saves (services/cloud); without them it offers none.

source "$(dirname "$0")/common.sh"

OUT="$DIST/web"
WASM="target/wasm32-unknown-unknown/release/re_zoids_saga_web.wasm"

rm -rf "$OUT"
mkdir -p "$OUT/pkg"
cargo build --release --locked --target wasm32-unknown-unknown -p re-zoids-saga-web
wasm-bindgen --target web --no-typescript --out-dir "$OUT/pkg" "$WASM"
cp apps/web/static/* "$OUT/"
if [ -n "${SUPABASE_URL:-}" ] && [ -n "${SUPABASE_ANON_KEY:-}" ]; then
    printf "export const CLOUD = { url: '%s', anonKey: '%s' };\n" \
        "$SUPABASE_URL" "$SUPABASE_ANON_KEY" > "$OUT/config.js"
fi
for FILE in sw.js index.html; do
    sed -i.bak "s/__VERSION__/$VERSION/" "$OUT/$FILE"
    rm "$OUT/$FILE.bak"
done
cp assets/icons/re-zoids-saga.png "$OUT/icon.png"
cp LICENSE "$OUT/LICENSE.txt"
ARCHIVE="$EXECUTABLE-$VERSION-web.zip"
rm -f "$DIST/$ARCHIVE"
(cd "$OUT" && zip -qr "../$ARCHIVE" .)
echo "Built $OUT and $DIST/$ARCHIVE ($VERSION); serve it with, for example: python3 -m http.server -d $OUT"
