//! The .reflux text format.
//!
//! ```text
//! reflux 1
//! # comments start with #
//! node rings torus major=1.7,2,2.3 minor=0.02
//! node spin  rotate y=30
//! wire rings.geo -> spin.geo
//! layout rings 300 120
//! ```
//!
//! `node <id> <type> [input=value,value,...] [option=word]`, `wire <node>.<output> ->
//! <node>.<input>`, `layout <node> <x> <y>`. A wire overrides an input's value.

use std::collections::HashMap;

use super::nodes::{NodeType, SocketKind};
use super::{Graph, Node, Wire};
use crate::events::command::CommandTable;

pub fn parse(text: &str, table: &CommandTable) -> Result<Graph, String> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut wire_lines: Vec<(usize, String, String)> = Vec::new();
    let mut layouts: Vec<(usize, String, [f32; 2])> = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let fail = |message: String| format!("line {line_number}: {message}");
        let line = raw.split('#').next().unwrap_or("").trim();
        let owned = split_words(line).map_err(fail)?;
        let words: Vec<&str> = owned.iter().map(String::as_str).collect();
        let Some(&keyword) = words.first() else {
            continue;
        };
        match keyword {
            "reflux" => {}
            "node" => {
                let node = parse_node(&words, table).map_err(fail)?;
                if nodes.iter().any(|n| n.id == node.id) {
                    return Err(fail(format!("there is already a node called '{}'", node.id)));
                }
                nodes.push(node);
            }
            "wire" => {
                if words.len() != 4 || words[2] != "->" {
                    return Err(fail("expected: wire node.output -> node.input".to_string()));
                }
                wire_lines.push((line_number, words[1].to_string(), words[3].to_string()));
            }
            "layout" => {
                let position = match words.as_slice() {
                    [_, _, x, y] => x.parse().ok().zip(y.parse().ok()).map(|(x, y)| [x, y]),
                    _ => None,
                };
                let position = position.ok_or_else(|| fail("expected: layout node x y".to_string()))?;
                layouts.push((line_number, words[1].to_string(), position));
            }
            other => return Err(fail(format!("unknown line type '{other}'"))),
        }
    }

    let find = |id: &str, line: usize, nodes: &[Node]| {
        nodes
            .iter()
            .position(|n| n.id == id)
            .ok_or_else(|| format!("line {line}: no node called '{id}'"))
    };

    let mut wires = Vec::new();
    for (line, from, to) in &wire_lines {
        let line = *line;
        let (from_id, output) = from
            .split_once('.')
            .ok_or_else(|| format!("line {line}: '{from}' should be node.output"))?;
        let (to_id, input) = to
            .split_once('.')
            .ok_or_else(|| format!("line {line}: '{to}' should be node.input"))?;
        let from_node = find(from_id, line, &nodes)?;
        let to_node = find(to_id, line, &nodes)?;
        let output_index = nodes[from_node]
            .output_index(output)
            .ok_or_else(|| format!("line {line}: '{from_id}' has no output '{output}'"))?;
        let input_index = nodes[to_node]
            .input_index(input)
            .ok_or_else(|| format!("line {line}: '{to_id}' has no input '{input}'"))?;
        let output_kind = nodes[from_node].outputs[output_index].1;
        let input_kind = nodes[to_node].kind.inputs()[input_index].kind;
        if output_kind != input_kind {
            let describe = |k: SocketKind| if k == SocketKind::Geo { "geo" } else { "values" };
            return Err(format!(
                "line {line}: {from} carries {} but {to} takes {}",
                describe(output_kind),
                describe(input_kind)
            ));
        }
        wires.push(Wire {
            from: (from_node, output_index),
            to: (to_node, input_index),
        });
    }

    for (line, id, position) in layouts {
        let index = find(&id, line, &nodes)?;
        nodes[index].position = Some(position);
    }

    Graph::build(nodes, wires)
}

/// Splits on whitespace, keeping double-quoted parts together (`file="my model.obj"`).
fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if quoted {
        return Err("a quote is not closed".to_string());
    }
    if !current.is_empty() {
        words.push(current);
    }
    Ok(words)
}

fn parse_node(words: &[&str], table: &CommandTable) -> Result<Node, String> {
    let (id, type_name) = match words {
        [_, id, type_name, ..] => (*id, *type_name),
        _ => return Err("expected: node id type [input=value ...]".to_string()),
    };
    if id.contains('.') {
        return Err(format!("node names can't contain '.': {id}"));
    }
    let kind = NodeType::from_name(type_name).ok_or_else(|| format!("unknown node type '{type_name}'"))?;

    let mut constants = HashMap::new();
    let mut options = HashMap::new();
    for setting in &words[3..] {
        let (key, raw) = setting
            .split_once('=')
            .ok_or_else(|| format!("'{setting}' should be name=value"))?;
        if kind.options().contains(&key) {
            options.insert(key.to_string(), raw.to_string());
        } else if kind.inputs().iter().any(|i| i.name == key) {
            let values = raw
                .split(',')
                .map(|v| v.parse::<f32>().map_err(|_| format!("'{v}' in {key} is not a number")))
                .collect::<Result<Vec<f32>, String>>()?;
            constants.insert(key.to_string(), values);
        } else {
            return Err(format!("a {type_name} node has no setting '{key}'"));
        }
    }

    let wax_commands = match options.get("commands") {
        Some(list) => list
            .split(',')
            .map(|c| u8::from_str_radix(c, 16).map_err(|_| format!("'{c}' is not a hex command")))
            .collect::<Result<Vec<u8>, String>>()?,
        None => Vec::new(),
    };

    Ok(Node {
        id: id.to_string(),
        kind,
        outputs: kind.outputs(&wax_commands, table),
        constants,
        options,
        wax_commands,
        position: None,
        loaded: Vec::new(),
    })
}

/// Rewrites only the `layout` lines of a .reflux text with the graph's node positions,
/// keeping everything else (comments included) as it was. Missing ones are appended.
pub fn with_layout(text: &str, graph: &Graph) -> String {
    let position_line = |node: &Node| {
        node.position
            .map(|[x, y]| format!("layout {} {} {}", node.id, x.round(), y.round()))
    };
    let mut written = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() == Some(&"layout") && words.len() >= 2 {
            if let Some(node) = graph.nodes.iter().find(|n| n.id == words[1]) {
                if let Some(new_line) = position_line(node) {
                    out.push(new_line);
                    written.insert(node.id.clone());
                    continue;
                }
            }
        }
        out.push(line.to_string());
    }
    let missing: Vec<String> = graph
        .nodes
        .iter()
        .filter(|n| !written.contains(&n.id))
        .filter_map(position_line)
        .collect();
    if !missing.is_empty() {
        if out.last().is_some_and(|l| !l.trim().is_empty()) {
            out.push(String::new());
        }
        out.extend(missing);
    }
    let mut result = out.join("\n");
    result.push('\n');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "\
reflux 1
# three rings, spun
node radii series start=1 step=0.5 count=3
node rings torus minor=0.02
node spin rotate y=30   # degrees
wire radii.out -> rings.major
wire rings.geo -> spin.geo
layout spin 400 100
";

    #[test]
    fn parses_nodes_wires_and_layout() {
        let graph = parse(TEXT, &CommandTable::standard()).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.wires.len(), 2);
        assert_eq!(graph.nodes[1].constants["minor"], vec![0.02]);
        assert_eq!(graph.nodes[2].position, Some([400.0, 100.0]));
        // Sources come before the nodes they feed.
        let place = |id| graph.order.iter().position(|&n| graph.nodes[n].id == id).unwrap();
        assert!(place("radii") < place("rings") && place("rings") < place("spin"));
    }

    #[test]
    fn mistakes_are_reported_with_line_numbers() {
        let table = CommandTable::standard();
        let err = |t: &str| parse(t, &table).unwrap_err();
        assert!(err("node a blob").contains("unknown node type"));
        assert!(err("node a torus size=2").contains("no setting 'size'"));
        assert!(err("node a torus\nwire a.geo -> b.geo").starts_with("line 2"));
        assert!(err("node a torus\nnode b torus\nwire a.geo -> b.major").contains("carries geo"));
        assert!(err("node a move\nnode b move\nwire a.geo -> b.geo\nwire b.geo -> a.geo").contains("loop"));
    }

    #[test]
    fn quoted_values_keep_their_spaces() {
        let graph = parse(r#"node m obj file="my models/thing.obj""#, &CommandTable::standard()).unwrap();
        assert_eq!(graph.nodes[0].options["file"], "my models/thing.obj");
        assert!(parse(r#"node m obj file="open"#, &CommandTable::standard()).is_err());
    }

    #[test]
    fn wax_nodes_get_one_output_per_command() {
        let graph = parse("node w wax commands=29,60", &CommandTable::standard()).unwrap();
        let names: Vec<&str> = graph.nodes[0].outputs.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["orbit", "flash"]);
    }

    #[test]
    fn saving_layout_keeps_comments_and_other_lines() {
        let mut graph = parse(TEXT, &CommandTable::standard()).unwrap();
        graph.nodes[2].position = Some([420.0, 90.0]);
        graph.nodes[0].position = Some([10.0, 20.0]);
        let saved = with_layout(TEXT, &graph);
        assert!(saved.contains("# three rings, spun"));
        assert!(saved.contains("layout spin 420 90"));
        assert!(saved.ends_with("layout radii 10 20\n"));
        assert!(parse(&saved, &CommandTable::standard()).is_ok());
    }
}
