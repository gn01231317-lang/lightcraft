# LightCraft on Android

`apps/lightcraft-android` runs the same egui UI as the desktop app (`crates/ui-egui`) on Android
phones and tablets, drawn with wgpu on Vulkan. The look, the panels and the widgets are the
desktop ones; on a narrow screen they are arranged by the phone layout (below).

Status: **early**. It builds in CI (`.github/workflows/android.yml`, an arm64 APK as the run's
artifact) and the phone layout is covered by headless tests (`crates/ui-egui/src/tests_compact.rs`),
but it has not been verified on many devices yet. Requirements: Android 10 (API 29) or later, a
GPU with Vulkan 1.0.3.

## The phone layout

Windows narrower than 700 points (`panels::compact::COMPACT_WIDTH`) switch to it: a phone held
upright, a folded foldable, or a phone browser running the web build. Desktop windows (900 points
minimum) never do.

| Desktop | Phone |
|---|---|
| Tool strip at the right edge | A row of tool tabs along the bottom edge (Presets, Edit, Crop, Remove, Masking, Red Eye, Versions, Activity, Keywords, Info); tapping the open tool closes it |
| Right panel (Edit, Crop, Masking …) and the Presets column at the side | A bottom sheet under the photo, resized by dragging its handle |
| My Photos sidebar at the left | The whole window while open; picking a source closes it |
| Menus in the top bar, centred search, five icons | One **Menu**, the search field between the side groups, Export and Activity (plus the unsaved-changes warning when there is one) |
| Bottom bar with every view mode and both zoom menus | Grid, square grid, loupe and compare (Survey and People stay in the View menu), Before/After, Filmstrip and the zoom menu |
| Filmstrip under the loupe | Hidden while a sheet is open |
| Dialogs at their desktop size | Kept on screen; they scroll |

Two-finger pinch zooms the loupe (back to Fit when pinched down to it) and two-finger drags pan it;
a double tap toggles Fit and the click-zoom ratio, as a double click does on the desktop.

## What the Android app adds

- **Storage.** The library (catalog, previews, thumbnails, picked originals) is in the app's
  external files folder, `Android/data/ai.storyteller.lightcraft/files/LightCraft Library`
  (reachable over USB; removed with the app). Settings are in the app's private storage.
- **Adding photos.** Menu ▸ File ▸ Import Photos… opens the system photo picker. Picked files (JPEG, PNG, TIFF, DNG and other raws) are copied into the library's
  `Originals/Picked` folder and added to the library.
- **Exports** go to the shared `Pictures/LightCraft` folder and show up in the gallery
  (`LIGHTCRAFT_EXPORT_DIR`, see `crates/ui-egui/src/control.rs` → `default_export_dir`).
- **Back** closes what is open, one step at a time (a dialog, full screen, the sidebar, the sheet,
  the loupe); with nothing open it sends the app to the background.
- **Text fields** bring up the soft keyboard while they have the focus.
- **Phone defaults.** A new install starts with smaller grid thumbnails (and, like the desktop,
  a new library with the procedural demo photos).

Not there yet: a folder chooser (Local folders, Open Library…, Move Folder To…), Reveal / Open
With, preset and tracklog file pickers, sharing an export to another app, AI masks (SAM 3 is a
desktop feature), and the craft-fonts input (Chinese and Japanese text have no glyphs in CI builds).

## How it fits together

- `apps/lightcraft-android`: the app (session, services, Back, keyboard, picked-photo import,
  `ui.json`). No `unsafe`; what needs Java goes through the `Host` trait.
- `apps/lightcraft-android-main`: `libmain.so`, the library the activity loads. It implements
  `Host` over JNI and holds the crate's only `unsafe` (the `android_main` export and one JNI cast).
- `packaging/android`: the Gradle project: `MainActivity` (a `NativeActivity`: environment,
  photo picker and copying, media scan, links) and the manifest.

Both crates are empty on other targets, so `cargo build`, `cargo test` and `cargo xtask ci` on a
desktop are unaffected.

## Build

One-time setup: an Android SDK with platform 35 and build-tools, the NDK (r26 or later: only its
linker is used; everything compiled is Rust), Java 17, Gradle 8.9 or later, and

```sh
rustup target add aarch64-linux-android          # phones
rustup target add x86_64-linux-android           # the emulator (optional)
```

Then:

```sh
export ANDROID_HOME=~/Android/Sdk ANDROID_NDK_HOME=~/Android/Sdk/ndk/27.2.12479018
packaging/android/build.sh                       # → dist/release/lightcraft-android-<version>.apk
packaging/android/build.sh --fast --abi all      # quicker build, phones + emulator
adb install -r dist/release/lightcraft-android-*.apk
adb logcat -s lightcraft                         # the app's log
```

Quick check without the SDK: `cargo clippy -p lightcraft-android -p lightcraft-android-main
--target aarch64-linux-android -- -D warnings` (what CI runs first).

The APK is signed with Gradle's debug key unless `LIGHTCRAFT_ANDROID_KEYSTORE`,
`LIGHTCRAFT_ANDROID_KEYSTORE_PASSWORD`, `LIGHTCRAFT_ANDROID_KEY_ALIAS` and
`LIGHTCRAFT_ANDROID_KEY_PASSWORD` name a release key. CI's debug key changes from run to run, so
uninstall an APK from an earlier run before installing a newer one.

## Looking at the phone layout without a phone

```sh
lightcraft-cli snapshot --demo -o phone.png --size 412x915 --scale 2
```

draws the app headlessly at phone size (add `--script` with `ui.set` requests to open panels; see
`docs/control-protocol.md` → Headless rendering).
