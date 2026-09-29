#!/usr/bin/env bash
# Checks that the Android app installs, starts and stays up on the device
# adb reaches (an emulator): its activity has the focus, its process runs and
# the log shows no crash. Leaves a screenshot of what it shows. Never needs a
# ROM: without one the app shows the launcher's screen.
# Usage: tools/package/android-smoke.sh <app.apk> [screenshot.png]
#
# Needs adb on the PATH, or ANDROID_HOME; with several devices, ANDROID_SERIAL
# names the one to use.

set -euo pipefail

APK="${1:?usage: android-smoke.sh <app.apk> [screenshot.png]}"
SHOT="${2:-dist/android-smoke.png}"
PACKAGE="io.github.serivt.rezoidssaga"
ACTIVITY="$PACKAGE/.MainActivity"
STARTUP_SECONDS=15
ADB="$(command -v adb || echo "${ANDROID_HOME:?set ANDROID_HOME or put adb on the PATH}/platform-tools/adb")"

"$ADB" wait-for-device
"$ADB" install -r "$APK"
"$ADB" shell am force-stop "$PACKAGE"
"$ADB" logcat -c
"$ADB" shell am start -W -n "$ACTIVITY"
sleep "$STARTUP_SECONDS"

mkdir -p "$(dirname "$SHOT")"
"$ADB" exec-out screencap -p > "$SHOT"

failed=0
if ! "$ADB" shell pidof "$PACKAGE" > /dev/null; then
    echo "the app is not running" >&2
    failed=1
fi
if ! "$ADB" shell dumpsys window | grep -E "mCurrentFocus|mFocusedApp" | grep -q "$PACKAGE"; then
    echo "the app does not have the focus" >&2
    failed=1
fi
if "$ADB" logcat -d | grep -E "FATAL EXCEPTION|Fatal signal"; then
    echo "the log shows a crash" >&2
    failed=1
fi
if [[ "$failed" -ne 0 ]]; then
    "$ADB" logcat -d | tail -100 >&2
    exit 1
fi
echo "the app runs; screenshot in $SHOT"
