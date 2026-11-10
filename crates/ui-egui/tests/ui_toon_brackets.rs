use effectcraft_engine::{Session, project::LayerId, time::Tick};
use effectcraft_ui_egui::EffectcraftApp;
use egui::{Event, Key, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;

fn harness() -> (Harness<'static, EffectcraftApp>, LayerId) {
    let mut s = Session::default();
    s.execute("comp.new", json!({"width": 32, "height": 32, "duration": 20, "frameRate": 30})).unwrap();
    s.execute("layer.newNull", json!({"name": "Move me"})).unwrap();
    let id = s.active_comp().unwrap().layers[0].id;
    s.execute("layer.timing", json!({"start": 1, "in": 2, "out": 6})).unwrap();
    s.execute("prop.addKey", json!({"layer": id.0, "path": "transform/opacity", "time": 1, "value": 50})).unwrap();
    s.execute("layer.select", json!({"layers": []})).unwrap();
    s.set_time(Tick::from_seconds_f64(10.0));
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_eframe(move |_| EffectcraftApp::new(s));
    h.run_steps(3);
    (h, id)
}

fn key(h: &mut Harness<'_, EffectcraftApp>, key: Key, modifiers: Modifiers) {
    println!("Press {modifiers:?}+{key:?}; focus={:?}", h.ctx.memory(|m| m.focused()));
    for pressed in [true, false] {
        h.event(Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
        h.step();
    }
    h.run_steps(2);
    println!("Command status: {}", h.state().ui.status);
}

#[test]
fn bracket_keys_move_clicked_layers_and_undo_without_changing_animation() {
    for widget_focus in [false, true] {
        for surface in ["row", "bar"] {
            for bracket in [Key::OpenBracket, Key::CloseBracket] {
                let (mut h, id) = harness();
                let r = h.state().auto.find(&format!("timeline.layer.{}.{surface}", id.0)).unwrap().rect;
                let x = if surface == "row" {
                    let twirl = h.state().auto.find(&format!("timeline.layer.{}.twirl", id.0)).unwrap().rect;
                    twirl[0] + twirl[2] + 40.0
                } else {
                    r[0] + r[2] / 2.0
                };
                let p = pos2(x, r[1] + r[3] / 2.0);
                println!("Click {surface} at {p:?}");
                h.event(Event::PointerMoved(p));
                h.step();
                for pressed in [true, false] {
                    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
                    h.step();
                }
                h.run_steps(2);
                assert_eq!(h.state().session.state.selected_layers, vec![id]);
                if widget_focus {
                    // Keyboard navigation can leave row focus.
                    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new((surface, id.0))));
                    h.run_steps(2);
                    assert!(h.ctx.egui_wants_keyboard_input());
                    assert!(!h.ctx.text_edit_focused());
                }
                let before = h.state().session.active_comp().unwrap().layer(id).unwrap().clone();
                let undo = h.state().session.history.undo.len();
                key(&mut h, bracket, Modifiers::NONE);
                let after = h.state().session.active_comp().unwrap().layer(id).unwrap();
                let edge = if bracket == Key::OpenBracket { before.in_point } else { before.out_point };
                let delta = h.state().session.time() - edge;
                assert_eq!(after.in_point, before.in_point + delta, "{surface}: {bracket:?}");
                assert_eq!(after.out_point, before.out_point + delta);
                assert_eq!(after.start_time, before.start_time + delta);
                assert_eq!(after.props, before.props);
                assert_eq!(h.state().session.history.undo.len(), undo + 1);
                h.state_mut().session.execute("edit.undo", json!({})).unwrap();
                assert_eq!(h.state().session.active_comp().unwrap().layer(id).unwrap(), &before);
            }
        }
    }
}

#[test]
fn brackets_do_not_move_layers_while_renaming() {
    let (mut h, id) = harness();
    h.state_mut().session.execute("layer.select", json!({"layers": [id.0]})).unwrap();
    let ctx = h.ctx.clone();
    effectcraft_ui_egui::menus::invoke(h.state_mut(), &ctx, "layer.rename", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.ctx.text_edit_focused());
    let before = h.state().session.active_comp().unwrap().layer(id).unwrap().clone();
    for bracket in [Key::OpenBracket, Key::CloseBracket] {
        key(&mut h, bracket, Modifiers::NONE);
        assert_eq!(h.state().session.active_comp().unwrap().layer(id).unwrap(), &before);
    }
}

#[test]
fn focused_timeline_honors_alt_trimming_and_custom_timing_bindings() {
    for (command, pressed, modifiers) in [
        ("timeline.trimInToTime", Key::OpenBracket, Modifiers::ALT),
        ("timeline.trimOutToTime", Key::CloseBracket, Modifiers::ALT),
        ("timeline.moveInToTime", Key::F6, Modifiers::NONE),
    ] {
        let (mut h, id) = harness();
        h.state_mut().session.execute("layer.select", json!({"layers": [id.0]})).unwrap();
        h.state_mut().session.set_time(Tick::from_seconds_f64(4.0));
        if pressed == Key::F6 {
            h.state_mut().session.execute("shortcuts.set", json!({"command": command, "keys": "F6"})).unwrap();
        }
        h.ctx.memory_mut(|m| m.request_focus(egui::Id::new(("row", id.0))));
        h.run_steps(2);
        assert!(h.ctx.egui_wants_keyboard_input());
        let before = h.state().session.active_comp().unwrap().layer(id).unwrap().clone();
        key(&mut h, pressed, modifiers);
        let after = h.state().session.active_comp().unwrap().layer(id).unwrap();
        if command == "timeline.trimInToTime" {
            assert_eq!(after.in_point, Tick::from_seconds_f64(4.0));
            assert_eq!(after.out_point, before.out_point);
            assert_eq!(after.start_time, before.start_time);
        } else if command == "timeline.trimOutToTime" {
            assert_eq!(after.out_point, Tick::from_seconds_f64(4.0));
            assert_eq!(after.in_point, before.in_point);
            assert_eq!(after.start_time, before.start_time);
        } else {
            assert_eq!(after.in_point, Tick::from_seconds_f64(4.0));
            assert_eq!(after.out_point - after.in_point, before.out_point - before.in_point);
        }
        assert_eq!(after.props, before.props);
    }
}
