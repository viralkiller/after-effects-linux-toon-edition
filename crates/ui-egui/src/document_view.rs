//! Project-owned editor positions. Rendering and undo never depend on these snapshots.
use std::collections::{BTreeMap, BTreeSet};

use effectcraft_engine::project::ItemId;
use serde::{Deserialize, Serialize};

use crate::{EffectcraftApp, state::UiState, workspace_state::Snapshot};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct CompView {
    start: f64,
    pps: Option<f64>,
    scroll_y: f32,
    columns_w: f32,
    outline_scroll: f32,
    zoom: Option<f32>,
    pan: [f32; 2],
}

impl CompView {
    fn capture(ui: &UiState) -> Self {
        Self {
            start: ui.timeline.start, pps: ui.timeline.pps, scroll_y: ui.timeline.scroll_y,
            columns_w: ui.timeline.columns_w, outline_scroll: ui.timeline.outline_scroll,
            zoom: ui.viewer.zoom, pan: ui.viewer.pan,
        }
    }

    fn valid(&self) -> bool {
        self.start.is_finite() && self.start >= 0.0
            && self.pps.is_none_or(|v| v.is_finite() && v > 0.0 && v <= 1e6)
            && [self.scroll_y, self.columns_w, self.outline_scroll].iter().all(|v| v.is_finite() && (0.0..=1e6).contains(v))
            && self.zoom.is_none_or(|v| v.is_finite() && v > 0.0 && v <= 1e4)
            && self.pan.iter().all(|v| v.is_finite() && v.abs() <= 1e6)
    }

    fn apply(&self, ui: &mut UiState, duration: f64) {
        ui.timeline.start = self.start.clamp(0.0, duration.max(0.0));
        ui.timeline.pps = self.pps;
        ui.timeline.scroll_y = self.scroll_y;
        ui.timeline.columns_w = self.columns_w;
        ui.timeline.outline_scroll = self.outline_scroll;
        ui.viewer.zoom = self.zoom;
        ui.viewer.pan = self.pan;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct DocumentSnapshot {
    version: u32,
    layout: Snapshot,
    comps: BTreeMap<u64, CompView>,
    viewers: BTreeMap<u32, Option<u64>>,
    active_viewer: u32,
    locked_tabs: BTreeSet<String>,
}

#[derive(Default)]
pub(crate) struct DocumentViews {
    active: Option<ItemId>,
    comps: BTreeMap<u64, CompView>,
    saved: Option<DocumentSnapshot>,
}

impl EffectcraftApp {
    pub(crate) fn document_views_initial_comp(&mut self) {
        self.document_views.active = self.session.active_comp_id();
    }

    /// Remember the old comp before routing a newly opened tab to its viewer.
    pub(crate) fn switch_document_view(&mut self, comp: ItemId) {
        if self.document_views.active == Some(comp) {
            return;
        }
        if let Some(old) = self.document_views.active {
            self.document_views.comps.insert(old.0, CompView::capture(&self.ui));
        }
        self.document_views.active = Some(comp);
        if let Some(saved) = self.document_views.comps.get(&comp.0) {
            let duration = self.session.project.comp(comp).map_or(0.0, |c| c.duration.seconds());
            saved.apply(&mut self.ui, duration);
        } else {
            self.ui.timeline.start = 0.0;
            self.ui.timeline.pps = None;
            self.ui.timeline.scroll_y = 0.0;
            self.ui.viewer.zoom = None;
            self.ui.viewer.pan = [0.0; 2];
        }
    }

    pub(crate) fn capture_document_view(&mut self) -> Result<(), String> {
        // An engine command can open a comp before its queued UI event is handled.
        // Do not assign the old canvas positions to that new composition.
        if let Some(id) = self.document_views.active {
            self.document_views.comps.insert(id.0, CompView::capture(&self.ui));
        }
        self.document_views.comps.retain(|id, _| self.session.project.comp(ItemId(*id)).is_some());
        let snapshot = DocumentSnapshot {
            version: 1, layout: Snapshot::capture(&self.ui), comps: self.document_views.comps.clone(),
            viewers: self.ui.viewers.clone(), active_viewer: self.ui.active_viewer,
            locked_tabs: self.ui.locked_tabs.clone(),
        };
        if self.document_views.saved.as_ref() != Some(&snapshot) {
            let value = serde_json::to_value(&snapshot).map_err(|e| format!("Cannot save editor layout: {e}"))?;
            self.session.frontend_state = value;
            self.document_views.saved = Some(snapshot);
        }
        Ok(())
    }

    pub(crate) fn restore_document_view(&mut self) {
        self.document_views = DocumentViews::default();
        self.ui.timeline = Default::default();
        self.ui.viewer = Default::default();
        self.ui.viewer.res = self.session.project.viewport_resolution;
        self.ui.viewers.clear();
        self.ui.active_viewer = 0;
        self.ui.locked_tabs.clear();
        let value = self.session.frontend_state.clone();
        if !value.is_null() {
            let restored = serde_json::from_value::<DocumentSnapshot>(value).map_err(|e| e.to_string()).and_then(|s| {
                if s.version != 1 || s.comps.len() > 4096 || s.viewers.len() > 128 || s.active_viewer > 65_535
                    || s.viewers.keys().any(|id| *id > 65_535) || s.locked_tabs.len() > 128
                    || s.locked_tabs.iter().any(|name| name.len() > 256)
                    || s.comps.values().any(|v| !v.valid()) {
                    return Err("unsupported or invalid editor positions".to_string());
                }
                s.layout.validate()?;
                Ok(s)
            });
            match restored {
                Ok(mut s) => {
                    s.comps.retain(|id, _| self.session.project.comp(ItemId(*id)).is_some());
                    for comp in s.viewers.values_mut() {
                        *comp = comp.filter(|id| self.session.project.comp(ItemId(*id)).is_some());
                    }
                    self.ui.viewers = s.viewers;
                    s.layout.apply(&mut self.ui);
                    self.ui.active_viewer = if s.active_viewer == 0 || self.ui.viewers.contains_key(&s.active_viewer) { s.active_viewer } else { 0 };
                    self.ui.locked_tabs = s.locked_tabs;
                    self.document_views.comps = s.comps;
                }
                Err(e) => self.ui.status = format!("Project opened; saved editor layout ignored: {e}"),
            }
        }
        crate::workspace_state::remove_retired_panels(&mut self.ui);
        let mut panels = Vec::new();
        self.ui.dock.panels(&mut panels);
        for f in &self.ui.floating { panels.extend(f.panels.iter().copied()); }
        let active = crate::panels::viewers::panel(self.ui.active_viewer);
        if !panels.contains(&active) { self.ui.active_viewer = 0; }
        if let Some(id) = self.session.active_comp_id() {
            self.switch_document_view(id);
            self.ui.viewers.insert(self.ui.active_viewer, Some(id.0));
        }
        // Opening a document must not let an old preview move its restored playhead.
        self.playback = Default::default();
        self.audio = None;
        self.viewer_shown = None;
        self.ui.start_screen = false;
    }

    pub(crate) fn refresh_trimmed_comp(&mut self, comp: ItemId) {
        if let Some(view) = self.document_views.comps.get_mut(&comp.0) {
            view.start = 0.0;
            view.pps = None;
        }
        if self.session.active_comp_id() == Some(comp) {
            self.ui.timeline.start = 0.0;
            self.ui.timeline.pps = None;
            if let Some(maximized) = self.playback.restore_max.take() { self.ui.maximized = maximized; }
            self.playback = Default::default();
            self.audio = None;
            self.viewer_shown = None;
        }
    }
}
