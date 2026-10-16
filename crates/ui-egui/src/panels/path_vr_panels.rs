//! Create Nulls From Paths panel.

use crate::i18n::tr;
use egui::{Rect, RichText};
use serde_json::{Value, json};

use crate::EffectcraftApp;

fn run(app: &mut EffectcraftApp, ctx: &egui::Context, actions: Vec<(&str, Value)>) {
    for (id, p) in actions {
        match crate::menus::invoke(app, ctx, id, p) {
            Ok(_) => {}
            Err(e) => app.ui.status = e,
        }
    }
}

fn area(ui: &mut egui::Ui, rect: Rect) -> egui::Ui {
    ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(10.0)).layout(egui::Layout::top_down(egui::Align::Min)))
}

/// Window ▸ Create Nulls From Paths.
pub fn create_nulls(app: &mut EffectcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    let mut ui = area(ui, rect);
    let loop_id = egui::Id::new("createNulls.loop");
    let mut looping: bool = ui.data(|d| d.get_temp(loop_id)).unwrap_or(false);
    let ready = app.session.is_enabled("paths.nullsFollowPoints") && !app.session.state.selected_layers.is_empty();
    ui.label(RichText::new(tr("Select a layer with a mask or shape path (or the path property), or Puppet pins.")).color(t.text_dim).small());
    ui.add_space(6.0);
    let mut acts = vec![];
    for (id, label, tip) in [
        ("paths.pointsFollowNulls", "Points Follow Nulls", "A null on each vertex; move the nulls to reshape the path"),
        ("paths.nullsFollowPoints", "Nulls Follow Points", "A null on each vertex that follows the (animated) path"),
        ("paths.tracePath", "Trace Path", "A null moving along the path, driven by its Progress slider"),
    ] {
        let r = ui.add_enabled(ready, egui::Button::new(label).min_size(egui::vec2(170.0, 24.0))).on_hover_text(tip);
        let auto = format!("createNulls.{}", id.trim_start_matches("paths."));
        app.auto.add(&auto, r.rect, label);
        if r.clicked() {
            acts.push((id, if id == "paths.tracePath" { json!({"loop": looping}) } else { json!({}) }));
        }
    }
    let r = ui.checkbox(&mut looping, tr("Loop (Trace Path)"));
    app.auto.add("createNulls.loop", r.rect, "Loop");
    ui.data_mut(|d| d.insert_temp(loop_id, looping));
    run(app, &ctx, acts);
}

