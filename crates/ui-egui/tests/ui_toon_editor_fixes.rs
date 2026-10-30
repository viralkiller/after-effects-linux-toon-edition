//! Regressions for split order, persistent resolution/editor context, and live trimming.
use effectcraft_engine::{Session, project::{ItemId, Project}, time::Tick};
use effectcraft_ui_egui::{EffectcraftApp, dock::{Floating, PanelKind}, menus};
use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use serde_json::{Value, json};

fn comp(s: &mut Session, name: &str) -> ItemId {
    s.execute("comp.new", json!({"name": name, "width": 64, "height": 48, "frameRate": 24, "duration": 10})).unwrap();
    s.active_comp_id().unwrap()
}
fn harness(s: Session) -> Harness<'static, EffectcraftApp> {
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_eframe(move |_| EffectcraftApp::new(s));
    h.run_steps(3);
    h
}
fn invoke(h: &mut Harness<'_, EffectcraftApp>, id: &str, params: Value) -> Value {
    let ctx = h.ctx.clone();
    menus::invoke(h.state_mut(), &ctx, id, params).unwrap()
}
fn center(h: &Harness<'_, EffectcraftApp>, id: &str) -> egui::Pos2 {
    let e = h.state().auto.find(id).unwrap_or_else(|| panic!("missing {id}"));
    pos2(e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0)
}
fn click(h: &mut Harness<'_, EffectcraftApp>, id: &str) {
    let pos = center(h, id);
    h.event(Event::PointerMoved(pos));
    h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("toon-editor-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

#[test]
fn toon_editor_split_shortcut_ignores_old_below_preference() {
    let mut s = Session::default();
    comp(&mut s, "Split");
    let original = s.execute("layer.newSolid", json!({"name": "Art"})).unwrap()["layer"].as_u64().unwrap();
    s.prefs.general.create_split_layers_above = false;
    s.set_time(Tick::from_seconds_f64(4.0));
    let mut h = harness(s);
    h.state_mut().ui.focused = PanelKind::Timeline;
    let modifiers = Modifiers { ctrl: true, command: true, shift: true, ..Modifiers::NONE };
    h.event(Event::ModifiersChanged(modifiers));
    h.event(Event::Key { key: egui::Key::D, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.event(Event::Key { key: egui::Key::D, physical_key: None, pressed: false, repeat: false, modifiers });
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
    let c = h.state().session.active_comp().unwrap();
    assert_eq!(c.layers.len(), 2);
    assert_eq!(c.layers[1].id.0, original);
    assert_eq!(c.layers[0].in_point.seconds(), 4.0);
    assert_eq!(c.layers[1].out_point.seconds(), 4.0);
}

#[test]
fn toon_editor_full_resolution_survives_ui_edits_undo_and_save() {
    use effectcraft_engine::preview::PreviewResolution;
    use effectcraft_ui_egui::state::Resolution;
    let mut s = Session::default(); comp(&mut s, "Full");
    let mut h = harness(s);
    invoke(&mut h, "window.panel", json!({"panel": "preview"}));
    h.state_mut().toggle_maximize(PanelKind::Preview);
    h.run_steps(3);
    click(&mut h, "preview.resolution");
    click(&mut h, "preview.resolution.1");
    assert_eq!(h.state().session.project.preview.spacebar.resolution, PreviewResolution::Full);
    invoke(&mut h, "edit.undo", json!({}));
    assert_eq!(h.state().session.project.preview.spacebar.resolution, PreviewResolution::Half);
    invoke(&mut h, "edit.redo", json!({}));
    invoke(&mut h, "view.res.full", json!({}));
    h.run_steps(3);
    assert_eq!(h.state().ui.viewer.res, Resolution::Full);
    invoke(&mut h, "comp.workArea", json!({"start": 1, "end": 8}));
    h.run_steps(3);
    let p = Project::from_json(&h.state().session.project_for_save().to_file_json().unwrap()).unwrap();
    assert_eq!(p.preview.spacebar.resolution, PreviewResolution::Full);
    assert_eq!(p.viewport_resolution, Resolution::Full);
    assert!(h.state().session.is_dirty());
}

#[test]
fn toon_editor_range_handles_show_horizontal_cursor_and_drag() {
    let mut s = Session::default(); comp(&mut s, "Range");
    s.execute("comp.workArea", json!({"start": 2, "end": 8})).unwrap();
    let mut h = harness(s);
    for id in ["timeline.workArea.begin", "timeline.workArea.end", "timeline.workArea.bar", "timeline.navigator"] {
        let pos = center(&h, id);
        h.event(Event::PointerMoved(pos)); h.run_steps(2);
        assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal, "{id}");
    }
    let pos = center(&h, "timeline.workArea.begin");
    h.event(Event::PointerMoved(pos));
    h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    let to = pos + vec2(30.0, 0.0);
    h.event(Event::PointerMoved(to)); h.run_steps(2);
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal);
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert!(h.state().session.active_comp().unwrap().work_area.0.seconds() > 2.0);
}

#[test]
fn toon_editor_save_reopens_tabs_playheads_layout_and_each_comp_position() {
    let temp = Temp::new(); let path = temp.0.join("editor.ecproj");
    let mut s = Session::default();
    let a = comp(&mut s, "A"); let b = comp(&mut s, "B");
    s.prefs.general.sync_time_related_items = false;
    let mut h = harness(s);
    h.state_mut().session.open_comp(a); h.run_steps(3);
    h.state_mut().session.set_time(Tick::from_seconds_f64(3.0));
    h.state_mut().ui.timeline.start = 1.0;
    h.state_mut().ui.timeline.pps = Some(220.0);
    h.state_mut().ui.viewer.zoom = Some(2.0);
    h.state_mut().ui.viewer.pan = [23.0, -15.0];
    h.state_mut().session.open_comp(b); h.run_steps(3);
    h.state_mut().session.set_time(Tick::from_seconds_f64(6.0));
    h.state_mut().ui.timeline.start = 2.0;
    h.state_mut().ui.timeline.pps = Some(300.0);
    h.state_mut().ui.viewer.zoom = Some(1.5);
    h.state_mut().ui.viewer.pan = [-7.0, 11.0];
    h.state_mut().ui.dock.close(PanelKind::Preview);
    h.state_mut().ui.floating.push(Floating { panels: vec![PanelKind::Preview], active: 0, rect: [50.0, 70.0, 280.0, 400.0] });
    h.run_steps(3);
    let dock = h.state().ui.dock.clone(); let floating = h.state().ui.floating.clone();
    invoke(&mut h, "file.saveAs", json!({"path": path.to_string_lossy()}));
    assert!(!h.state().session.is_dirty());
    let mut s = Session::default();
    s.execute("file.open", json!({"path": path.to_string_lossy()})).unwrap();
    let mut reopened = harness(s);
    assert_eq!(reopened.state().session.state.open_comps, vec![a, b]);
    assert_eq!(reopened.state().session.active_comp_id(), Some(b));
    assert_eq!(reopened.state().session.time().seconds(), 6.0);
    assert_eq!(reopened.state().ui.dock, dock);
    assert_eq!(reopened.state().ui.floating, floating);
    assert_eq!(reopened.state().ui.timeline.start, 2.0);
    assert_eq!(reopened.state().ui.timeline.pps, Some(300.0));
    assert_eq!(reopened.state().ui.viewer.pan, [-7.0, 11.0]);
    reopened.state_mut().session.open_comp(a); reopened.run_steps(3);
    assert_eq!(reopened.state().session.time().seconds(), 3.0);
    assert_eq!(reopened.state().ui.timeline.start, 1.0);
    assert_eq!(reopened.state().ui.timeline.pps, Some(220.0));
    assert_eq!(reopened.state().ui.viewer.zoom, Some(2.0));
    assert_eq!(reopened.state().ui.viewer.pan, [23.0, -15.0]);
    assert!(!reopened.state().session.is_dirty());
}

#[test]
fn toon_editor_trim_updates_without_reopening_and_preserves_artwork() {
    let mut s = Session::default(); let cid = comp(&mut s, "Trim");
    let lid = s.execute("layer.newSolid", json!({"color": "#ff0000"})).unwrap()["layer"].as_u64().unwrap();
    let mut h = harness(s);
    let props = h.state().session.active_comp().unwrap().layers[0].props.clone();
    h.state_mut().session.execute("comp.workArea", json!({"start": 2, "end": 6})).unwrap();
    h.state_mut().session.set_time(Tick::from_seconds_f64(9.0));
    h.state_mut().ui.timeline.start = 8.0;
    h.state_mut().ui.timeline.pps = Some(300.0);
    invoke(&mut h, "comp.trimToWorkArea", json!({}));
    h.run_steps(3);
    let app = h.state();
    assert_eq!(app.session.active_comp_id(), Some(cid));
    assert_eq!(app.session.active_comp().unwrap().duration.seconds(), 4.0);
    assert!(app.session.time().seconds() < 4.0);
    assert_eq!(app.ui.timeline.start, 0.0);
    assert_eq!(app.ui.timeline.pps, None);
    assert!(app.auto.find(&format!("timeline.layer.{lid}.bar")).is_some());
    assert_eq!(app.session.active_comp().unwrap().layers[0].props, props);
    invoke(&mut h, "edit.undo", json!({})); h.run_steps(2);
    assert_eq!(h.state().session.active_comp().unwrap().duration.seconds(), 10.0);
    invoke(&mut h, "edit.redo", json!({})); h.run_steps(2);
    assert_eq!(h.state().session.active_comp().unwrap().duration.seconds(), 4.0);
}

#[test]
fn toon_editor_invalid_layout_and_stale_comp_ids_do_not_lose_artwork() {
    let mut s = Session::default(); let id = comp(&mut s, "Valid");
    let mut p = s.project_for_save();
    let saved = p.editing_session.as_mut().unwrap();
    saved.open_comps = vec![ItemId(u64::MAX), id, id];
    saved.active_comp = Some(ItemId(u64::MAX));
    saved.times.insert(id, Tick::from_seconds_f64(1000.0));
    saved.frontend = json!({"version": 999});
    s.replace_project(p, None);
    let mut h = harness(s);
    assert_eq!(h.state().session.state.open_comps, vec![id]);
    assert_eq!(h.state().session.active_comp_id(), Some(id));
    assert!(h.state().session.time().seconds() < 10.0);
    assert_eq!(h.state().session.project.comps().count(), 1);
    assert!(h.state().ui.status.contains("layout ignored"));
    h.state_mut().session.state.open_comps.clear();
    h.state_mut().session.state.active_comp = None;
    let p = h.state().session.project_for_save();
    h.state_mut().session.replace_project(p, None); h.run_steps(2);
    assert!(h.state().session.state.open_comps.is_empty());
    assert_eq!(h.state().session.active_comp_id(), None);
}
