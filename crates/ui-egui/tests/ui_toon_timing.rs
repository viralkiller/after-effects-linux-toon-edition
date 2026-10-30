//! Group edge drags must trim every selected layer, preserving relative timing.
use effectcraft_engine::Session;
use effectcraft_ui_egui::EffectcraftApp;
use egui::{Event, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;

fn rect(h: &Harness<'_, EffectcraftApp>, id: &str) -> Rect {
    let e = h.state().auto.find(id).unwrap_or_else(|| panic!("missing {id}"));
    Rect::from_min_size(pos2(e.rect[0], e.rect[1]), vec2(e.rect[2], e.rect[3]))
}

fn drag(h: &mut Harness<'_, EffectcraftApp>, from: Pos2, to: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(from));
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Default::default() });
    h.step();
    for i in 1..=8 {
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * (i as f32 / 8.0)));
        h.step();
    }
    h.input_mut().events.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Default::default() });
    h.run_steps(2);
}

#[test]
fn toon_timeline_selected_edges_trim_together() {
    for edge in ["in", "out"] {
        let mut s = Session::default();
        s.execute("comp.new", json!({"name": "Main", "width": 640, "height": 360, "frameRate": 30, "duration": 10})).unwrap();
        let a = s.execute("layer.newSolid", json!({"name": "A"})).unwrap()["layer"].as_u64().unwrap();
        let b = s.execute("layer.newSolid", json!({"name": "B"})).unwrap()["layer"].as_u64().unwrap();
        s.execute("layer.timing", json!({"layers": [a], "in": 1, "out": 5})).unwrap();
        s.execute("layer.timing", json!({"layers": [b], "in": 2, "out": 6})).unwrap();
        s.execute("layer.select", json!({"layers": [a, b]})).unwrap();
        let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_eframe(move |_| EffectcraftApp::new(s));
        h.run_steps(3);
        let from = rect(&h, &format!("timeline.layer.{a}.bar.{edge}")).center();
        drag(&mut h, from, from + vec2(25.0, 0.0));
        let c = h.state().session.active_comp().unwrap();
        let la = c.layer(effectcraft_engine::project::LayerId(a)).unwrap();
        let lb = c.layer(effectcraft_engine::project::LayerId(b)).unwrap();
        let (ea, eb, original) = if edge == "in" { (la.in_point, lb.in_point, 1.0) } else { (la.out_point, lb.out_point, 5.0) };
        assert!(ea.seconds() > original, "the grabbed edge must move");
        assert_eq!((eb - ea).seconds(), 1.0, "relative timing must survive");
        assert_eq!(la.start_time, effectcraft_engine::time::Tick(0));
    }
}
