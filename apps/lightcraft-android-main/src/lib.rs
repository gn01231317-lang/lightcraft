//! `libmain.so`, the library LightCraft's Android activity loads (`packaging/android`): the
//! `android_main` entry point and the JNI calls into `MainActivity`, behind the safe
//! [`lightcraft_android::Host`] trait. The app itself is `lightcraft-android` (no `unsafe`).
//!
//! The only `unsafe` here (allowed in this crate alone, see CLAUDE.md → Never crash): the
//! `android_main` export `android-activity` calls, and turning the activity reference it hands
//! out into a JNI object (`jni_glue::call`). Every Java call goes through that function, which attaches the
//! calling thread (the UI thread, or an export worker) and turns Java exceptions into errors.
#![cfg_attr(target_os = "android", allow(unsafe_code))]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "android")]
mod jni_glue;
