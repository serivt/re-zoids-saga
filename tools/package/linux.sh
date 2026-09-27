#!/usr/bin/env bash
# Builds the Linux package: the program and its texts in
# dist/re-zoids-saga-<version>-linux-x86_64.tar.gz.
# Usage: tools/package/linux.sh [version]

source "$(dirname "$0")/common.sh"

cargo build --release --locked -p launcher --features packaged

FOLDER="$EXECUTABLE-$VERSION"
mkdir -p "$STAGE/$FOLDER"
cp target/release/launcher "$STAGE/$FOLDER/$EXECUTABLE"
strip "$STAGE/$FOLDER/$EXECUTABLE"
add_texts "$STAGE/$FOLDER"

ARCHIVE="$DIST/$EXECUTABLE-$VERSION-linux-x86_64.tar.gz"
tar -czf "$ARCHIVE" -C "$STAGE" "$FOLDER"
echo "$ARCHIVE"
