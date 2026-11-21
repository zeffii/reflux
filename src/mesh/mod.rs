//! Imported geometry.

pub mod obj;

use std::path::Path;

/// A triangle soup: every three entries form one triangle, with a normal per corner.
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
}

/// A mesh plus the name it was loaded under, as shown in the graph.
pub struct LoadedMesh {
    pub name: String,
    pub data: MeshData,
}

impl MeshData {
    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Centre of the bounding box and the distance from it to the furthest vertex.
    pub fn bounds(&self) -> ([f32; 3], f32) {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &self.positions {
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        let centre = [0, 1, 2].map(|axis| (min[axis] + max[axis]) * 0.5);
        let radius = self
            .positions
            .iter()
            .map(|p| {
                let d = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
            })
            .fold(0.0, f32::max);
        (centre, radius)
    }
}

pub fn load_obj(path: &Path) -> Result<LoadedMesh, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let data = obj::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mesh".to_string());
    Ok(LoadedMesh { name, data })
}
