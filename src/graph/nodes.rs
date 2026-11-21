//! Every node type Reflux knows: its name in .reflux files, its inputs and outputs.

use crate::events::command::CommandTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketKind {
    /// A list of numbers (one per mesh, or per anything).
    Values,
    /// A list of meshes.
    Geo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Brings values in: numbers, time, Wax commands.
    Source,
    /// Reshapes values.
    Value,
    /// Makes meshes.
    Generator,
    /// Changes meshes.
    Operation,
    /// Ends of the graph: what gets drawn, where the camera is.
    Output,
}

pub struct InputDef {
    pub name: &'static str,
    pub kind: SocketKind,
    /// Used when the input has neither a wire nor a value in the file.
    pub default: f32,
}

const fn value(name: &'static str, default: f32) -> InputDef {
    InputDef {
        name,
        kind: SocketKind::Values,
        default,
    }
}

const fn geo(name: &'static str) -> InputDef {
    InputDef {
        name,
        kind: SocketKind::Geo,
        default: 0.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    Number,
    Series,
    Time,
    Wax,
    Map,
    Circle,
    Grid,
    Line,
    Arc,
    Sphere,
    Torus,
    Cylinder,
    Json,
    Obj,
    Move,
    Rotate,
    Scale,
    Array,
    Join,
    Select,
    Render,
    Camera,
}

pub const ALL_TYPES: [NodeType; 22] = [
    NodeType::Number,
    NodeType::Series,
    NodeType::Time,
    NodeType::Wax,
    NodeType::Map,
    NodeType::Circle,
    NodeType::Grid,
    NodeType::Line,
    NodeType::Arc,
    NodeType::Sphere,
    NodeType::Torus,
    NodeType::Cylinder,
    NodeType::Json,
    NodeType::Obj,
    NodeType::Move,
    NodeType::Rotate,
    NodeType::Scale,
    NodeType::Array,
    NodeType::Join,
    NodeType::Select,
    NodeType::Render,
    NodeType::Camera,
];

impl NodeType {
    pub fn name(self) -> &'static str {
        match self {
            NodeType::Number => "number",
            NodeType::Series => "series",
            NodeType::Time => "time",
            NodeType::Wax => "wax",
            NodeType::Map => "map",
            NodeType::Circle => "circle",
            NodeType::Grid => "grid",
            NodeType::Line => "line",
            NodeType::Arc => "arc",
            NodeType::Sphere => "sphere",
            NodeType::Torus => "torus",
            NodeType::Cylinder => "cylinder",
            NodeType::Json => "json",
            NodeType::Obj => "obj",
            NodeType::Move => "move",
            NodeType::Rotate => "rotate",
            NodeType::Scale => "scale",
            NodeType::Array => "array",
            NodeType::Join => "join",
            NodeType::Select => "select",
            NodeType::Render => "render",
            NodeType::Camera => "camera",
        }
    }

    pub fn from_name(name: &str) -> Option<NodeType> {
        ALL_TYPES.into_iter().find(|t| t.name() == name)
    }

    pub fn family(self) -> Family {
        use NodeType::*;
        match self {
            Number | Series | Time | Wax => Family::Source,
            Map => Family::Value,
            Circle | Grid | Line | Arc | Sphere | Torus | Cylinder | Json | Obj => Family::Generator,
            Move | Rotate | Scale | Array | Join | Select => Family::Operation,
            Render | Camera => Family::Output,
        }
    }

    pub fn inputs(self) -> &'static [InputDef] {
        use NodeType::*;
        match self {
            Time | Wax | Json | Obj => &[],
            Number => const { &[value("value", 0.0)] },
            Series => const { &[value("start", 0.0), value("step", 1.0), value("count", 4.0)] },
            Map => const {
                &[
                    value("value", 0.0),
                    value("from_min", 0.0),
                    value("from_max", 1.0),
                    value("to_min", 0.0),
                    value("to_max", 1.0),
                ]
            },
            Circle => const { &[value("radius", 1.0), value("segments", 24.0), value("fill", 0.0)] },
            Grid => const { &[value("width", 2.0), value("depth", 2.0), value("columns", 4.0), value("rows", 4.0)] },
            Line => const { &[value("length", 2.0), value("segments", 1.0)] },
            Arc => const { &[value("radius", 1.0), value("start", 0.0), value("end", 90.0), value("segments", 12.0)] },
            Sphere => const { &[value("radius", 1.0), value("rings", 8.0), value("segments", 16.0)] },
            Torus => const { &[value("major", 1.0), value("minor", 0.25), value("rings", 32.0), value("segments", 12.0)] },
            Cylinder => const { &[value("radius", 0.5), value("height", 1.0), value("segments", 16.0), value("caps", 1.0)] },
            Move => const { &[geo("geo"), value("x", 0.0), value("y", 0.0), value("z", 0.0)] },
            Rotate => const { &[geo("geo"), value("x", 0.0), value("y", 0.0), value("z", 0.0)] },
            Scale => const { &[geo("geo"), value("x", 1.0), value("y", 1.0), value("z", 1.0)] },
            Array => const { &[geo("shape"), geo("points")] },
            Join => const { &[geo("geo")] },
            Select => const { &[geo("geo"), value("index", 0.0)] },
            Render => const { &[geo("geo")] },
            Camera => const { &[value("angle", 0.0), value("radius", 3.0), value("height", 0.0), value("fov", 50.0)] },
        }
    }

    /// Options are non-numeric settings written as `key=word` in the file.
    pub fn options(self) -> &'static [&'static str] {
        match self {
            NodeType::Wax => &["commands"],
            NodeType::Render => &["mode"],
            NodeType::Json => &["file", "objects"],
            NodeType::Obj => &["file"],
            _ => &[],
        }
    }

    /// Outputs; for `wax` they depend on its `commands` option.
    pub fn outputs(self, wax_commands: &[u8], table: &CommandTable) -> Vec<(String, SocketKind)> {
        use NodeType::*;
        let one = |name: &str, kind| vec![(name.to_string(), kind)];
        match self {
            Number | Series | Map => one("out", SocketKind::Values),
            Time => vec![
                ("seconds".to_string(), SocketKind::Values),
                ("beat".to_string(), SocketKind::Values),
            ],
            Wax => wax_commands
                .iter()
                .map(|c| (table.name_of(*c), SocketKind::Values))
                .collect(),
            Circle | Grid | Line | Arc | Sphere | Torus | Cylinder | Json | Obj | Move | Rotate | Scale | Array
            | Join | Select => {
                one("geo", SocketKind::Geo)
            }
            Render | Camera => Vec::new(),
        }
    }
}
