//! Project-owned preview settings. They do not affect export resolution.
use serde::{Deserialize, Serialize};

/// A Preview panel shortcut.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreviewShortcut {
    #[default]
    Spacebar,
    ShiftSpacebar,
    Numpad0,
    ShiftNumpad0,
    AltNumpad0,
}

impl PreviewShortcut {
    pub const ALL: [PreviewShortcut; 5] = [Self::Spacebar, Self::ShiftSpacebar, Self::Numpad0, Self::ShiftNumpad0, Self::AltNumpad0];
    pub fn label(self) -> &'static str {
        match self {
            Self::Spacebar => "Spacebar",
            Self::ShiftSpacebar => "Shift+Spacebar",
            Self::Numpad0 => "Numpad 0",
            Self::ShiftNumpad0 => "Shift+Numpad 0",
            Self::AltNumpad0 => "Alt+Numpad 0",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Spacebar => "spacebar",
            Self::ShiftSpacebar => "shiftSpacebar",
            Self::Numpad0 => "numpad0",
            Self::ShiftNumpad0 => "shiftNumpad0",
            Self::AltNumpad0 => "altNumpad0",
        }
    }
    /// From an id (`shiftSpacebar`) or a label (`Shift+Spacebar`).
    pub fn parse(s: &str) -> Option<Self> {
        let k = s.to_ascii_lowercase().replace([' ', '+', '-', '_'], "");
        Self::ALL.into_iter().find(|x| x.id().to_ascii_lowercase() == k || x.label().to_ascii_lowercase().replace([' ', '+'], "") == k)
    }
}

/// Range of a preview.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreviewRange {
    WorkArea,
    #[default]
    WorkAreaExtended,
    EntireDuration,
    /// The pre-roll before and post-roll after the current time.
    AroundCurrentTime,
}

impl PreviewRange {
    pub const ALL: [PreviewRange; 4] = [Self::WorkArea, Self::WorkAreaExtended, Self::EntireDuration, Self::AroundCurrentTime];
    pub fn label(self) -> &'static str {
        match self {
            Self::WorkArea => "Work Area",
            Self::WorkAreaExtended => "Work Area Extended By Current Time",
            Self::EntireDuration => "Entire Duration",
            Self::AroundCurrentTime => "Play Around Current Time",
        }
    }
}

/// Where playback starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayFrom {
    RangeStart,
    #[default]
    CurrentTime,
}

impl PlayFrom {
    pub const ALL: [PlayFrom; 2] = [Self::RangeStart, Self::CurrentTime];
    pub fn label(self) -> &'static str {
        match self {
            Self::RangeStart => "Range Start",
            Self::CurrentTime => "Current Time",
        }
    }
}

/// Resolution of a preview (Auto: the viewer's own).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreviewResolution {
    Auto,
    Full,
    #[default]
    Half,
    Third,
    Quarter,
    /// Every n-th pixel ([`PreviewPreset::custom_resolution`]).
    Custom,
}

impl PreviewResolution {
    pub const ALL: [PreviewResolution; 6] = [Self::Auto, Self::Full, Self::Half, Self::Third, Self::Quarter, Self::Custom];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Full => "Full",
            Self::Half => "Half",
            Self::Third => "Third",
            Self::Quarter => "Quarter",
            Self::Custom => "Custom",
        }
    }
    /// Pixel step (1 Full … 4 Quarter, `custom` for Custom); `None` for Auto.
    pub fn factor(self, custom: u32) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Full => Some(1),
            Self::Half => Some(2),
            Self::Third => Some(3),
            Self::Quarter => Some(4),
            Self::Custom => Some(custom.clamp(1, 40)),
        }
    }
}

/// The options of one shortcut.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PreviewPreset {
    pub include_video: bool,
    pub include_audio: bool,
    /// Grids, guides and safe margins stay visible while playing.
    pub include_overlays: bool,
    /// Layer handles, paths and motion paths stay visible while playing.
    pub include_layer_controls: bool,
    #[serde(rename = "loop")]
    pub loop_: bool,
    /// Render the whole range before playing it.
    pub cache_before_playback: bool,
    pub range: PreviewRange,
    /// Play Around Current Time: seconds before and after the current time.
    pub pre_roll: f64,
    pub post_roll: f64,
    pub play_from: PlayFrom,
    /// Frames per second; `None` = Auto (the comp's rate).
    pub frame_rate: Option<f64>,
    /// Frames skipped between the frames played (0 = every frame).
    pub skip: u32,
    pub resolution: PreviewResolution,
    /// Custom resolution: render every n-th pixel (1–40).
    pub custom_resolution: u32,
    /// Play in a full-screen viewer.
    pub full_screen: bool,
    /// With Cache Before Playback, pressing the shortcut again while caching plays the frames
    /// already cached.
    pub play_cached_frames: bool,
    /// When the preview stops, the current time moves to the last frame shown (off: it returns
    /// to where the preview started).
    pub move_time_to_preview_time: bool,
}

impl Default for PreviewPreset {
    fn default() -> Self {
        PreviewPreset {
            include_video: true,
            include_audio: true,
            include_overlays: true,
            include_layer_controls: false,
            loop_: true,
            cache_before_playback: false,
            range: PreviewRange::WorkAreaExtended,
            pre_roll: 2.0,
            post_roll: 2.0,
            play_from: PlayFrom::CurrentTime,
            frame_rate: None,
            skip: 0,
            resolution: PreviewResolution::Half,
            custom_resolution: 2,
            full_screen: false,
            play_cached_frames: true,
            move_time_to_preview_time: true,
        }
    }
}

impl PreviewPreset {
    /// The factory options of a shortcut.
    pub fn default_for(s: PreviewShortcut) -> Self {
        let d = PreviewPreset::default();
        match s {
            PreviewShortcut::Spacebar => d,
            // A lighter preview: every other frame.
            PreviewShortcut::ShiftSpacebar => PreviewPreset { skip: 1, ..d },
            PreviewShortcut::Numpad0 => PreviewPreset { range: PreviewRange::WorkArea, play_from: PlayFrom::RangeStart, ..d },
            PreviewShortcut::ShiftNumpad0 => {
                PreviewPreset { range: PreviewRange::WorkArea, play_from: PlayFrom::RangeStart, skip: 1, resolution: PreviewResolution::Half, ..d }
            }
            // Audio only.
            PreviewShortcut::AltNumpad0 => PreviewPreset { include_video: false, range: PreviewRange::WorkArea, play_from: PlayFrom::RangeStart, ..d },
        }
    }

    pub fn normalize(&mut self) {
        self.pre_roll = if self.pre_roll.is_finite() { self.pre_roll.clamp(0.0, 3600.0) } else { 2.0 };
        self.post_roll = if self.post_roll.is_finite() { self.post_roll.clamp(0.0, 3600.0) } else { 2.0 };
        self.frame_rate = self.frame_rate.filter(|r| r.is_finite() && *r > 0.0).map(|r| r.clamp(0.01, 999.0));
        self.skip = self.skip.min(99);
        self.custom_resolution = self.custom_resolution.clamp(1, 40);
        if !self.include_video && !self.include_audio {
            self.include_video = true;
        }
    }
}

/// The Preview panel: the shortcut shown in the panel and every shortcut's options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PreviewSettings {
    /// The shortcut whose options the panel shows (and the play button uses).
    pub current: PreviewShortcut,
    pub spacebar: PreviewPreset,
    pub shift_spacebar: PreviewPreset,
    pub numpad0: PreviewPreset,
    pub shift_numpad0: PreviewPreset,
    pub alt_numpad0: PreviewPreset,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        use PreviewShortcut as S;
        PreviewSettings {
            current: S::Spacebar,
            spacebar: PreviewPreset::default_for(S::Spacebar),
            shift_spacebar: PreviewPreset::default_for(S::ShiftSpacebar),
            numpad0: PreviewPreset::default_for(S::Numpad0),
            shift_numpad0: PreviewPreset::default_for(S::ShiftNumpad0),
            alt_numpad0: PreviewPreset::default_for(S::AltNumpad0),
        }
    }
}

impl PreviewSettings {
    pub fn get(&self, s: PreviewShortcut) -> &PreviewPreset {
        match s {
            PreviewShortcut::Spacebar => &self.spacebar,
            PreviewShortcut::ShiftSpacebar => &self.shift_spacebar,
            PreviewShortcut::Numpad0 => &self.numpad0,
            PreviewShortcut::ShiftNumpad0 => &self.shift_numpad0,
            PreviewShortcut::AltNumpad0 => &self.alt_numpad0,
        }
    }
    pub fn get_mut(&mut self, s: PreviewShortcut) -> &mut PreviewPreset {
        match s {
            PreviewShortcut::Spacebar => &mut self.spacebar,
            PreviewShortcut::ShiftSpacebar => &mut self.shift_spacebar,
            PreviewShortcut::Numpad0 => &mut self.numpad0,
            PreviewShortcut::ShiftNumpad0 => &mut self.shift_numpad0,
            PreviewShortcut::AltNumpad0 => &mut self.alt_numpad0,
        }
    }
    /// The options of the shortcut shown in the panel.
    pub fn active(&self) -> &PreviewPreset {
        self.get(self.current)
    }
}

/// Viewer resolution (Auto follows the magnification).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewportResolution {
    Auto,
    Full,
    #[default]
    Half,
    Third,
    Quarter,
    /// View ▸ Resolution ▸ Custom: render every n-th pixel (2..=40).
    Custom(u8),
}

impl ViewportResolution {
    pub const ALL: [ViewportResolution; 5] = [ViewportResolution::Auto, ViewportResolution::Full, ViewportResolution::Half, ViewportResolution::Third, ViewportResolution::Quarter];
    pub fn label(self) -> &'static str {
        match self {
            ViewportResolution::Auto => "Auto",
            ViewportResolution::Full => "Full",
            ViewportResolution::Half => "Half",
            ViewportResolution::Third => "Third",
            ViewportResolution::Quarter => "Quarter",
            ViewportResolution::Custom(_) => "Custom",
        }
    }
    /// Render scale for a viewer magnification (and display pixel density).
    pub fn scale(self, zoom: f32, ppp: f32) -> f64 {
        match self {
            // As in After Effects, Auto renders the pixels the magnification needs: the coarsest
            // whole factor that still gives every screen pixel a rendered one (Full above 50 %,
            // Half down to 33.3 %, Third down to 25 %, Quarter below), never fewer (#417).
            ViewportResolution::Auto => {
                let n = (1.0 / (zoom * ppp) as f64 + 1e-3).floor();
                if n >= 1.0 { 1.0 / n.min(4.0) } else { 1.0 }
            }
            ViewportResolution::Full => 1.0,
            ViewportResolution::Half => 0.5,
            ViewportResolution::Third => 1.0 / 3.0,
            ViewportResolution::Quarter => 0.25,
            ViewportResolution::Custom(n) => 1.0 / n.max(1) as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Project;

    #[test]
    fn half_resolution_is_the_default_for_new_and_old_projects() {
        let project = Project::default();
        assert_eq!(project.viewport_resolution, ViewportResolution::Half);
        for shortcut in PreviewShortcut::ALL {
            assert_eq!(project.preview.get(shortcut).resolution, PreviewResolution::Half);
        }
        let mut old = serde_json::to_value(&project).unwrap();
        old.as_object_mut().unwrap().remove("preview");
        old.as_object_mut().unwrap().remove("viewport_resolution");
        let restored = Project::from_json(&old.to_string()).unwrap();
        assert_eq!(restored.viewport_resolution, ViewportResolution::Half);
        assert_eq!(restored.preview, project.preview);
    }

    #[test]
    fn project_roundtrip_preserves_independent_preview_choices() {
        let mut project = Project::default();
        project.viewport_resolution = ViewportResolution::Custom(3);
        project.preview.spacebar.resolution = PreviewResolution::Full;
        project.preview.numpad0.resolution = PreviewResolution::Quarter;
        let restored = Project::from_json(&project.to_json()).unwrap();
        assert_eq!(restored.viewport_resolution, ViewportResolution::Custom(3));
        assert_eq!(restored.preview, project.preview);
        assert_eq!(restored.settings, project.settings);
    }
}
