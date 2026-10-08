//! Headless tests of the phone layout (`panels::compact`): below `COMPACT_WIDTH` the open panel is
//! a bottom sheet above the bottom bar and the tool tabs, nothing runs off the window, the sheet
//! resizes by its handle, the sidebar takes the window and closes once a source is picked, and
//! desktop-sized windows keep the side panels.

use std::time::Duration;

use serde_json::json;

use crate::headless::Headless;
use crate::{LightcraftApp, Services};

const T: Duration = Duration::from_secs(20);
const SETTLE: Duration = Duration::from_secs(120);
/// A phone held upright (points).
const PHONE: [f32; 2] = [412.0, 915.0];

fn demo(size: [f32; 2], ui: serde_json::Value) -> Headless {
    let services = Services { png: None, ..Default::default() };
    let app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    let mut h = Headless::new(app, size, 1.0);
    let r = h.request("ui.set", ui, T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    h
}

fn find(h: &Headless, id: &str) -> Option<egui::Rect> {
    h.app.widgets.iter().find(|(w, _)| w == id).map(|(_, r)| *r)
}

fn widget(h: &Headless, id: &str) -> egui::Rect {
    find(h, id).unwrap_or_else(|| panic!("no widget {id}"))
}

#[test]
fn phone_layout_puts_the_panel_in_a_bottom_sheet_above_the_tool_tabs() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    let sheet = widget(&h, "panel:sheet");
    let edit = widget(&h, "icon:edit");
    let bottom = widget(&h, "icon:photoGrid");
    // full width, under the photo, above the bottom bar, which is above the tabs
    assert!(sheet.left() <= 1.0 && sheet.right() >= PHONE[0] - 1.0, "{sheet:?}");
    assert!(sheet.bottom() <= bottom.top() && bottom.bottom() <= edit.top(), "{sheet:?} {bottom:?} {edit:?}");
    let canvas = h.app.canvas_rect.unwrap();
    assert!(canvas.bottom() <= sheet.top() + 1.0 && canvas.height() >= 150.0, "{canvas:?} {sheet:?}");
    // no desktop side panel or tool strip
    assert!(find(&h, "panel:right_panel").is_none());
    // the top bar's icons all fit on the screen
    for id in ["icon:sidebar", "icon:back", "field:search", "icon:filter", "icon:share", "icon:bell"] {
        let r = widget(&h, id);
        assert!(r.left() >= 0.0 && r.right() <= PHONE[0], "{id} {r:?}");
    }
    // every tool tab is on screen
    for id in ["icon:presets", "icon:edit", "icon:crop", "icon:masking", "icon:info"] {
        let r = widget(&h, id);
        assert!(r.left() >= 0.0 && r.right() <= PHONE[0] + 0.5, "{id} {r:?}");
    }
    // dragging the handle up grows the sheet
    let handle = widget(&h, "handle:sheet");
    let (x, y) = (handle.center().x, handle.center().y);
    let r = h.request("ui.drag", json!({"x": x, "y": y, "toX": x, "toY": y - 120.0, "steps": 10}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    let grown = widget(&h, "panel:sheet");
    assert!((grown.height() - sheet.height() - 120.0).abs() <= 3.0, "{sheet:?} → {grown:?}");
    // tapping the open tool closes the sheet: the photo gets the room
    let r = h.request("ui.clickWidget", json!({"id": "icon:edit"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(find(&h, "panel:sheet").is_none());
}

#[test]
fn phone_sidebar_takes_the_window_and_closes_on_a_pick() {
    let mut h = demo(PHONE, json!({"view": "photoGrid", "right": "none", "leftPanel": true}));
    let side = widget(&h, "panel:left_panel");
    assert!(side.width() >= PHONE[0] - 1.0, "{side:?}");
    let r = h.request("ui.clickWidget", json!({"id": "source:picks"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(!h.app.ui.left_panel);
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::Picks);
}

#[test]
fn desktop_windows_keep_the_side_panels() {
    let h = demo([1400.0, 900.0], json!({"view": "detail", "right": "edit"}));
    assert!(find(&h, "panel:right_panel").is_some());
    assert!(find(&h, "panel:sheet").is_none());
}
