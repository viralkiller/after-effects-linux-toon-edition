use effectcraft_engine::{Session, project::ItemId};
use effectcraft_ui_egui::EffectcraftApp;
use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;

#[test]
fn toon_names_creation_and_nesting_share_numbering_and_preserve_dots() {
    let mut s = Session::default();
    for (base, expected) in [("master", "master"), ("master", "master2"), ("master2", "master3"), ("master.v2", "master.v2")] {
        s.execute("comp.new", json!({"name": base, "width": 32, "height": 32})).unwrap();
        assert_eq!(s.project.item(s.active_comp_id().unwrap()).unwrap().name, expected);
    }
    let source = s.active_comp_id().unwrap();
    for expected in ["master.v3", "master.v4"] {
        s.state.project_selection = vec![source];
        let r = s.execute("file.newCompFromSelection", json!({})).unwrap();
        let id = ItemId(r["comps"][0].as_u64().unwrap());
        assert_eq!(s.project.item(id).unwrap().name, expected);
        assert_eq!(s.project.comp(id).unwrap().layers[0].source.item(), Some(source));
    }
    assert_eq!(s.project.item(source).unwrap().name, "master.v2");
}

#[test]
fn toon_layer_drag_scrolls_both_ways_and_drops_after_origin_leaves_view() {
    let mut s = Session::default();
    s.execute("comp.new", json!({"width": 32, "height": 32, "duration": 1})).unwrap();
    for i in 0..60 {
        s.execute("layer.newNull", json!({"name": format!("Layer {i}")})).unwrap();
    }
    let moving: Vec<u64> = s.active_comp().unwrap().layers.iter().take(2).map(|l| l.id.0).collect();
    s.execute("layer.select", json!({"layers": moving})).unwrap();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_eframe(move |_| EffectcraftApp::new(s));
    h.run_steps(3);
    let r = h.state().auto.find("timeline.layerList").unwrap().rect;
    let twirl = h.state().auto.find(&format!("timeline.layer.{}.twirl", moving[0])).unwrap().rect;
    let origin = pos2(twirl[0] + twirl[2] + 40.0, twirl[1] + twirl[3] / 2.0);
    let bottom = pos2(origin.x, r[1] + r[3] - 2.0);
    let top = pos2(bottom.x, r[1] + 2.0);
    h.event(Event::PointerMoved(origin));
    h.step();
    h.event(Event::PointerButton { pos: origin, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    // Start the actual selected-row drag.
    h.event(Event::PointerMoved(origin + vec2(12.0, 0.0)));
    h.run_steps(2);
    assert_eq!(h.ctx.data(|d| d.get_temp::<Vec<u64>>(egui::Id::new("tl-layer-drag"))), Some(moving.clone()));
    println!("Dragging {moving:?} from {origin:?} to {bottom:?}");
    h.event(Event::PointerMoved(bottom));
    h.run_steps(60);
    let down = h.state().ui.timeline.scroll_y;
    assert!(down > 100.0);
    h.event(Event::PointerMoved(top));
    h.run_steps(20);
    assert!(h.state().ui.timeline.scroll_y < down);
    h.event(Event::PointerMoved(bottom));
    h.run_steps(300);
    assert_eq!(h.ctx.data(|d| d.get_temp::<Vec<u64>>(egui::Id::new("tl-layer-drag"))), Some(moving.clone()));
    assert!(h.state().auto.find(&format!("timeline.layer.{}.row", moving[0])).is_none());
    h.event(Event::PointerButton { pos: bottom, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
    assert!(h.ctx.data(|d| d.get_temp::<Vec<u64>>(egui::Id::new("tl-layer-drag"))).is_none());
    let order: Vec<u64> = h.state().session.active_comp().unwrap().layers.iter().map(|l| l.id.0).collect();
    println!("Drop order: {order:?}");
    assert_eq!(&order[order.len() - 2..], moving.as_slice());
    let stopped = h.state().ui.timeline.scroll_y;
    h.run_steps(10);
    assert_eq!(h.state().ui.timeline.scroll_y, stopped);
}
