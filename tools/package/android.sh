#!/usr/bin/env bash
# Builds the Android app: dist/re-zoids-saga-<version>-android.apk.
# Usage: tools/package/android.sh [version] [debug|release]
#
# Needs JAVA_HOME (a JDK 17), ANDROID_HOME (the Android SDK, with an NDK
# under ndk/), cargo-ndk and the Rust targets of $ABIS (arm64-v8a by default;
# add x86_64 for x86 emulators). A release build is signed with the keystore
# in ANDROID_KEYSTORE (ANDROID_KEYSTORE_PASSWORD, ANDROID_KEY_ALIAS,
# ANDROID_KEY_PASSWORD); a debug build with Android's debug key.

source "$(dirname "$0")/common.sh"

BUILD="${2:-release}"
ABIS="${ABIS:-arm64-v8a}"
PROJECT="apps/android/project"
JNI_LIBS="$PROJECT/app/src/main/jniLibs"
: "${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)}"
TOOLCHAIN="$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake"

rm -rf "$JNI_LIBS"
for ABI in $ABIS; do
    case "$ABI" in
        arm64-v8a) TRIPLE=aarch64_linux_android ;;
        x86_64) TRIPLE=x86_64_linux_android ;;
        *) echo "unsupported ABI $ABI" >&2; exit 1 ;;
    esac
    export "CMAKE_TOOLCHAIN_FILE_$TRIPLE=$TOOLCHAIN"
    cargo ndk -t "$ABI" -P 21 -o "$JNI_LIBS" build --release --locked -p re-zoids-saga-android
    # SDL's own build leaves libSDL3.so in its build script's folder: the
    # newest one for this target is the one just linked against.
    TARGET_DIR="target/${TRIPLE//_/-}/release/build"
    SDL_LIB=$(ls -t "$TARGET_DIR"/sdl3-sys-*/out/lib/libSDL3.so | head -1)
    cp "$SDL_LIB" "$JNI_LIBS/$ABI/"
done

TASK="assemble$(tr '[:lower:]' '[:upper:]' <<< "${BUILD:0:1}")${BUILD:1}"
(cd "$PROJECT" && ./gradlew --quiet "$TASK")
APK=$(ls "$PROJECT"/app/build/outputs/apk/"$BUILD"/*.apk | head -1)
PACKAGE="$DIST/$EXECUTABLE-$VERSION-android.apk"
cp "$APK" "$PACKAGE"
echo "$PACKAGE"
