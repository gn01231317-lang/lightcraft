//! The UI state and app settings (`<config>/ui.json`), as on the desktop: read at launch, written
//! atomically a few seconds after anything changes. Android may end the process at any time
//! once the app is in the background (no exit hook runs), so nothing waits for the end.

use lightcraft_ui_egui::{LightcraftApp, UiState};

fn path() -> Option<std::path::PathBuf> {
    lightcraft_engine::camera_profiles::config_dir().map(|d| d.join("ui.json"))
}

/// The saved state, if any; a damaged file is set aside (`ui.json.corrupt-<unix time>`) and the
/// returned message says so.
pub fn load() -> (Option<UiState>, Option<String>) {
    let Some(path) = path() else { return (None, None) };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (None, None),
        Err(e) => return (None, Some(format!("The app settings couldn't be read ({e}); defaults are used."))),
    };
    match serde_json::from_slice::<UiState>(&bytes) {
        Ok(ui) => (Some(ui.sanitized()), None),
        Err(e) => {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let _ = std::fs::rename(&path, path.with_file_name(format!("ui.json.corrupt-{secs}")));
            (None, Some(format!("The app settings were damaged ({e}) and were reset.")))
        }
    }
}

/// Writes `ui.json` (and the library's view state) when they changed, at most every few seconds.
#[derive(Default)]
pub struct Writer {
    written: Vec<u8>,
    checked: f64,
    failing: bool,
}

impl Writer {
    pub fn new(app: &LightcraftApp) -> Self {
        Writer { written: serde_json::to_vec_pretty(&app.ui).unwrap_or_default(), ..Default::default() }
    }

    pub fn tick(&mut self, app: &mut LightcraftApp, now: f64) {
        if now - self.checked < 3.0 {
            return;
        }
        self.checked = now;
        let Ok(bytes) = serde_json::to_vec_pretty(&app.ui) else { return };
        if bytes == self.written {
            return;
        }
        // the grid's scroll position, sort and filters live in the library's view.json
        app.session.save_view();
        match path().ok_or_else(|| "no settings folder".to_string()).and_then(|p| write_atomic(&p, &bytes).map_err(|e| e.to_string())) {
            Ok(()) => {
                self.written = bytes;
                self.failing = false;
            }
            Err(e) => {
                log::warn!("saving the app settings failed: {e}");
                if !self.failing {
                    app.notices.push(format!("Saving the app settings failed: {e}. LightCraft keeps trying."));
                }
                self.failing = true;
            }
        }
    }
}

/// Write `bytes` to `path` via a synced temp file renamed over it.
fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let written = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path)
}
