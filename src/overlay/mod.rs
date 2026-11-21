//! Everything drawn over the render: the graph, the lane view and the status line.

pub mod graph;
pub mod lane;
pub mod style;

use egui::{Align2, LayerId, Order, Pos2};

use crate::events::song::Song;
use crate::graph::eval::Evaluation;
use crate::graph::Graph;
use crate::view::{Overlay, ViewState, DIM_LEVELS};
use graph::GraphView;

pub struct OverlayFrame<'a> {
    pub view: &'a ViewState,
    pub song: &'a Song,
    pub evaluation: &'a Evaluation,
    /// Scene file name, and the last error reading it.
    pub scene_name: &'a str,
    pub scene_error: Option<&'a str>,
    pub seconds: f64,
    pub show_status: bool,
    /// Name and triangle count of the loaded mesh, for the status line.
    pub mesh: Option<(&'a str, usize)>,
}

/// Builds this frame's overlay shapes. Runs inside an egui pass. Returns true when a
/// node drag just ended (so the layout can be saved).
pub fn build(ctx: &egui::Context, view: &mut GraphView, graph: &mut Graph, frame: &OverlayFrame<'_>) -> bool {
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::unique("reflux overlay")));
    let screen = ctx.content_rect();
    let tick = frame.song.tick_at(frame.seconds);
    let mut dropped = false;

    match frame.view.overlay {
        Some(Overlay::Graph) => {
            dropped = view.handle_input(ctx, graph);
            view.draw(&painter, graph, frame.evaluation, frame.song, tick);
        }
        Some(Overlay::Lane) => lane::draw(&painter, screen, frame.song, frame.song.row_at(frame.seconds)),
        None => {}
    }

    if frame.show_status {
        painter.text(
            Pos2::new(screen.left() + 10.0, screen.bottom() - 10.0),
            Align2::LEFT_CENTER,
            status_line(frame),
            style::mono(11.0),
            style::TEXT_DIM,
        );
        if let Some(error) = frame.scene_error {
            painter.text(
                Pos2::new(screen.left() + 10.0, screen.bottom() - 26.0),
                Align2::LEFT_CENTER,
                format!("{}: {error}", frame.scene_name),
                style::mono(11.0),
                style::PLAYHEAD,
            );
        }
    }
    dropped
}

fn status_line(frame: &OverlayFrame<'_>) -> String {
    let view = frame.view;
    let looped = frame.seconds.rem_euclid(frame.song.length_seconds());
    let row = frame.song.row_at(frame.seconds).floor() as u32;
    let overlay = match view.overlay {
        None => "render",
        Some(Overlay::Graph) => "graph",
        Some(Overlay::Lane) => "lane",
    };
    let dim = view.dim_setting();
    let mut flags = Vec::new();
    if !view.playing {
        flags.push("paused");
    }
    if view.frozen {
        flags.push("frozen");
    }
    if dim.soft {
        flags.push("soft");
    }
    if view.quality == crate::render::scene_pass::Quality::Eco {
        flags.push("eco");
    }
    let mesh = frame
        .mesh
        .map(|(name, triangles)| format!("  {name} {triangles} tris"))
        .unwrap_or_default();
    format!(
        "{} · {}  {:02}:{:05.2}  row {row:03}  {overlay}  dim {:.0}%  {}{mesh}    tab graph · L lane · D dim · S soft · F freeze · P peek · Q quality · C core · ctrl+S save layout",
        frame.scene_name,
        frame.song.name,
        (looped / 60.0) as u32,
        looped % 60.0,
        DIM_LEVELS[dim.level] * 100.0,
        flags.join(" "),
    )
}
