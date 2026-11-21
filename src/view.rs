//! What is on screen: which overlay is open and how the render behind it is treated.
//!
//! Everything lives in one window. The render is always the backdrop; overlays
//! (graph, lane view, later script editors) float on top. Each overlay remembers
//! its own backdrop setting, so switching overlays also switches the mood.

use crate::render::scene_pass::Quality;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Graph,
    Lane,
}

/// Brightness reduction for each dim level.
pub const DIM_LEVELS: [f32; 4] = [0.0, 0.35, 0.6, 0.85];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimSetting {
    pub level: usize,
    /// Desaturate and blur as well as darken: keeps motion visible but calm.
    pub soft: bool,
}

/// The values the composite shader needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub dim: f32,
    pub desaturate: f32,
    pub blur_pixels: f32,
}

impl Backdrop {
    pub const CLEAR: Backdrop = Backdrop {
        dim: 0.0,
        desaturate: 0.0,
        blur_pixels: 0.0,
    };
}

pub struct ViewState {
    pub overlay: Option<Overlay>,
    no_overlay_dim: DimSetting,
    graph_dim: DimSetting,
    lane_dim: DimSetting,
    /// Hold the current frame as a still backdrop (playback time keeps running).
    pub frozen: bool,
    /// While the peek key is held the render shows at full brightness.
    pub peeking: bool,
    pub playing: bool,
    pub quality: Quality,
    /// The SDF core object; hidden by default once a mesh is loaded in its place.
    pub core_visible: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            overlay: None,
            no_overlay_dim: DimSetting { level: 0, soft: false },
            graph_dim: DimSetting { level: 2, soft: true },
            lane_dim: DimSetting { level: 3, soft: false },
            frozen: false,
            peeking: false,
            playing: true,
            quality: Quality::Standard,
            core_visible: true,
        }
    }
}

impl ViewState {
    pub fn toggle_overlay(&mut self, overlay: Overlay) {
        self.overlay = if self.overlay == Some(overlay) {
            None
        } else {
            Some(overlay)
        };
    }

    pub fn dim_setting(&self) -> DimSetting {
        match self.overlay {
            None => self.no_overlay_dim,
            Some(Overlay::Graph) => self.graph_dim,
            Some(Overlay::Lane) => self.lane_dim,
        }
    }

    fn dim_setting_mut(&mut self) -> &mut DimSetting {
        match self.overlay {
            None => &mut self.no_overlay_dim,
            Some(Overlay::Graph) => &mut self.graph_dim,
            Some(Overlay::Lane) => &mut self.lane_dim,
        }
    }

    /// Steps the current overlay's dim level: 0 -> 35% -> 60% -> 85% -> 0.
    pub fn cycle_dim(&mut self) {
        let setting = self.dim_setting_mut();
        setting.level = (setting.level + 1) % DIM_LEVELS.len();
    }

    pub fn set_dim(&mut self, setting: DimSetting) {
        *self.dim_setting_mut() = DimSetting {
            level: setting.level.min(DIM_LEVELS.len() - 1),
            soft: setting.soft,
        };
    }

    pub fn toggle_soft(&mut self) {
        let setting = self.dim_setting_mut();
        setting.soft = !setting.soft;
    }

    pub fn toggle_quality(&mut self) {
        self.quality = match self.quality {
            Quality::Standard => Quality::Eco,
            Quality::Eco => Quality::Standard,
        };
    }

    pub fn backdrop(&self) -> Backdrop {
        if self.peeking {
            return Backdrop::CLEAR;
        }
        let setting = self.dim_setting();
        Backdrop {
            dim: DIM_LEVELS[setting.level],
            desaturate: if setting.soft { 0.65 } else { 0.0 },
            blur_pixels: if setting.soft { 7.0 } else { 0.0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_overlay_remembers_its_own_dim() {
        let mut view = ViewState::default();
        assert_eq!(view.backdrop(), Backdrop::CLEAR);

        view.toggle_overlay(Overlay::Graph);
        let graph = view.backdrop();
        assert!(graph.dim > 0.0 && graph.blur_pixels > 0.0);

        view.cycle_dim();
        view.toggle_overlay(Overlay::Lane);
        view.toggle_overlay(Overlay::Graph);
        assert_eq!(view.dim_setting().level, 3);
    }

    #[test]
    fn peeking_shows_the_render_clean() {
        let mut view = ViewState::default();
        view.toggle_overlay(Overlay::Lane);
        view.peeking = true;
        assert_eq!(view.backdrop(), Backdrop::CLEAR);
    }
}
