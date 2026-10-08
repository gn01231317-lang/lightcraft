//! The Android app: the library session, the platform services and the eframe app around
//! [`LightcraftApp`].

use android_activity::AndroidApp;
use lightcraft_engine::Session;
use lightcraft_ui_egui::panels::library_problem::LibraryProblem;
use lightcraft_ui_egui::state::{RightPanel, ViewMode};
use lightcraft_ui_egui::{LightcraftApp, Services};
use serde_json::json;
use std::sync::Arc;

use crate::Host;

/// Grid thumbnail size on a phone's first launch (the desktop default, 220, shows one photo per row).
const PHONE_THUMB_SIZE: f32 = 120.0;
/// How often a pending photo pick is checked for (seconds).
const PICK_POLL: f64 = 0.3;

/// Run the app in `android`'s activity (called from `android_main`).
pub fn run(android: AndroidApp, host: Arc<dyn Host>) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("lightcraft"));
    lightcraft_engine::guard::install_hook(std::env::temp_dir().join("lightcraft-panics.log"));
    let (prefs, prefs_warning) = crate::prefs::load();
    // GPU compute, unless the preference turned it off (the window itself always draws with wgpu)
    lightcraft_engine::gpu::set_enabled(prefs.as_ref().is_none_or(|u| u.settings.gpu));
    let options = eframe::NativeOptions { android_app: Some(android.clone()), ..Default::default() };
    let result = eframe::run_native(
        "LightCraft",
        options,
        Box::new(move |_cc| {
            let (session, problem) = open_session(prefs.as_ref().map(|u| u.settings.library_path.clone()));
            let picking = std::rc::Rc::new(std::cell::Cell::new(false));
            let mut app = LightcraftApp::new(session, services(&host, picking.clone()));
            match prefs {
                Some(ui) => app.ui = ui,
                None => app.ui.thumb_size = PHONE_THUMB_SIZE,
            }
            lightcraft_ui_egui::i18n::set_language(app.ui.language);
            app.notices.extend(prefs_warning);
            app.library_problem = problem;
            let writer = crate::prefs::Writer::new(&app);
            Ok(Box::new(App { app, android, host, writer, picking, picked_at: 0.0, keyboard: false }))
        }),
    );
    if let Err(e) = result {
        log::error!("LightCraft couldn't start: {e}");
    }
}

/// The library at `LIGHTCRAFT_LIBRARY` (the activity sets it to the app's external files folder),
/// or the one last opened from Settings. A library that can't be opened is never replaced: the
/// window says why (`library_problem`).
fn open_session(last: Option<String>) -> (Session, Option<LibraryProblem>) {
    let unopened = || Session::new().with_fs().with_system_clock();
    let dir = last.filter(|p| !p.is_empty()).map(std::path::PathBuf::from).or_else(lightcraft_engine::library::default_dir);
    let Some(dir) = dir else {
        return (unopened(), Some(LibraryProblem::new("", "There is no folder to keep the library in.")));
    };
    let mut s = unopened();
    match s.open_library(&dir, true) {
        Ok(r) => {
            let replayed = r.replayed;
            log::info!("library {}: {} photos, {replayed} log records replayed", dir.display(), s.catalog.len());
            (s, None)
        }
        Err(e) => {
            log::error!("can't open library {}: {e}", dir.display());
            (unopened(), Some(LibraryProblem::new(dir.to_string_lossy(), e.to_string())))
        }
    }
}

/// `picking` is set while the photo picker is open (see [`App::poll_picked`]).
fn services(host: &Arc<dyn Host>, picking: std::rc::Rc<std::cell::Cell<bool>>) -> Services {
    let (pick, scan, url) = (host.clone(), host.clone(), host.clone());
    // exports: written like on the desktop, then announced to the gallery
    let write = std::sync::Arc::new(move |path: &str, bytes: &[u8]| -> Result<(), String> {
        lightcraft_engine::export::write_file(path, bytes)?;
        if let Err(e) = scan.media_scan(path) {
            log::warn!("media scan of {path}: {e}");
        }
        Ok(())
    });
    let shared = write.clone();
    Services {
        pick_files: Some(Box::new(move || {
            match pick.pick_photos() {
                Ok(()) => picking.set(true),
                Err(e) => log::warn!("photo picker: {e}"),
            }
            Vec::new() // the photos arrive later (App::poll_picked)
        })),
        write: Some(Box::new(move |path: &str, bytes: &[u8]| write(path, bytes))),
        write_shared: Some(shared),
        png: Some(Box::new(|img: &lightcraft_raster::Rgba8| {
            lightcraft_codecs::encode_png(&lightcraft_codecs::EncodeImage::rgba8(img), &lightcraft_codecs::EncodeMeta::default()).unwrap_or_default()
        })),
        open_url: Some(Box::new(move |u: &str| {
            if !u.starts_with("https://") {
                return Err("only https links are opened".into());
            }
            url.open_url(u)
        })),
        // no folder chooser, file manager or other apps to hand files to (yet): the menus and
        // dialogs that need them are disabled
        ..Default::default()
    }
}

struct App {
    app: LightcraftApp,
    android: AndroidApp,
    host: Arc<dyn Host>,
    writer: crate::prefs::Writer,
    /// The photo picker is open (or its photos are being copied).
    picking: std::rc::Rc<std::cell::Cell<bool>>,
    picked_at: f64,
    /// The soft keyboard is shown.
    keyboard: bool,
}

impl App {
    /// Import the photos picked in the system picker once they are copied.
    fn poll_picked(&mut self, ctx: &egui::Context) {
        if !self.picking.get() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(PICK_POLL));
        let now = ctx.input(|i| i.time);
        if now - self.picked_at < PICK_POLL {
            return;
        }
        self.picked_at = now;
        match self.host.take_picked() {
            Ok(None) => {} // still open, or still copying
            Ok(Some(paths)) => {
                self.picking.set(false);
                if paths.is_empty() {
                    return;
                }
                let n = paths.len();
                match self.app.run("library.import", json!({"paths": paths})) {
                    Ok(_) => {
                        self.app.toast(ctx, format!("Adding {n} photo{}", if n == 1 { "" } else { "s" }));
                        self.app.ui.view = ViewMode::PhotoGrid;
                    }
                    Err(e) => self.app.toast_error(ctx, format!("Import failed: {e}")),
                }
            }
            Err(e) => {
                self.picking.set(false);
                log::warn!("picked photos: {e}");
            }
        }
    }

    /// Show the soft keyboard while a text field has the focus.
    fn soft_keyboard(&mut self, ctx: &egui::Context) {
        let wants = ctx.egui_wants_keyboard_input();
        if wants != self.keyboard {
            if wants {
                self.android.show_soft_input(true);
            } else {
                self.android.hide_soft_input(true);
            }
            self.keyboard = wants;
        }
    }

    /// The Back button closes what is open, one step at a time: a dialog, the sidebar, the panel
    /// sheet, the loupe; with nothing open it sends the app to the background.
    fn back(&mut self) {
        let ui = &mut self.app.ui;
        if ui.dialog.is_some() {
            ui.dialog = None;
        } else if ui.fullscreen {
            ui.fullscreen = false;
        } else if ui.left_panel {
            ui.left_panel = false;
        } else if ui.presets {
            ui.presets = false;
        } else if ui.right != RightPanel::None {
            ui.right = RightPanel::None;
        } else if ui.view != ViewMode::PhotoGrid && ui.view != ViewMode::SquareGrid {
            ui.view = ViewMode::PhotoGrid;
        } else if let Err(e) = self.host.move_to_back() {
            log::warn!("back: {e}");
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.logic(ctx);
        self.poll_picked(ctx);
        self.soft_keyboard(ctx);
        let now = ctx.input(|i| i.time);
        self.writer.tick(&mut self.app, now);
    }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        let back = |e: &egui::Event| matches!(e, egui::Event::Key { key: egui::Key::BrowserBack, .. });
        if raw.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::BrowserBack, pressed: true, .. })) {
            self.back();
        }
        raw.events.retain(|e| !back(e));
        self.app.raw_input_hook(raw);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}
