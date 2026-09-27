# Shared by the packaging scripts: the version, the names and the folders.
# Source it from the repository's root.

set -euo pipefail

VERSION="${1:-$(git describe --tags --always --dirty)}"
EXECUTABLE="re-zoids-saga"
APP_NAME="Re:Zoids Saga"
BUNDLE_NAME="Re Zoids Saga"
BUNDLE_ID="io.github.serivt.re-zoids-saga"
DIST="dist"
STAGE="$DIST/stage"

rm -rf "$STAGE"
mkdir -p "$STAGE"

# Copies the texts every package carries into the folder given.
add_texts() {
    cp LICENSE "$1/LICENSE.txt"
    cp tools/package/README-player.txt "$1/README.txt"
}
