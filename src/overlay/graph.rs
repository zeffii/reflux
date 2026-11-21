//! The node graph overlay.
//!
//! For now the graph mirrors the hard-wired mappings in `scene.rs`, so you can see
//! how Wax events flow: the stream node exposes one socket per command range,
//! deputy nodes unfold a range into named commands, and the scene nodes consume
//! them. Output ports show their live value. Nodes can be dragged by their title.

use egui::{Align2, Color32, Painter, Pos2, Rect, Stroke, Vec2};

use super::style;
use crate::events::command::{self, Kind};
use crate::events::song::Song;

const NODE_WIDTH: f32 = 150.0;
const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 17.0;
const PORT_RADIUS: f32 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Source,
    Deputy,
    Geometry,
    Output,
}

impl Family {
    fn colour(self) -> Color32 {
        match self {
            Family::Source => style::SOURCE,
            Family::Deputy => style::DEPUTY,
            Family::Geometry => style::GEOMETRY,
            Family::Output => style::OUTPUT,
        }
    }
}

/// What an output port shows live. Range ports light up when any command in the range fires.
#[derive(Clone, Copy, Debug)]
pub enum Probe {
    Off,
    Command(u8),
    Range(u8),
}

pub struct OutputPort {
    pub name: String,
    pub probe: Probe,
}

pub struct Node {
    pub title: String,
    pub family: Family,
    pub position: Pos2,
    pub inputs: Vec<String>,
    pub outputs: Vec<OutputPort>,
}

impl Node {
    fn rows(&self) -> usize {
        self.inputs.len().max(self.outputs.len())
    }

    fn rect(&self) -> Rect {
        let height = HEADER_HEIGHT + self.rows() as f32 * ROW_HEIGHT + 6.0;
        Rect::from_min_size(self.position, Vec2::new(NODE_WIDTH, height))
    }

    fn header_rect(&self) -> Rect {
        Rect::from_min_size(self.position, Vec2::new(NODE_WIDTH, HEADER_HEIGHT))
    }

    fn row_y(&self, row: usize) -> f32 {
        self.position.y + HEADER_HEIGHT + ROW_HEIGHT * (row as f32 + 0.5) + 2.0
    }

    fn input_anchor(&self, index: usize) -> Pos2 {
        Pos2::new(self.position.x, self.row_y(index))
    }

    fn output_anchor(&self, index: usize) -> Pos2 {
        Pos2::new(self.position.x + NODE_WIDTH, self.row_y(index))
    }
}

/// (node, output port) -> (node, input port)
pub struct Wire {
    pub from: (usize, usize),
    pub to: (usize, usize),
}

pub struct Graph {
    pub nodes: Vec<Node>,
    pub wires: Vec<Wire>,
    dragging: Option<(usize, Vec2)>,
}

fn node(title: &str, family: Family, x: f32, y: f32, inputs: &[&str], outputs: &[(&str, Probe)]) -> Node {
    Node {
        title: title.to_string(),
        family,
        position: Pos2::new(x, y),
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
        outputs: outputs
            .iter()
            .map(|(name, probe)| OutputPort {
                name: name.to_string(),
                probe: *probe,
            })
            .collect(),
    }
}

impl Graph {
    /// The graph matching the demo song's hard-wired scene.
    pub fn demo() -> Graph {
        use Family::{Deputy, Geometry, Output, Source};
        use Probe::{Command, Off, Range};
        let nodes = vec![
            node(
                "wax stream",
                Source,
                40.0,
                60.0,
                &[],
                &[("20-2F", Range(0x20)), ("40-4F", Range(0x40)), ("60-6F", Range(0x60)), ("80-8F", Range(0x80))],
            ),
            node("camera", Deputy, 250.0, 40.0, &["20-2F"], &[("28 fov", Command(0x28)), ("29 orbit", Command(0x29)), ("2A radius", Command(0x2A))]),
            node("colour", Deputy, 250.0, 150.0, &["40-4F"], &[("41 hue", Command(0x41)), ("44 glow", Command(0x44))]),
            node("post", Deputy, 250.0, 240.0, &["60-6F"], &[("60 flash", Command(0x60))]),
            node("aux", Deputy, 250.0, 310.0, &["80-8F"], &[("80 pulse_ring", Command(0x80)), ("81 tunnel_twist", Command(0x81))]),
            node("orbit camera", Geometry, 470.0, 40.0, &["fov", "angle", "radius"], &[("camera", Off)]),
            node("core", Geometry, 470.0, 160.0, &["glow"], &[("sdf", Off)]),
            node("tunnel", Geometry, 470.0, 250.0, &["pulse", "twist"], &[("sdf", Off)]),
            node("union", Geometry, 680.0, 190.0, &["a", "b"], &[("sdf", Off)]),
            node("raymarch", Output, 880.0, 110.0, &["camera", "sdf", "hue", "flash"], &[]),
        ];
        let wire = |from: (usize, usize), to: (usize, usize)| Wire { from, to };
        let wires = vec![
            wire((0, 0), (1, 0)),
            wire((0, 1), (2, 0)),
            wire((0, 2), (3, 0)),
            wire((0, 3), (4, 0)),
            wire((1, 0), (5, 0)),
            wire((1, 1), (5, 1)),
            wire((1, 2), (5, 2)),
            wire((2, 1), (6, 0)),
            wire((4, 0), (7, 0)),
            wire((4, 1), (7, 1)),
            wire((6, 0), (8, 0)),
            wire((7, 0), (8, 1)),
            wire((5, 0), (9, 0)),
            wire((8, 0), (9, 1)),
            wire((2, 0), (9, 2)),
            wire((3, 0), (9, 3)),
        ];
        Graph {
            nodes,
            wires,
            dragging: None,
        }
    }

    /// Moves nodes dragged by their title bar. Returns true while a drag is active.
    pub fn handle_input(&mut self, ctx: &egui::Context) -> bool {
        let (pointer, pressed, down) = ctx.input(|i| (i.pointer.interact_pos(), i.pointer.primary_pressed(), i.pointer.primary_down()));
        if pressed {
            if let Some(pointer) = pointer {
                // Topmost (last drawn) node wins.
                self.dragging = self
                    .nodes
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, n)| n.header_rect().contains(pointer))
                    .map(|(index, n)| (index, n.position - pointer));
            }
        }
        if !down {
            self.dragging = None;
        }
        if let (Some((index, offset)), Some(pointer)) = (self.dragging, pointer) {
            self.nodes[index].position = pointer + offset;
        }
        self.dragging.is_some()
    }

    pub fn draw(&self, painter: &Painter, song: &Song, tick: f64) {
        for wire in &self.wires {
            self.draw_wire(painter, wire, song, tick);
        }
        for node in &self.nodes {
            draw_node(painter, node, song, tick);
        }
    }

    fn draw_wire(&self, painter: &Painter, wire: &Wire, song: &Song, tick: f64) {
        let from_node = &self.nodes[wire.from.0];
        let start = from_node.output_anchor(wire.from.1);
        let end = self.nodes[wire.to.0].input_anchor(wire.to.1);
        let activity = from_node
            .outputs
            .get(wire.from.1)
            .map(|port| activity(port.probe, song, tick))
            .unwrap_or(0.0);
        let colour = lerp_colour(style::LINE, style::ACCENT, activity);
        let bend = ((end.x - start.x).abs() * 0.5).max(30.0);
        let points = [start, start + Vec2::new(bend, 0.0), end - Vec2::new(bend, 0.0), end];
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            points,
            false,
            Color32::TRANSPARENT,
            Stroke::new(1.0, colour),
        ));
    }
}

fn draw_node(painter: &Painter, node: &Node, song: &Song, tick: f64) {
    let rect = node.rect();
    painter.rect_filled(rect, 3.0, style::GROUND);

    // Title with a family dot and a thin rule beneath.
    let title_y = node.position.y + HEADER_HEIGHT * 0.5;
    painter.circle_filled(Pos2::new(rect.left() + 10.0, title_y), 3.0, node.family.colour());
    painter.text(Pos2::new(rect.left() + 20.0, title_y), Align2::LEFT_CENTER, &node.title, style::title_font(), style::TEXT);
    let rule_y = node.position.y + HEADER_HEIGHT;
    painter.line_segment(
        [Pos2::new(rect.left() + 6.0, rule_y), Pos2::new(rect.right() - 6.0, rule_y)],
        Stroke::new(0.5, style::LINE),
    );

    // With a single port the name is implicit, so it is left out.
    let label_inputs = node.inputs.len() > 1;
    for (index, name) in node.inputs.iter().enumerate() {
        let anchor = node.input_anchor(index);
        painter.circle_filled(anchor, PORT_RADIUS, style::TEXT_DIM);
        if label_inputs {
            painter.text(anchor + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, name, style::port_font(), style::TEXT_DIM);
        }
    }

    for (index, port) in node.outputs.iter().enumerate() {
        let anchor = node.output_anchor(index);
        let level = activity(port.probe, song, tick);
        painter.circle_filled(anchor, PORT_RADIUS, lerp_colour(style::TEXT_DIM, style::ACCENT, level));
        if node.outputs.len() > 1 || node.inputs.is_empty() || matches!(port.probe, Probe::Command(_)) {
            painter.text(anchor - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER, &port.name, style::port_font(), style::TEXT);
        }
        if let Probe::Command(command) = port.probe {
            draw_value_readout(painter, node, index, command, song, tick);
        }
    }
}

/// A short bar and hex value at the left of a deputy's output row.
fn draw_value_readout(painter: &Painter, node: &Node, row: usize, command: u8, song: &Song, tick: f64) {
    let y = node.row_y(row);
    let left = node.position.x + 10.0;
    let (fraction, text) = match song.table.kind_of(command) {
        Kind::Trigger => {
            let pulse = song.timeline.pulse(command, tick, 12.0, 1.0);
            let raw = song.timeline.last_hit(command, tick).map(|(_, v)| v).unwrap_or(0);
            (pulse, format!("{raw:04X}"))
        }
        _ => {
            let value = song.timeline.value(command, tick);
            let raw = (value.clamp(0.0, 1.0) * 65535.0).round() as u16;
            (value.clamp(0.0, 1.0), format!("{raw:04X}"))
        }
    };
    painter.text(Pos2::new(left, y), Align2::LEFT_CENTER, text, style::mono(10.0), style::TEXT_DIM);
    let bar_left = left + 30.0;
    let bar_width = 22.0;
    painter.line_segment([Pos2::new(bar_left, y), Pos2::new(bar_left + bar_width, y)], Stroke::new(1.0, style::LINE));
    painter.line_segment(
        [Pos2::new(bar_left, y), Pos2::new(bar_left + bar_width * fraction, y)],
        Stroke::new(1.5, style::ACCENT),
    );
}

/// 0..1: how lively a port is right now (a glide in progress or a recent trigger).
fn activity(probe: Probe, song: &Song, tick: f64) -> f32 {
    match probe {
        Probe::Off => 0.0,
        Probe::Command(command) => command_activity(command, song, tick),
        Probe::Range(first) => song
            .timeline
            .commands()
            .into_iter()
            .filter(|c| command::group_of(*c) == first)
            .map(|c| command_activity(c, song, tick))
            .fold(0.0, f32::max),
    }
}

fn command_activity(command: u8, song: &Song, tick: f64) -> f32 {
    match song.table.kind_of(command) {
        Kind::Trigger => song.timeline.pulse(command, tick, 12.0, 1.0),
        _ => {
            let change = (song.timeline.value(command, tick) - song.timeline.value(command, tick - 3.0)).abs();
            (change * 60.0).min(1.0)
        }
    }
}

fn lerp_colour(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}
