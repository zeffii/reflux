//! Everything drawn over the render: the graph, the lane view and the status line.

pub mod graph;
pub mod lane;
pub mod style;

use egui::{Align2, LayerId, Order, Pos2};

use crate::events::song::Song;
use crate::view::{Overlay, ViewState, DIM_LEVELS};
use graph::Graph;

pub struct OverlayFrame<'a> {
    pub view: &'a ViewState,
    pub song: &'a Song,
    pub seconds: f64,
    pub show_status: bool,
}

/// Builds this frame's overlay shapes. Runs inside an egui pass.
pub fn build(ctx: &egui::Context, graph: &mut Graph, frame: &OverlayFrame<'_>) {
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::unique("reflux overlay")));
    let screen = ctx.content_rect();
    let tick = frame.song.tick_at(frame.seconds);

    match frame.view.overlay {
        Some(Overlay::Graph) => {
            graph.handle_input(ctx);
            graph.draw(&painter, frame.song, tick);
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
    }
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
    format!(
        "{}  {:02}:{:05.2}  row {row:03}  {overlay}  dim {:.0}%  {}    tab graph · L lane · D dim · S soft · F freeze · P peek · Q quality · space play",
        frame.song.name,
        (looped / 60.0) as u32,
        looped % 60.0,
        DIM_LEVELS[dim.level] * 100.0,
        flags.join(" "),
    )
}
