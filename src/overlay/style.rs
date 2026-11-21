//! The overlay look: thin lines, text and dots on a faint flat ground (no framed boxes).

use egui::{Color32, FontId};

pub const GROUND: Color32 = Color32::from_rgba_premultiplied(8, 11, 14, 150);
pub const TEXT: Color32 = Color32::from_rgb(200, 208, 216);
pub const TEXT_DIM: Color32 = Color32::from_rgb(122, 135, 148);
pub const LINE: Color32 = Color32::from_rgb(84, 98, 112);
pub const ACCENT: Color32 = Color32::from_rgb(232, 163, 61);
pub const PLAYHEAD: Color32 = Color32::from_rgb(214, 64, 52);

/// One colour per node family, used for the small dot beside the node title.
pub const SOURCE: Color32 = Color32::from_rgb(232, 163, 61);
pub const DEPUTY: Color32 = Color32::from_rgb(120, 170, 200);
pub const GEOMETRY: Color32 = Color32::from_rgb(110, 190, 150);
pub const OUTPUT: Color32 = Color32::from_rgb(200, 208, 216);

pub fn title_font() -> FontId {
    FontId::proportional(13.0)
}

pub fn port_font() -> FontId {
    FontId::proportional(11.0)
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}
