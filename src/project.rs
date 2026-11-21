//! The .reflux file being worked on: loading, hot reloading and saving the layout.
//!
//! The file is the meeting point for hand edits (yours or mine) and the graph view:
//! saving the text in an editor reloads the graph within half a second, and Ctrl+S
//! in Reflux writes back only the node positions.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::events::command::CommandTable;
use crate::graph::{file, Graph};

pub const DEFAULT_SCENE: &str = "scenes/demo.reflux";
const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct Project {
    pub path: Option<PathBuf>,
    pub graph: Graph,
    /// The last problem reading the file; the previous good graph stays in use meanwhile.
    pub error: Option<String>,
    text: String,
    modified: Option<SystemTime>,
    last_poll: Instant,
}

impl Project {
    pub fn empty() -> Project {
        Project {
            path: None,
            graph: Graph::empty(),
            error: None,
            text: String::new(),
            modified: None,
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
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if modified == self.modified {
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
                self.graph = graph;
                self.text = text;
                self.error = None;
                true
            }
            Err(message) => {
                self.error = Some(message);
                false
            }
        }
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
