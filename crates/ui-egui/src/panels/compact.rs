//! The phone layout: the same panels, widgets and theme as the desktop window, arranged for a
//! narrow portrait screen (Android, or a phone browser running the web build).
//!
//! Below [`COMPACT_WIDTH`] points the window is laid out top to bottom: the top bar, the photo
//! (grid, loupe, compare …), a bottom sheet holding the open right-hand panel (Edit, Crop,
//! Masking …) or the Presets column, the bottom bar, and the tool strip as a row of tabs along
//! the bottom edge. The sheet is resized by dragging its handle; the My Photos sidebar takes the
//! whole width while it is open. Nothing else changes: every panel draws the same contents as on
//! the desktop ([`super::right::contents`], [`super::presets::contents`],
//! [`super::left::contents`]).

use egui::{Rect, Sense, pos2, vec2};
use serde_json::json;

use crate::LightcraftApp;
use crate::icons::Icon;
use crate::state::RightPanel;
use crate::theme::Tokens;
use crate::widgets::{icon_button, register};

/// Windows narrower than this (in points) use the phone layout. The desktop app's minimum window
/// width is 900, so desktop windows never switch.
pub const COMPACT_WIDTH: f32 = 700.0;
/// Height of the tool tab row along the bottom edge.
pub const TABS_H: f32 = 48.0;
/// Height of the sheet's drag handle.
const HANDLE_H: f32 = 18.0;
/// The sheet never gets smaller than this…
const SHEET_MIN: f32 = 140.0;
/// …and always leaves the photo this much room.
const PHOTO_MIN: f32 = 160.0;
/// Share of the window the sheet opens with.
const SHEET_DEFAULT: f32 = 0.42;

/// Whether `ctx`'s window gets the phone layout.
pub fn is_compact(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < COMPACT_WIDTH
}

/// The tools of the desktop tool strip ([`super::strip`]), in its order: (id, icon, panel, tooltip).
pub const TOOLS: [(&str, Icon, RightPanel, &str); 9] = [
    ("edit", Icon::Sliders, RightPanel::Edit, "Edit (E)"),
    ("crop", Icon::Crop, RightPanel::Crop, "Crop & Rotate (C)"),
    ("remove", Icon::Eraser, RightPanel::Remove, "Remove (H)"),
    ("masking", Icon::Mask, RightPanel::Masking, "Masking (M)"),
    ("redeye", Icon::Eye, RightPanel::RedEye, "Red Eye"),
    ("versions", Icon::Versions, RightPanel::Versions, "Versions (Shift+V)"),
    ("activity", Icon::Activity, RightPanel::Activity, "History & Activity (Y)"),
    ("keywords", Icon::Tag, RightPanel::Keywords, "Keywords (K)"),
    ("info", Icon::Info, RightPanel::Info, "Info (I)"),
];

/// Whether `panel`'s tool shows as on (Profiles belongs to Edit).
pub fn tool_on(app: &LightcraftApp, panel: RightPanel) -> bool {
    app.ui.right == panel || (panel == RightPanel::Edit && app.ui.right == RightPanel::Profiles)
}

/// The tool strip as a row of tabs along the bottom edge: Presets, then the tools. Tapping the
/// open tool again closes its sheet.
pub fn tabs(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("compact_tabs")
        .exact_size(TABS_H)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.chrome).stroke(egui::Stroke::new(1.0, t.divider)))
        .show(ui, |ui| {
            let has_photo = app.session.active().is_some();
            let full = ui.max_rect();
            // every tab fits on a phone held upright (≥ 36 points each); narrower windows scroll
            let n = TOOLS.len() + 1;
            let w = (full.width() / n as f32).max(36.0);
            egui::ScrollArea::horizontal()
                .id_salt("compact-tabs-scroll")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let sz = vec2(w, TABS_H - 4.0);
                        if icon_button(ui, "presets", Icon::Presets, sz, app.ui.presets, has_photo, "Presets (Shift+P)").clicked() {
                            let _ = app.run("panel.presets", json!({}));
                        }
                        for (id, icon, panel, tip) in TOOLS {
                            let enabled = has_photo || panel == RightPanel::Activity;
                            if icon_button(ui, id, icon, sz, tool_on(app, panel), enabled, tip).clicked() {
                                let _ = app.run(&format!("panel.{id}"), json!({}));
                            }
                        }
                    });
                });
        });
}

/// The bottom sheet: a drag handle, then `add` (the open panel's contents). Its height is kept in
/// egui's memory, so it survives switching panels; it is clamped to the window every frame.
pub fn sheet(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    let id = egui::Id::new("compact_sheet");
    let avail = ui.available_rect_before_wrap().height();
    let max = (avail - PHOTO_MIN).max(SHEET_MIN);
    let stored: Option<f32> = ui.ctx().data(|d| d.get_temp(id));
    let h = stored.unwrap_or(ui.ctx().content_rect().height() * SHEET_DEFAULT).clamp(SHEET_MIN, max);
    egui::Panel::bottom(id).exact_size(h).resizable(false).frame(egui::Frame::NONE.fill(t.chrome).stroke(egui::Stroke::new(1.0, t.divider))).show(
        ui,
        |ui| {
            let full = ui.max_rect();
            register(ui.ctx(), "panel:sheet", full);
            let hr = Rect::from_min_size(full.min, vec2(full.width(), HANDLE_H));
            let resp = ui.interact(hr, id.with("handle"), Sense::drag());
            register(ui.ctx(), "handle:sheet", hr);
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            let pill = Rect::from_center_size(hr.center(), vec2(36.0, 4.0));
            ui.painter().rect_filled(pill, 2.0, if resp.dragged() { t.text } else { t.button_border });
            if resp.dragged() {
                // dragging up grows the sheet
                let nh = (h - resp.drag_delta().y).clamp(SHEET_MIN, max);
                ui.ctx().data_mut(|d| d.insert_temp(id, nh));
            }
            let body = Rect::from_min_max(pos2(full.left(), hr.bottom()), full.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
            add(&mut child);
        },
    );
}

/// The My Photos sidebar across the whole window (the photo is hidden until it closes).
pub fn sidebar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.chrome)).show(ui, |ui| {
        register(ui.ctx(), "panel:left_panel", ui.max_rect());
        super::left::contents(app, ui);
    });
}
