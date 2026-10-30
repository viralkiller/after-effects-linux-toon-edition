//! Paint authoring is retired; masks, shapes, Puppet and legacy rendering remain.
use effectcraft_engine::Session;
use effectcraft_ui_egui::{EffectcraftApp, dock::PanelKind, menus, state::Tool};
use egui_kittest::Harness;
use serde_json::json;

#[test]
fn toon_no_paint_tools_panels_or_shortcuts() {
    let mut app = EffectcraftApp::new(Session::default());
    let ctx = egui::Context::default();
    for tool in [Tool::Brush, Tool::Clone, Tool::Eraser] {
        assert!(!Tool::ALL.contains(&tool));
        assert!(!Tool::SLOTS.iter().any(|s| s.contains(&tool)));
        assert!(tool.shortcut().is_none());
    }
    for name in ["brush", "clone", "eraser", "rotoBrush"] {
        assert!(menus::invoke(&mut app, &ctx, &format!("tool.{name}"), json!({})).is_err());
    }
    for panel in [PanelKind::Paint, PanelKind::Brushes] {
        assert!(panel.retired());
        assert!(!PanelKind::ALL.contains(&panel));
        assert!(panel.window_shortcut().is_none());
        assert!(menus::invoke(&mut app, &ctx, "window.panel", json!({"panel": panel.title()})).is_err());
    }
    assert!(!effectcraft_ui_egui::dock::WORKSPACES.contains(&"Paint"));
    for id in ["tool.shape", "tool.pen", "tool.type", "tool.puppet"] {
        for _ in 0..7 {
            menus::invoke(&mut app, &ctx, id, json!({})).unwrap();
            assert!(Tool::ALL.contains(&app.ui.tool));
        }
    }
    assert!(app.ui.tool.puppet_kind().is_some());
}

#[test]
fn toon_no_paint_toolbar_still_shows_masks_and_puppet() {
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_eframe(|_| {
        let mut s = Session::default();
        s.execute("comp.new", json!({"width": 640, "height": 360, "duration": 1})).unwrap();
        EffectcraftApp::new(s)
    });
    h.run_steps(3);
    for id in ["tools.Brush", "tools.Clone", "tools.Eraser"] {
        assert!(h.state().auto.find(id).is_none());
    }
    for id in ["tools.Pen", "tools.Rectangle", "tools.Puppet"] {
        assert!(h.state().auto.find(id).is_some(), "{id}");
    }
}

#[test]
fn toon_no_paint_keeps_existing_stroke_data_and_renderer() {
    let mut s = Session::default();
    s.execute("comp.new", json!({"width": 32, "height": 32, "duration": 1})).unwrap();
    let layer = s.execute("layer.newSolid", json!({"width": 32, "height": 32, "color": "#000000"})).unwrap()["layer"].as_u64().unwrap();
    // The compatibility backend remains callable; the desktop authoring UI is removed.
    s.execute("paint.stroke", json!({"layer": layer, "points": [[8, 8], [24, 24]]})).unwrap();
    let cid = s.active_comp_id().unwrap();
    let time = s.time();
    let before = s.render(cid, time, Default::default());
    assert!(before.data.iter().any(|px| px[0] > 0.1), "legacy paint must still render");
    let saved = s.project.to_json();
    s.execute("edit.duplicate", json!({"layers": [layer]})).unwrap();
    s.execute("edit.undo", json!({})).unwrap();
    let app = EffectcraftApp::new(s);
    assert_eq!(app.session.project.to_json(), saved);
    let after = app.session.render(cid, time, Default::default());
    assert_eq!(before.data, after.data);
    assert!(effectcraft_engine::effects::all().iter().all(|e| e.id != "ec.paint.paint"));
    assert!(effectcraft_engine::effects::find("ec.paint.paint").is_some());
}
