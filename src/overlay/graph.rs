//! The graph overlay: draws the loaded graph with live values and lets nodes be
//! dragged by their title. Positions are saved to the .reflux file with Ctrl+S.

use std::collections::HashMap;

use egui::{Align2, Color32, Painter, Pos2, Rect, Stroke, Vec2};

use super::style;
use crate::events::command::Kind;
use crate::events::song::Song;
use crate::graph::eval::Evaluation;
use crate::graph::nodes::{Family, SocketKind};
use crate::graph::{Graph, Node};

const NODE_WIDTH: f32 = 170.0;
const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 17.0;
const PORT_RADIUS: f32 = 2.5;
const COLUMN_SPACING: f32 = 210.0;

fn family_colour(family: Family) -> Color32 {
    match family {
        Family::Source => style::SOURCE,
        Family::Value => style::DEPUTY,
        Family::Generator | Family::Operation => style::GEOMETRY,
        Family::Output => style::OUTPUT,
    }
}

fn rows(node: &Node) -> usize {
    node.kind.inputs().len().max(node.outputs.len()).max(1)
}

fn node_height(node: &Node) -> f32 {
    HEADER_HEIGHT + rows(node) as f32 * ROW_HEIGHT + 6.0
}

fn origin(node: &Node) -> Pos2 {
    let [x, y] = node.position.unwrap_or([0.0, 0.0]);
    Pos2::new(x, y)
}

fn row_y(node: &Node, row: usize) -> f32 {
    origin(node).y + HEADER_HEIGHT + ROW_HEIGHT * (row as f32 + 0.5) + 2.0
}

fn input_anchor(node: &Node, index: usize) -> Pos2 {
    Pos2::new(origin(node).x, row_y(node, index))
}

fn output_anchor(node: &Node, index: usize) -> Pos2 {
    Pos2::new(origin(node).x + NODE_WIDTH, row_y(node, index))
}

/// Places nodes that have no position yet in columns by how far they are from a source.
pub fn auto_layout(graph: &mut Graph) {
    let mut depth = vec![0usize; graph.nodes.len()];
    for &node in &graph.order {
        for wire in graph.wires.iter().filter(|w| w.to.0 == node) {
            depth[node] = depth[node].max(depth[wire.from.0] + 1);
        }
    }
    let mut column_bottom: HashMap<usize, f32> = HashMap::new();
    for &index in &graph.order {
        let node = &mut graph.nodes[index];
        if node.position.is_some() {
            continue;
        }
        let bottom = column_bottom.entry(depth[index]).or_insert(40.0);
        node.position = Some([40.0 + depth[index] as f32 * COLUMN_SPACING, *bottom]);
        *bottom += node_height(node) + 18.0;
    }
}

#[derive(Default)]
pub struct GraphView {
    dragging: Option<(usize, Vec2)>,
    /// Last frame's first value per output, to light up wires whose value is moving.
    previous: HashMap<(usize, usize), f32>,
}

impl GraphView {
    /// Moves nodes dragged by their title bar. Returns true when a drag just ended.
    pub fn handle_input(&mut self, ctx: &egui::Context, graph: &mut Graph) -> bool {
        let (pointer, pressed, down) = ctx.input(|i| {
            (
                i.pointer.interact_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
            )
        });
        if pressed {
            if let Some(pointer) = pointer {
                self.dragging = graph
                    .nodes
                    .iter()
                    .enumerate()
                    .rev() // the topmost (last drawn) node wins
                    .find(|(_, n)| Rect::from_min_size(origin(n), Vec2::new(NODE_WIDTH, HEADER_HEIGHT)).contains(pointer))
                    .map(|(index, n)| (index, origin(n) - pointer));
            }
        }
        let was_dragging = self.dragging.is_some();
        if !down {
            self.dragging = None;
        }
        if let (Some((index, offset)), Some(pointer)) = (self.dragging, pointer) {
            let p = pointer + offset;
            graph.nodes[index].position = Some([p.x, p.y]);
        }
        was_dragging && self.dragging.is_none()
    }

    pub fn draw(&mut self, painter: &Painter, graph: &Graph, evaluation: &Evaluation, song: &Song, tick: f64) {
        let mut current = HashMap::new();
        for wire in &graph.wires {
            let activity = self.wire_activity(graph, evaluation, song, tick, wire.from);
            draw_wire(painter, graph, wire.from, wire.to, activity);
        }
        for (index, node) in graph.nodes.iter().enumerate() {
            draw_node(painter, graph, index, node, evaluation);
            for output in 0..node.outputs.len() {
                if let Some(value) = evaluation.first_value(index, output) {
                    current.insert((index, output), value);
                }
            }
        }
        self.previous = current;
    }

    /// 0..1: how lively a wire's source is (a Wax trigger just fired, or the value moved).
    fn wire_activity(&self, graph: &Graph, evaluation: &Evaluation, song: &Song, tick: f64, from: (usize, usize)) -> f32 {
        let node = &graph.nodes[from.0];
        if let Some(&command) = node.wax_commands.get(from.1) {
            if song.table.kind_of(command) == Kind::Trigger {
                return song.timeline.pulse(command, tick, 12.0, 1.0);
            }
        }
        match (evaluation.first_value(from.0, from.1), self.previous.get(&from)) {
            (Some(now), Some(before)) => ((now - before).abs() * 40.0).min(1.0),
            _ => 0.0,
        }
    }
}

fn draw_wire(painter: &Painter, graph: &Graph, from: (usize, usize), to: (usize, usize), activity: f32) {
    let start = output_anchor(&graph.nodes[from.0], from.1);
    let end = input_anchor(&graph.nodes[to.0], to.1);
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

fn draw_node(painter: &Painter, graph: &Graph, index: usize, node: &Node, evaluation: &Evaluation) {
    let top_left = origin(node);
    let rect = Rect::from_min_size(top_left, Vec2::new(NODE_WIDTH, node_height(node)));
    painter.rect_filled(rect, 3.0, style::GROUND);

    // Title with a family dot and a thin rule beneath.
    let title_y = top_left.y + HEADER_HEIGHT * 0.5;
    painter.circle_filled(Pos2::new(rect.left() + 10.0, title_y), 3.0, family_colour(node.kind.family()));
    let title = if node.id == node.kind.name() {
        node.id.clone()
    } else {
        format!("{} · {}", node.id, node.kind.name())
    };
    painter.text(Pos2::new(rect.left() + 20.0, title_y), Align2::LEFT_CENTER, title, style::title_font(), style::TEXT);
    let rule_y = top_left.y + HEADER_HEIGHT;
    painter.line_segment(
        [Pos2::new(rect.left() + 6.0, rule_y), Pos2::new(rect.right() - 6.0, rule_y)],
        Stroke::new(0.5, style::LINE),
    );

    let inputs = node.kind.inputs();
    for (i, input) in inputs.iter().enumerate() {
        let anchor = input_anchor(node, i);
        let wired = graph.source_of(index, i).is_some();
        painter.circle_filled(anchor, PORT_RADIUS, if wired { style::TEXT } else { style::TEXT_DIM });
        // A single input's name is implicit, so it's left out.
        if inputs.len() > 1 {
            painter.text(anchor + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, input.name, style::port_font(), style::TEXT_DIM);
        }
        if !wired && input.kind == SocketKind::Values {
            let text = match node.constants.get(input.name) {
                Some(values) => format_list(values),
                None => format_number(input.default),
            };
            painter.text(
                Pos2::new(rect.left() + NODE_WIDTH * 0.55, anchor.y),
                Align2::LEFT_CENTER,
                text,
                style::mono(10.0),
                style::TEXT_DIM,
            );
        }
    }

    for (o, (name, kind)) in node.outputs.iter().enumerate() {
        let anchor = output_anchor(node, o);
        painter.circle_filled(anchor, PORT_RADIUS, style::TEXT);
        let readout = match kind {
            SocketKind::Values => evaluation.first_value(index, o).map(format_number),
            SocketKind::Geo => evaluation.mesh_count(index, o).map(|n| format!("×{n}")),
        };
        let label = match readout {
            Some(readout) if node.outputs.len() > 1 || inputs.is_empty() => format!("{readout}  {name}"),
            Some(readout) => readout,
            None => name.clone(),
        };
        painter.text(anchor - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER, label, style::mono(10.0), style::TEXT);
    }

    if let Some(mode) = node.options.get("mode") {
        painter.text(
            Pos2::new(rect.right() - 8.0, title_y),
            Align2::RIGHT_CENTER,
            mode,
            style::port_font(),
            style::TEXT_DIM,
        );
    }
}

fn format_number(value: f32) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_string() } else { text.to_string() }
}

fn format_list(values: &[f32]) -> String {
    let text: Vec<String> = values.iter().take(4).map(|v| format_number(*v)).collect();
    let mut text = text.join(",");
    if values.len() > 4 {
        text.push('…');
    }
    text
}

fn lerp_colour(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}
