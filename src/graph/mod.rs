//! The node graph: what a .reflux file describes and what gets evaluated each frame.

pub mod eval;
pub mod file;
pub mod nodes;

use std::collections::HashMap;

use nodes::{NodeType, SocketKind};

#[derive(Debug)]
pub struct Node {
    pub id: String,
    pub kind: NodeType,
    /// Values written in the file for inputs that have no wire (`radius=1,1.5,2`).
    pub constants: HashMap<String, Vec<f32>>,
    /// Non-numeric settings (`mode=wire`).
    pub options: HashMap<String, String>,
    /// Commands a `wax` node exposes, in output order.
    pub wax_commands: Vec<u8>,
    pub outputs: Vec<(String, SocketKind)>,
    /// Where the node sits in the graph view (`layout` lines); `None` until placed.
    pub position: Option<[f32; 2]>,
}

impl Node {
    pub fn output_index(&self, name: &str) -> Option<usize> {
        self.outputs.iter().position(|(n, _)| n == name)
    }

    pub fn input_index(&self, name: &str) -> Option<usize> {
        self.kind.inputs().iter().position(|i| i.name == name)
    }
}

/// A wire between two sockets, by node index and socket index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    pub from: (usize, usize),
    pub to: (usize, usize),
}

#[derive(Debug)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub wires: Vec<Wire>,
    /// Nodes in an order where every node comes after the nodes feeding it.
    pub order: Vec<usize>,
    /// (node, input) -> (node, output) for every wired input.
    incoming: HashMap<(usize, usize), (usize, usize)>,
}

impl Graph {
    pub fn empty() -> Graph {
        Graph {
            nodes: Vec::new(),
            wires: Vec::new(),
            order: Vec::new(),
            incoming: HashMap::new(),
        }
    }

    pub fn source_of(&self, node: usize, input: usize) -> Option<(usize, usize)> {
        self.incoming.get(&(node, input)).copied()
    }

    /// Finishes a graph whose nodes and wires are known: indexes the wires and
    /// sorts the nodes. Fails on a cycle or an input with two wires.
    fn build(nodes: Vec<Node>, wires: Vec<Wire>) -> Result<Graph, String> {
        let mut incoming = HashMap::new();
        for wire in &wires {
            if incoming.insert(wire.to, wire.from).is_some() {
                let node = &nodes[wire.to.0];
                let input = node.kind.inputs()[wire.to.1].name;
                return Err(format!("{}.{input} has more than one wire", node.id));
            }
        }
        let order = topological_order(nodes.len(), &wires, &nodes)?;
        Ok(Graph {
            nodes,
            wires,
            order,
            incoming,
        })
    }
}

fn topological_order(count: usize, wires: &[Wire], nodes: &[Node]) -> Result<Vec<usize>, String> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Visiting,
        Done,
    }
    let mut feeds: Vec<Vec<usize>> = vec![Vec::new(); count];
    for wire in wires {
        feeds[wire.to.0].push(wire.from.0);
    }
    let mut marks = vec![Mark::New; count];
    let mut order = Vec::with_capacity(count);

    fn visit(
        node: usize,
        feeds: &[Vec<usize>],
        marks: &mut [Mark],
        order: &mut Vec<usize>,
        nodes: &[Node],
    ) -> Result<(), String> {
        match marks[node] {
            Mark::Done => return Ok(()),
            Mark::Visiting => return Err(format!("the wires loop back through '{}'", nodes[node].id)),
            Mark::New => {}
        }
        marks[node] = Mark::Visiting;
        for &source in &feeds[node] {
            visit(source, feeds, marks, order, nodes)?;
        }
        marks[node] = Mark::Done;
        order.push(node);
        Ok(())
    }

    for node in 0..count {
        visit(node, &feeds, &mut marks, &mut order, nodes)?;
    }
    Ok(order)
}
