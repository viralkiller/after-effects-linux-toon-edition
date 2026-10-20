//! Progress and background import status.

use crate::i18n::tr;
use egui::{Align2, Rect, Stroke, pos2, vec2};
use serde_json::json;

use super::panel_kit as kit;
use crate::theme::Tokens;
use crate::{EffectcraftApp, widgets};

// ---------------------------------------------------------------- Progress

pub fn progress(app: &mut EffectcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 0.0, t.panel_bg);
    if !app.session.tasks.is_empty() {
        app.session.poll_jobs();
    }
    let jobs = app.session.jobs();
    let mut y = rect.min.y + 8.0;
    let x0 = rect.min.x + 12.0;
    let w = (rect.width() - 24.0).max(60.0);
    if jobs.is_empty() {
        p.text(pos2(x0, y + 10.0), Align2::LEFT_CENTER, tr("No background jobs running"), Tokens::ui(12.0), t.text_faint);
        y += 28.0;
    } else {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
    }
    for j in &jobs {
        p.text(pos2(x0, y + 8.0), Align2::LEFT_CENTER, &j.label, Tokens::medium(12.0), t.text);
        let frac = j.fraction.unwrap_or(0.0) as f32;
        let pct = j.fraction.map(|f| format!("{:.0} %", f * 100.0)).unwrap_or_else(|| "…".into());
        p.text(pos2(x0 + w - 70.0, y + 8.0), Align2::RIGHT_CENTER, format!("{pct}  {}", j.message), Tokens::ui(11.0), t.text_dim);
        progress_bar(&p, Rect::from_min_size(pos2(x0, y + 20.0), vec2(w - 76.0, 6.0)), frac, &t);
        app.auto.add(&format!("progress.job.{}", j.id), Rect::from_min_size(pos2(x0, y), vec2(w, 28.0)), &j.label);
        if j.cancellable
            && kit::button(app, ui, Rect::from_min_size(pos2(x0 + w - 64.0, y + 12.0), vec2(64.0, 18.0)), &format!("progress.cancel.{}", j.id), "Cancel", false)
        {
            kit::exec(app, "jobs.cancel", json!({"job": j.id}));
        }
        y += 38.0;
    }
    if !app.session.job_log.is_empty() {
        p.line_segment([pos2(x0, y), pos2(x0 + w, y)], Stroke::new(1.0, t.separator));
        y += 6.0;
        p.text(pos2(x0, y + 8.0), Align2::LEFT_CENTER, tr("Finished"), Tokens::semibold(11.5), t.text_dim);
        y += 20.0;
        for r in app.session.job_log.iter().rev().take(20) {
            let col = match r.status.as_str() {
                "done" => t.text,
                "cancelled" => t.text_dim,
                _ => t.danger,
            };
            let msg = if r.message.is_empty() { format!("{} · {:.1} s", r.status, r.seconds) } else { format!("{} · {}", r.status, r.message) };
            p.text(pos2(x0, y + 8.0), Align2::LEFT_CENTER, &r.label, Tokens::ui(11.5), col);
            p.text(pos2(x0 + w, y + 8.0), Align2::RIGHT_CENTER, msg, Tokens::ui(11.0), t.text_faint);
            y += 18.0;
            if y > rect.max.y {
                break;
            }
        }
    }
}

/// A job's progress bar (`frac` 0…1).
fn progress_bar(p: &egui::Painter, bar: Rect, frac: f32, t: &Tokens) {
    p.rect_filled(bar, 3.0, t.field_bg);
    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * frac.clamp(0.0, 1.0), bar.height())), 3.0, t.accent);
}

/// The Importing card, bottom left over the panels while files import in the background (dropped
/// or picked in the Import dialog): the file being read, how many are left, and Cancel (#270).
/// Returns the height it takes, so toasts sit above it.
pub fn import_card(app: &mut EffectcraftApp, ui: &mut egui::Ui, full: Rect) -> f32 {
    if app.session.tasks.is_empty() {
        return 0.0;
    }
    let t = app.tokens;
    let w = 320.0_f32.min(full.width() - 32.0).max(120.0);
    let mut bottom = full.max.y - 16.0;
    for j in app.session.jobs().into_iter().filter(|j| j.kind == "import" && j.running) {
        let r = Rect::from_min_max(pos2(full.min.x + 16.0, bottom - 56.0), pos2(full.min.x + 16.0 + w, bottom));
        let p = ui.painter();
        p.rect_filled(r, 6.0, t.panel_bg);
        p.rect_stroke(r, 6.0, Stroke::new(1.0, t.field_border), egui::StrokeKind::Inside);
        let x0 = r.min.x + 12.0;
        let inner = w - 24.0;
        widgets::text_fit(p, pos2(x0, r.min.y + 15.0), Align2::LEFT_CENTER, &j.label, Tokens::medium(12.0), inner - 64.0, t.text);
        widgets::text_fit(p, pos2(x0, r.min.y + 32.0), Align2::LEFT_CENTER, &j.message, Tokens::ui(11.5), inner, t.text_dim);
        progress_bar(p, Rect::from_min_size(pos2(x0, r.min.y + 42.0), vec2(inner, 6.0)), j.fraction.unwrap_or(0.0) as f32, &t);
        app.auto.add(&format!("import.job.{}", j.id), r, &j.message);
        if kit::button(app, ui, Rect::from_min_size(pos2(r.max.x - 68.0, r.min.y + 6.0), vec2(56.0, 18.0)), &format!("import.cancel.{}", j.id), "Cancel", false)
        {
            kit::exec(app, "jobs.cancel", json!({"job": j.id}));
        }
        bottom = r.min.y - 8.0;
    }
    full.max.y - 16.0 - bottom
}
