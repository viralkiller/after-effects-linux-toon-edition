use effectcraft_engine::{Session, geom::vec3, project::LayerId, render::EvalCtx};
use effectcraft_ui_egui::EffectcraftApp;
use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;

fn fixture() -> (Harness<'static, EffectcraftApp>, Vec<LayerId>) {
    let mut s = Session::default();
    s.execute("comp.new", json!({"width":64,"height":48,"duration":4})).unwrap();
    let mut ids = vec![];
    for name in ["Hand", "Mouth A", "Mouth B", "Other"] {
        let id = s.execute("layer.newNull", json!({"name":name})).unwrap()["layer"].as_u64().unwrap();
        ids.push(LayerId(id));
    }
    s.execute("prop.set", json!({"layer":ids[0].0,"path":"transform/position","value":[20,30,0]})).unwrap();
    s.execute("layer.timing", json!({"layers":[ids[2].0],"in":2.0})).unwrap();
    s.state.selected_layers = ids[1..3].to_vec();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_eframe(move |_| EffectcraftApp::new(s));
    h.run_steps(3);
    (h, ids)
}
fn center(h: &Harness<'_, EffectcraftApp>, id: LayerId, suffix: &str) -> Pos2 {
    let r = h.state().auto.find(&format!("timeline.layer.{}.{suffix}", id.0)).unwrap().rect;
    pos2(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
}
fn drag(h: &mut Harness<'_, EffectcraftApp>, from: Pos2, to: Pos2) {
    h.event(Event::PointerMoved(from));
    h.step();
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    let steps = if from == to { 1 } else { 8 };
    for k in 1..=steps {
        h.event(Event::PointerMoved(from + (to - from) * (k as f32 / steps as f32)));
        h.step();
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}
fn choose(h: &mut Harness<'_, EffectcraftApp>, source: LayerId, parent: LayerId) {
    let p = center(h, source, "parent");
    drag(h, p, p);
    let c = h.state().session.active_comp().unwrap();
    let index = c.layers.iter().position(|l| l.id == parent).unwrap() + 1;
    let label = format!("{index}. {}", c.layer(parent).unwrap().name);
    let p = h.query_by_label(&label).unwrap().rect().center();
    drag(h, p, p);
}
fn parent(h: &Harness<'_, EffectcraftApp>, id: LayerId) -> Option<LayerId> {
    h.state().session.active_comp().unwrap().layer(id).unwrap().parent
}
#[test]
fn dropdown_parents_selection_preserves_positions_and_undoes_together() {
    let (mut h, ids) = fixture();
    let before = h.state().session.project.clone();
    let undo = h.state().session.history.undo.len();
    choose(&mut h, ids[1], ids[0]);
    for id in &ids[1..3] {
        assert_eq!(parent(&h, *id), Some(ids[0]));
        let s = &h.state().session;
        let cid = s.active_comp_id().unwrap();
        let old = before.comp(cid).unwrap();
        let new = s.active_comp().unwrap();
        let a = EvalCtx::new(&before, cid, old, s.time()).world_matrix(old.layer(*id).unwrap());
        let b = EvalCtx::new(&s.project, cid, new, s.time()).world_matrix(new.layer(*id).unwrap());
        for p in [vec3(0., 0., 0.), vec3(10., 20., 0.)] {
            assert!((a.apply(p) - b.apply(p)).length() < 1e-9);
        }
    }
    if let Ok(path) = std::env::var("EFFECTCRAFT_PARENT_SNAPSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    assert_eq!(parent(&h, ids[3]), None);
    assert_eq!(h.state().session.state.selected_layers, ids[1..3]);
    assert_eq!(h.state().session.history.undo.len(), undo + 1);
    assert!(h.state_mut().session.undo());
    assert!(std::sync::Arc::ptr_eq(&before, &h.state().session.project));
}
#[test]
fn pick_whip_parents_every_selected_layer() {
    let (mut h, ids) = fixture();
    let from = center(&h, ids[1], "pickWhip");
    let to = center(&h, ids[0], "row");
    drag(&mut h, from, to);
    for id in &ids[1..3] {
        assert_eq!(parent(&h, *id), Some(ids[0]));
    }
    assert_eq!(parent(&h, ids[3]), None);
}
#[test]
fn unselected_row_only_changes_itself() {
    let (mut h, ids) = fixture();
    choose(&mut h, ids[3], ids[0]);
    assert_eq!(parent(&h, ids[3]), Some(ids[0]));
    for id in &ids[1..3] {
        assert_eq!(parent(&h, *id), None);
    }
}
#[test]
fn selected_parent_is_not_parented_to_itself() {
    let (mut h, ids) = fixture();
    h.state_mut().session.state.selected_layers.push(ids[0]);
    h.run_steps(2);
    choose(&mut h, ids[1], ids[0]);
    for id in &ids[1..3] {
        assert_eq!(parent(&h, *id), Some(ids[0]));
    }
    assert_eq!(parent(&h, ids[0]), None);
}
#[test]
fn clearing_parent_applies_to_selection() {
    let (mut h, ids) = fixture();
    h.state_mut().session.execute("layer.setParent", json!({"layers":[ids[1].0,ids[2].0],"parent":ids[0].0})).unwrap();
    h.run_steps(3);
    let undo = h.state().session.history.undo.len();
    let p = center(&h, ids[1], "parent");
    drag(&mut h, p, p);
    let p = h.query_by_label("None").unwrap().rect().center();
    drag(&mut h, p, p);
    for id in &ids[1..3] {
        assert_eq!(parent(&h, *id), None);
    }
    assert_eq!(h.state().session.history.undo.len(), undo + 1);
}
#[test]
fn cycle_in_any_selected_layer_rejects_whole_operation() {
    let (mut h, ids) = fixture();
    h.state_mut().session.execute("layer.setParent", json!({"layers":[ids[0].0],"parent":ids[2].0})).unwrap();
    h.run_steps(3);
    let before = h.state().session.project.clone();
    let undo = h.state().session.history.undo.len();
    choose(&mut h, ids[1], ids[0]);
    assert!(std::sync::Arc::ptr_eq(&before, &h.state().session.project));
    assert_eq!(h.state().session.history.undo.len(), undo);
}
#[test]
fn locked_selected_layers_are_unchanged() {
    let (mut h, ids) = fixture();
    h.state_mut().session.execute("layer.setSwitch", json!({"layers":[ids[2].0],"switch":"locked","value":true})).unwrap();
    h.state_mut().session.state.selected_layers = ids[1..3].to_vec();
    h.run_steps(3);
    choose(&mut h, ids[1], ids[0]);
    assert_eq!(parent(&h, ids[1]), Some(ids[0]));
    assert_eq!(parent(&h, ids[2]), None);
}
