//! Running the graph for one moment of the song.

use super::nodes::{NodeType, SocketKind};
use super::Graph;
use crate::events::command::Kind;
use crate::events::song::Song;
use crate::geo::{generators, nth, ops, Geo};

#[derive(Clone, Debug)]
pub enum Data {
    Values(Vec<f32>),
    Geo(Geo),
}

/// Where a `camera` node puts the camera. Angle is in turns, like the orbit command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraParams {
    pub angle: f32,
    pub radius: f32,
    pub height: f32,
    pub fov: f32,
}

#[derive(Default)]
pub struct Evaluation {
    /// Meshes drawn with faces (render nodes in `solid` or `both` mode).
    pub solid: Geo,
    /// Meshes drawn as edges (render nodes in `wire` or `both` mode).
    pub wire: Geo,
    pub camera: Option<CameraParams>,
    /// Every node's outputs, by node index then output index (for the graph view).
    pub outputs: Vec<Vec<Data>>,
}

impl Evaluation {
    /// The first number on an output, for live readouts.
    pub fn first_value(&self, node: usize, output: usize) -> Option<f32> {
        match self.outputs.get(node)?.get(output)? {
            Data::Values(values) => values.first().copied(),
            Data::Geo(_) => None,
        }
    }

    pub fn mesh_count(&self, node: usize, output: usize) -> Option<usize> {
        match self.outputs.get(node)?.get(output)? {
            Data::Geo(geo) => Some(geo.len()),
            Data::Values(_) => None,
        }
    }
}

pub fn evaluate(graph: &Graph, song: &Song, seconds: f64) -> Evaluation {
    let mut evaluation = Evaluation {
        outputs: vec![Vec::new(); graph.nodes.len()],
        ..Default::default()
    };
    let tick = song.tick_at(seconds);

    for &index in &graph.order {
        let node = &graph.nodes[index];
        let inputs = Inputs {
            graph,
            node: index,
            outputs: &evaluation.outputs,
        };
        let v = |name: &str| inputs.values(name);
        let g = |name: &str| inputs.geo(name);

        use NodeType::*;
        let produced: Vec<Data> = match node.kind {
            Number => vec![Data::Values(v("value"))],
            Series => {
                let (start, step) = (nth(&v("start"), 0), nth(&v("step"), 0));
                let count = (nth(&v("count"), 0).round() as i64).clamp(0, 10_000) as usize;
                vec![Data::Values((0..count).map(|i| start + step * i as f32).collect())]
            }
            Time => {
                let looped = seconds.rem_euclid(song.length_seconds());
                vec![
                    Data::Values(vec![looped as f32]),
                    Data::Values(vec![song.timing.beats_at(looped) as f32]),
                ]
            }
            Wax => node
                .wax_commands
                .iter()
                .map(|&command| Data::Values(vec![wax_value(song, command, tick)]))
                .collect(),
            Map => {
                let (value, a0, a1, b0, b1) = (v("value"), v("from_min"), v("from_max"), v("to_min"), v("to_max"));
                let count = [&value, &a0, &a1, &b0, &b1].iter().map(|l| l.len()).max().unwrap_or(1);
                let mapped = (0..count)
                    .map(|i| {
                        let span = nth(&a1, i) - nth(&a0, i);
                        let t = if span.abs() < 1e-12 { 0.0 } else { (nth(&value, i) - nth(&a0, i)) / span };
                        nth(&b0, i) + t * (nth(&b1, i) - nth(&b0, i))
                    })
                    .collect();
                vec![Data::Values(mapped)]
            }
            Circle => vec![Data::Geo(generators::circle(&v("radius"), &v("segments"), &v("fill")))],
            Grid => vec![Data::Geo(generators::grid(&v("width"), &v("depth"), &v("columns"), &v("rows")))],
            Line => vec![Data::Geo(generators::line(&v("length"), &v("segments")))],
            Arc => vec![Data::Geo(generators::arc(&v("radius"), &v("start"), &v("end"), &v("segments")))],
            Sphere => vec![Data::Geo(generators::sphere(&v("radius"), &v("rings"), &v("segments")))],
            Torus => vec![Data::Geo(generators::torus(&v("major"), &v("minor"), &v("rings"), &v("segments")))],
            Cylinder => vec![Data::Geo(generators::cylinder(&v("radius"), &v("height"), &v("segments"), &v("caps")))],
            Move => vec![Data::Geo(ops::translate(&g("geo"), &v("x"), &v("y"), &v("z")))],
            Rotate => vec![Data::Geo(ops::rotate(&g("geo"), &v("x"), &v("y"), &v("z")))],
            Scale => vec![Data::Geo(ops::scale(&g("geo"), &v("x"), &v("y"), &v("z")))],
            Array => vec![Data::Geo(ops::array_on_verts(&g("shape"), &g("points")))],
            Join => vec![Data::Geo(ops::join(&g("geo")))],
            Select => vec![Data::Geo(ops::select(&g("geo"), &v("index")))],
            Render => {
                let geo = g("geo");
                let mode = node.options.get("mode").map(String::as_str).unwrap_or("solid");
                if mode == "solid" || mode == "both" {
                    evaluation.solid.extend(geo.iter().cloned());
                }
                if mode == "wire" || mode == "both" {
                    evaluation.wire.extend(geo);
                }
                Vec::new()
            }
            Camera => {
                evaluation.camera = Some(CameraParams {
                    angle: nth(&v("angle"), 0),
                    radius: nth(&v("radius"), 0),
                    height: nth(&v("height"), 0),
                    fov: nth(&v("fov"), 0),
                });
                Vec::new()
            }
        };
        evaluation.outputs[index] = produced;
    }
    evaluation
}

/// A Wax command as a number: targets give their current value (0..1), triggers a
/// pulse that jumps to the hit's velocity (low byte) and decays.
fn wax_value(song: &Song, command: u8, tick: f64) -> f32 {
    match song.table.kind_of(command) {
        Kind::Trigger => {
            let velocity = song
                .timeline
                .last_hit(command, tick)
                .map(|(_, value)| (value & 0xFF) as f32 / 255.0)
                .unwrap_or(0.0);
            song.timeline.pulse(command, tick, 10.0, velocity)
        }
        _ => song.timeline.value(command, tick),
    }
}

/// Reads a node's inputs: from a wire if there is one, else the file's value, else the default.
struct Inputs<'a> {
    graph: &'a Graph,
    node: usize,
    outputs: &'a [Vec<Data>],
}

impl Inputs<'_> {
    fn wired(&self, name: &str) -> Option<&Data> {
        let input = self.graph.nodes[self.node].input_index(name)?;
        let (source, output) = self.graph.source_of(self.node, input)?;
        self.outputs.get(source)?.get(output)
    }

    fn values(&self, name: &str) -> Vec<f32> {
        if let Some(Data::Values(values)) = self.wired(name) {
            if !values.is_empty() {
                return values.clone();
            }
        }
        let node = &self.graph.nodes[self.node];
        if let Some(values) = node.constants.get(name) {
            return values.clone();
        }
        let default = node
            .kind
            .inputs()
            .iter()
            .find(|i| i.name == name && i.kind == SocketKind::Values)
            .map(|i| i.default)
            .unwrap_or(0.0);
        vec![default]
    }

    fn geo(&self, name: &str) -> Geo {
        match self.wired(name) {
            Some(Data::Geo(geo)) => geo.clone(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::file;

    #[test]
    fn vectorised_generator_through_ops_to_render() {
        let text = "\
node radii series start=1 step=1 count=3
node rings circle segments=8
node up move y=0,1,2
node out render mode=both
node cam camera angle=0.25 radius=4
wire radii.out -> rings.radius
wire rings.geo -> up.geo
wire up.geo -> out.geo
";
        let song = Song::demo();
        let graph = file::parse(text, &song.table).unwrap();
        let result = evaluate(&graph, &song, 0.0);
        assert_eq!(result.wire.len(), 3);
        assert!(result.solid.iter().all(|m| m.faces.is_empty())); // circles without fill
        assert!((result.wire[2].verts[0][1] - 2.0).abs() < 1e-6);
        assert!((result.wire[2].verts[0][0] - 3.0).abs() < 1e-6);
        assert_eq!(result.camera.unwrap().radius, 4.0);
    }

    #[test]
    fn wax_and_map_follow_the_song() {
        let text = "\
node w wax commands=29
node m map to_min=10 to_max=20
wire w.orbit -> m.value
";
        let song = Song::demo();
        let graph = file::parse(text, &song.table).unwrap();
        // In the demo song orbit glides from 0 towards 0x4000 starting at row 4.
        let early = evaluate(&graph, &song, 0.1);
        let later = evaluate(&graph, &song, 6.0);
        assert_eq!(early.first_value(1, 0), Some(10.0));
        assert!(later.first_value(1, 0).unwrap() > 10.5);
    }
}
