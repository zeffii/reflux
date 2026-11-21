//! The .reflux file being worked on: loading, hot reloading and saving the layout.
//!
//! The file is the meeting point for hand edits (yours or mine) and the graph view:
//! saving the text in an editor reloads the graph within half a second, and Ctrl+S
//! in Reflux writes back only the node positions.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::events::command::CommandTable;
use crate::geo::{self, Geo, Mesh};
use crate::graph::nodes::NodeType;
use crate::graph::{file, Graph};
use crate::mesh::obj;

pub const DEFAULT_SCENE: &str = "scenes/demo.reflux";
const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct Project {
    pub path: Option<PathBuf>,
    pub graph: Graph,
    /// The last problem reading the file; the previous good graph stays in use meanwhile.
    pub error: Option<String>,
    text: String,
    modified: Option<SystemTime>,
    /// Geometry files the graph reads, with their modification times, so editing
    /// one of them reloads too.
    assets: Vec<(PathBuf, Option<SystemTime>)>,
    last_poll: Instant,
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Project {
    pub fn empty() -> Project {
        Project {
            path: None,
            graph: Graph::empty(),
            error: None,
            text: String::new(),
            modified: None,
            assets: Vec::new(),
            last_poll: Instant::now(),
        }
    }

    pub fn open(path: &Path, table: &CommandTable) -> Project {
        let mut project = Project::empty();
        project.path = Some(path.to_path_buf());
        project.reload(table);
        project
    }

    /// The file name shown in the status line.
    pub fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "no scene".to_string())
    }

    /// Reloads when the file changed on disk. Returns true when a new graph was loaded.
    pub fn poll(&mut self, table: &CommandTable) -> bool {
        if self.last_poll.elapsed() < POLL_INTERVAL {
            return false;
        }
        self.last_poll = Instant::now();
        let Some(path) = &self.path else {
            return false;
        };
        let scene_changed = modified_time(path) != self.modified;
        let asset_changed = self.assets.iter().any(|(asset, time)| modified_time(asset) != *time);
        if !scene_changed && !asset_changed {
            return false;
        }
        self.reload(table)
    }

    fn reload(&mut self, table: &CommandTable) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        self.modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.error = Some(format!("{}: {e}", path.display()));
                return false;
            }
        };
        match file::parse(&text, table) {
            Ok(mut graph) => {
                // Nodes the file doesn't place keep the spot they had before the reload.
                for node in &mut graph.nodes {
                    if node.position.is_none() {
                        if let Some(old) = self.graph.nodes.iter().find(|o| o.id == node.id) {
                            node.position = old.position;
                        }
                    }
                }
                self.error = self.load_assets(&mut graph).err();
                self.graph = graph;
                self.text = text;
                true
            }
            Err(message) => {
                self.error = Some(message);
                false
            }
        }
    }

    /// Reads the files of `json` and `obj` nodes, relative to the scene file. A file
    /// that fails leaves its node empty; the first problem is returned.
    fn load_assets(&mut self, graph: &mut Graph) -> Result<(), String> {
        let folder = self
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .unwrap_or_default();
        self.assets.clear();
        let mut first_error = None;
        for node in &mut graph.nodes {
            if !matches!(node.kind, NodeType::Json | NodeType::Obj) {
                continue;
            }
            let Some(file_name) = node.options.get("file") else {
                first_error.get_or_insert(format!("'{}' needs file=...", node.id));
                continue;
            };
            let path = folder.join(file_name);
            self.assets.push((path.clone(), modified_time(&path)));
            let only: Vec<String> = node
                .options
                .get("objects")
                .map(|list| list.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            match read_geometry(&path, node.kind, &only) {
                Ok(geo) => node.loaded = geo,
                Err(message) => {
                    node.loaded = Vec::new();
                    first_error.get_or_insert(format!("'{}': {}: {message}", node.id, path.display()));
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Writes the current node positions into the file's `layout` lines.
    pub fn save_layout(&mut self) -> Result<(), String> {
        let path = self.path.as_ref().ok_or("no scene file is open")?;
        let text = file::with_layout(&self.text, &self.graph);
        std::fs::write(path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.text = text;
        self.modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        Ok(())
    }
}

fn read_geometry(path: &Path, kind: NodeType, only: &[String]) -> Result<Geo, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    match kind {
        NodeType::Json => geo::json::parse_geo(&text, only),
        _ => {
            let (verts, faces) = obj::parse_polygons(&text).map_err(|e| e.to_string())?;
            Ok(vec![Mesh {
                edges: Mesh::edges_from_faces(&faces),
                verts,
                faces,
                id: 0,
            }])
        }
    }
}
