//! LightCraft on Android: the same egui UI as the desktop app (`crates/ui-egui`), laid out for a
//! phone by its compact layout (`lightcraft_ui_egui::panels::compact`), running in a
//! `NativeActivity` subclass (`packaging/android/app/src/main/java/.../MainActivity.java`).
//!
//! The activity prepares the environment before the app starts (`HOME` and `TMPDIR` in the
//! app's private storage, `LIGHTCRAFT_LIBRARY` in its external files folder,
//! `LIGHTCRAFT_EXPORT_DIR` = the shared `Pictures/LightCraft`) and does what needs the Java side
//! ([`Host`]): the system photo picker (picked photos are copied into the library folder, then
//! imported), media scanning of exports, opening links. `apps/lightcraft-android-main` is the
//! library the activity loads: it implements [`Host`] over JNI and calls [`run`].
//!
//! Build: `packaging/android/build.sh` (see `docs/android.md`).
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "android")]
mod app;
#[cfg(target_os = "android")]
mod prefs;

#[cfg(target_os = "android")]
pub use app::run;

/// What the Android activity does for the app (`MainActivity.java`).
pub trait Host: Send + Sync {
    /// Open the system photo picker (several photos and raws at once).
    fn pick_photos(&self) -> Result<(), String>;
    /// The photos picked since [`Host::pick_photos`], copied into the library folder: `None`
    /// while the picker is open or copies are still being made, else their paths (empty when the
    /// picker was cancelled).
    fn take_picked(&self) -> Result<Option<Vec<String>>, String>;
    /// Tell the media scanner about a written file, so exports show up in the gallery.
    fn media_scan(&self, path: &str) -> Result<(), String>;
    /// Open an https link in the browser.
    fn open_url(&self, url: &str) -> Result<(), String>;
    /// Send the app to the background (Back with nothing left to close).
    fn move_to_back(&self) -> Result<(), String>;
}
