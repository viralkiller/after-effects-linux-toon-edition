//! Layer source inspection with masks and effects; artwork is authored externally.

use crate::i18n::{tr, tr_args};
use effectcraft_engine::project::{Layer, LayerId};
use effectcraft_engine::render::{EvalCtx, RenderOpts, Renderer};
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::theme::Tokens;
use crate::{EffectcraftApp, widgets};

/// Cached texture of the rendered layer: (revision, layer, time, view, scale) → texture and the
/// layer-space rectangle it covers.
#[derive(Clone)]
struct Tex {
    key: (u64, u64, i64, usize, u32),
    tex: egui::TextureHandle,
    /// Layer-space rect of the image `[x0, y0, x1, y1]`.
    rect: [f64; 4],
}

fn tex_id() -> egui::Id {
    egui::Id::new("layer-panel-tex")
}

/// The layer shown: the opened one, else the first selected layer.
pub fn current_layer(app: &EffectcraftApp) -> Option<Layer> {
    let comp = app.session.active_comp()?;
    let id = app.ui.layer_panel.map(LayerId).filter(|id| comp.layer(*id).is_some()).or_else(|| app.session.state.selected_layers.first().copied())?;
    comp.layer(id).cloned()
}

pub fn show(app: &mut EffectcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    let p = ui.painter().with_clip_rect(rect);
    let (Some(cid), Some(layer)) = (app.session.active_comp_id(), current_layer(app)) else {
        p.rect_filled(rect, 0.0, t.pasteboard);
        p.text(rect.center(), Align2::CENTER_CENTER, tr("Double-click a layer to open it in the Layer panel"), Tokens::ui(12.0), t.text_faint);
        app.auto.add("layerPanel.empty", rect, "Layer panel (empty)");
        return;
    };
    let comp = app.session.project.comp_arc(cid).unwrap_or_else(|| effectcraft_engine::project::Comp::new(1, 1, Default::default(), Default::default()).into());
    let time = app.session.time();
    let fx_count = layer.effects().map(|f| f.groups().count()).unwrap_or(0);
    let view = app.ui.layer_view.map(|v| v.min(fx_count)).unwrap_or(fx_count);

    // Title strip.
    let top = Rect::from_min_size(rect.min, vec2(rect.width(), 24.0));
    p.rect_filled(top, 0.0, t.panel_bg);
    p.text(pos2(top.min.x + 10.0, top.center().y), Align2::LEFT_CENTER, tr_args("Layer: {}", &[&layer.name]), Tokens::semibold(12.0), t.text);
    app.auto.add("layerPanel.title", top, &layer.name);
    // Bottom bar: View menu.
    let bar = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 30.0), rect.max);
    p.rect_filled(bar, 0.0, t.panel_bg);
    p.text(pos2(bar.min.x + 10.0, bar.center().y), Align2::LEFT_CENTER, tr("View:"), Tokens::ui(12.0), t.text_dim);
    let mut names = vec!["Masks".to_string()];
    if let Some(fx) = layer.effects() {
        names.extend(fx.groups().map(|g| g.name.clone()));
    }
    let vr = Rect::from_min_size(pos2(bar.min.x + 46.0, bar.center().y - 10.0), vec2(150.0, 20.0));
    let pop = egui::Id::new("layer-view-pop");
    if widgets::dropdown(ui, vr, &names[view.min(names.len() - 1)], &t, egui::Id::new("layer-view")).clicked() {
        widgets::open_popup(ui, pop);
    }
    app.auto.add("layerPanel.view", vr, "View");
    if let Some(i) = widgets::popup_menu(ui, pop, vr.left_bottom(), &names, Some(view)) {
        app.ui.layer_view = Some(i);
    }
    let tc = super::timecode(&app.session, &comp, time);
    p.text(pos2(bar.max.x - 10.0, bar.center().y), Align2::RIGHT_CENTER, tc, Tokens::mono(12.0), t.timecode);
    // Canvas.
    let area = Rect::from_min_max(pos2(rect.min.x, top.max.y), pos2(rect.max.x, bar.min.y));
    p.rect_filled(area, 0.0, t.pasteboard);
    // The layer's effect bounds, in effect space (where paint and Roto Brush strokes are).
    let [lw, lh] = effectcraft_engine::render::effect_bounds(&app.session.project, &comp, &layer).0;
    let zoom = ((area.width() - 40.0) as f64 / lw).min((area.height() - 40.0) as f64 / lh).max(0.01);
    let origin = area.center() - vec2((lw * zoom / 2.0) as f32, (lh * zoom / 2.0) as f32);
    let to_screen = |q: [f64; 2]| pos2(origin.x + (q[0] * zoom) as f32, origin.y + (q[1] * zoom) as f32);
    let to_layer = |s: Pos2| [((s.x - origin.x) as f64) / zoom, ((s.y - origin.y) as f64) / zoom];
    let frame = Rect::from_min_max(to_screen([0.0, 0.0]), to_screen([lw, lh]));
    let painter = p.with_clip_rect(area);
    super::viewer::checker(&painter, frame);

    // Render (synchronously; the layer cache makes repeats cheap).
    let ppp = ctx.pixels_per_point() as f64;
    let scale = (zoom * ppp).clamp(0.05, 1.0);
    let scale = if scale > 0.75 {
        1.0
    } else if scale > 0.4 {
        0.5
    } else {
        0.25
    };
    let key = (app.session.revision, layer.id.0, time.0, view, (scale * 1000.0) as u32);
    let cached: Option<Tex> = ctx.data(|d| d.get_temp(tex_id()));
    let tex = match cached {
        Some(c) if c.key == key => Some(c),
        _ => {
            let ectx = EvalCtx { project: &app.session.project, comp_id: cid, comp: &comp, time, expr: app.session.expr.as_deref(), footage: None };
            let mut r = Renderer::new(&app.session.project, &*app.session.footage, RenderOpts { scale, ..Default::default() });
            r.expr = app.session.expr.as_deref();
            r.cache = Some(&app.session.layer_cache);
            r.layer_input(&ectx, &layer, view).map(|buf| {
                let img = crate::frames::to_color_image(&buf.img);
                let x0 = -buf.offset[0] / buf.scale;
                let y0 = -buf.offset[1] / buf.scale;
                let rect = [x0, y0, x0 + buf.img.width as f64 / buf.scale, y0 + buf.img.height as f64 / buf.scale];
                let tex = crate::frames::load_fitted(&ctx, "layer-panel", img, egui::TextureOptions::LINEAR);
                let c = Tex { key, tex, rect };
                ctx.data_mut(|d| d.insert_temp(tex_id(), c.clone()));
                c
            })
        }
    };
    if let Some(tx) = &tex {
        let r = Rect::from_min_max(to_screen([tx.rect[0], tx.rect[1]]), to_screen([tx.rect[2], tx.rect[3]]));
        painter.image(tx.tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    painter.rect_stroke(frame, 0.0, Stroke::new(1.0, Color32::from_black_alpha(160)), StrokeKind::Outside);
    app.auto.add("layerPanel.canvas", frame, &layer.name);
    app.auto.add("layerPanel.area", area, "Layer panel");

    let resp = ui.interact(area, egui::Id::new("layer-panel-interact"), Sense::hover());
    if let Some(pos) = resp.hover_pos() {
        let point = to_layer(pos);
        app.pointer_comp = Some([point[0] as f32, point[1] as f32]);
    }
}
