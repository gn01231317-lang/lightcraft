use android_activity::AndroidApp;
use jni::objects::{JObject, JString};
use jni::refs::Global;
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};
use lightcraft_android::Host;

/// Called by `android-activity` on its own thread once the activity is created.
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let host = std::sync::Arc::new(Jni(app.clone()));
    lightcraft_android::run(app, host);
}

/// [`Host`] over JNI: calls `MainActivity`'s methods.
struct Jni(AndroidApp);

impl Host for Jni {
    fn pick_photos(&self) -> Result<(), String> {
        pick_photos(&self.0)
    }
    fn take_picked(&self) -> Result<Option<Vec<String>>, String> {
        take_picked(&self.0)
    }
    fn media_scan(&self, path: &str) -> Result<(), String> {
        call_with_string(&self.0, jni_str!("mediaScan"), path)
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        call_with_string(&self.0, jni_str!("openUrl"), url)
    }
    fn move_to_back(&self) -> Result<(), String> {
        call(&self.0, |env, a| env.call_method(a, jni_str!("moveTaskToBack"), jni_sig!("(Z)Z"), &[JValue::Bool(true)]).map(|_| ()))
    }
}

/// Run `f` with the JNI environment and the activity.
fn call<T>(app: &AndroidApp, f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    let vm = JavaVM::singleton().map_err(|e| format!("no Java VM: {e}"))?;
    vm.attach_current_thread(|env| -> jni::errors::Result<T> {
        let raw = app.activity_as_ptr() as jni::sys::jobject;
        // SAFETY: `activity_as_ptr` is a JNI global reference to the activity, valid while `app`
        // lives (it does: we hold it), and not owned by us: `as_cast_raw` borrows it without
        // taking ownership, so it is never deleted here.
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw)? };
        f(env, &activity)
    })
    .map_err(|e| e.to_string())
}

/// Open the system photo picker (several photos and raws at once).
fn pick_photos(app: &AndroidApp) -> Result<(), String> {
    call(app, |env, a| env.call_method(a, jni_str!("pickPhotos"), jni_sig!("()V"), &[]).map(|_| ()))
}

/// The photos picked since [`pick_photos`], copied into the app's storage: `None` while the picker
/// is open or copies are still being made, else their paths (empty when the picker was cancelled).
fn take_picked(app: &AndroidApp) -> Result<Option<Vec<String>>, String> {
    call(app, |env, a| {
        let obj = env.call_method(a, jni_str!("takePicked"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
        let s = env.cast_local::<JString>(obj)?;
        if s.is_null() {
            return Ok(None);
        }
        let text = s.to_string();
        Ok(Some(text.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()))
    })
}

/// Call `name(String)` on the activity.
fn call_with_string(app: &AndroidApp, name: &'static jni::strings::JNIStr, arg: &str) -> Result<(), String> {
    call(app, |env, a| {
        let s = env.new_string(arg)?;
        env.call_method(a, name, jni_sig!("(Ljava/lang/String;)V"), &[JValue::Object(&s)]).map(|_| ())
    })
}
