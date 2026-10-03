# Android

The same launcher and game as on the desktop, as an app for Android 5.0 or later. It is
installed from its APK, downloaded from the project's releases; it is not in any store.

Source of knowledge: this project's own design. The app's lifecycle, file dialog and
keys are SDL3's for Android (its `docs/README-android.md` and `src/core/android`,
`src/video/android` in SDL 3.4.16). Implemented in `apps/android`, `apps/launcher` and
`platform-sdl3`.

## Installing

1. Download `re-zoids-saga-<version>-android.apk` from the
   [Releases](https://github.com/serivt/re-zoids-saga/releases) page on the phone (its
   `.sha256` file holds its checksum).
2. Open it. Android asks to allow the browser or the file manager to install unknown
   apps: allow it, then Install. Play Protect may warn about an app it does not know;
   choose to install anyway.
3. Open *Re:Zoids Saga*, choose your ROM (Japan, Rev 1) and, optionally, a translation,
   then Play.

A newer version installs over the old one and keeps the ROM, the settings and the saves,
because every release is signed with the same key. Uninstalling the app deletes them
all: export the saves you want to keep first (see [Saves](#saves)).

The APK holds the program for `arm64-v8a`, the phones' processors, and `x86_64`, for
Chromebooks and the emulators of a PC.

## Playing

The ROM and the translation chosen in Android's file dialog are copied into the app's own
folder (the ROM as `rom.gba`, a translation under its language's code, `es.po`), and the
copies are what the app remembers, so the originals can be moved or deleted. A
translation can also be downloaded from the Translation line, as on the desktop.

The launcher answers taps: a tap chooses the line under it and activates it (on the
volume and the pad's lines, the left half lowers the value and the right half raises
it), and a tap on Yes or No answers the question to quit.

While playing, the on-screen pad surrounds the game's screen: the cross and B, A on the
sides and L, R, SELECT and START in the corners when the phone is held sideways; under
the screen when it is upright. In the enhanced mode a `>>` button plays the game fast
while it is held (see [launcher.md](launcher.md)): at the top in the middle, over the game
screen's edge, when the phone is sideways, and between L and R when it is upright. The cross takes the eight directions and several fingers
press at once. It hides while a gamepad is connected (Bluetooth or USB), which then
plays as on the desktop.

Android's back button does what Esc does on the desktop: it goes back on the launcher's
screens, and on its main screen or while the game plays it asks before closing. When the
app goes to the background (home, another app, the screen off), the game and its sound
stop until it comes back; the screen stays on while the app is in front.

## Options

Android's Options holds the gamepad's buttons, Display (the filter, the colors and the LCD
trail, see [launcher.md](launcher.md)), the volume and:

- **Pad size:** 60 % to 140 % of the pad's usual size, in steps of 10;
- **Pad opacity:** 20 % to 100 %;
- **Saves:** the save slots, to export and import (below).

The window's size, fullscreen and the keyboard's keys are not offered: the app always
fills the screen.

## Saves

The saves are kept in the app's own folder, beside the ROM's copy, in the original's
format (slot 1 `rom.sav`, slot n `rom.n.sav`, the autosave `rom.auto.sav`), out of reach
of other apps. Options › Saves exports them through Android's dialog (the Download
folder, a cloud drive...) as `.sav` for an emulator, `.srm` for RetroArch or as the
cartridge would hold them, and imports a `.sav` or `.srm` after showing what it holds
against what the slot holds; see [launcher.md](launcher.md), Saves. Android's dialog
opens without a name: type one ending in the kind's extension.

## Building

`tools/package/android.sh [version] [debug|release]` builds
`dist/re-zoids-saga-<version>-android.apk`. It needs:

- a JDK 17 in `JAVA_HOME`;
- the Android SDK in `ANDROID_HOME`, with platform and build-tools 35 and an NDK (r27d is
  the one tested; `ANDROID_NDK_HOME` picks one, else the newest under `ndk/`);
- `cargo-ndk` and the Rust targets of the ABIs in `ABIS` (`arm64-v8a` by default,
  `x86_64` too with `ABIS="arm64-v8a x86_64"`).

On macOS: `brew install openjdk@17 android-commandlinetools`, then `sdkmanager
"platforms;android-35" "build-tools;35.0.0" "ndk;27.3.13750724"`, `rustup target add
aarch64-linux-android x86_64-linux-android` and `cargo install cargo-ndk`.

The script builds `libmain.so` (the `apps/android` crate, whose `SDL_main` runs the
launcher) with SDL3 compiled from source as `libSDL3.so` beside it, copies both into the
Gradle project in `apps/android/project` and assembles the APK. The project carries
SDL 3.4.16's Java layer unchanged (`org.libsdl.app`) and an activity of its own that
only names the library to load. The app's version comes from `Cargo.toml`.

A debug build is signed with Android's debug key. A release build is signed with the
keystore in `ANDROID_KEYSTORE`, its password in `ANDROID_KEYSTORE_PASSWORD` and the key's
alias and password in `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`. The release
workflow (see [packaging.md](packaging.md)) takes them from the repository's secrets,
the keystore as `ANDROID_KEYSTORE_BASE64` (`base64 -i release.jks`). The key must never
change: Android only installs an update signed with the key of the app it replaces.

## Testing

- **On the desktop:** `cargo run -p launcher -- --touch` shows the on-screen pad, and
  with `SDL_MOUSE_TOUCH_EVENTS=1` the mouse plays a finger. The pad's layout and its
  fingers-to-buttons function (`platform::touch`) and the saves' import have unit tests.
- **On an emulator or a phone:** `adb install -r dist/re-zoids-saga-<version>-android.apk`,
  then `adb shell input tap x y` and `adb shell input keyevent KEYCODE_BACK` play,
  `adb exec-out screencap -p > shot.png` captures the screen and `adb logcat` shows the
  log. A phone needs USB debugging on (Settings › About phone › Software information,
  tap the build number seven times, then Developer options › USB debugging).
- **The smoke test:** `tools/package/android-smoke.sh <app.apk> [shot.png]` installs the
  app on the device adb reaches, starts it and checks that it runs, has the focus and
  logged no crash, leaving a screenshot. The release workflow runs it on an `x86_64`
  emulator for every package, without a ROM, so it shows the launcher.
