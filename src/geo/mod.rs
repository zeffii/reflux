//! Procedural geometry.
//!
//! A geo wire carries a *list* of meshes (`Geo`). Generators are vectorised: give a
//! circle node three radii and it outputs three circles on one wire. Operations act
//! on every mesh in the list, taking the i-th value of each parameter list for the
//! i-th mesh (the last value repeats when a list is shorter).

pub mod generators;
pub mod ops;

/// One mesh: vertices, edges and polygon faces (any number of corners).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub verts: Vec<[f32; 3]>,
    pub edges: Vec<[u32; 2]>,
    pub faces: Vec<Vec<u32>>,
    /// Position in the list it was generated in; kept through operations, so a
    /// command can address "mesh 3 of the array".
    pub id: u32,
}

/// What a geo wire carries.
pub type Geo = Vec<Mesh>;

/// The i-th value of a parameter list, repeating the last value past the end.
pub fn nth(values: &[f32], i: usize) -> f32 {
    match values.len() {
        0 => 0.0,
        n => values[i.min(n - 1)],
    }
}

impl Mesh {
    /// Unique boundary edges of all faces, for meshes built from faces only.
    pub fn edges_from_faces(faces: &[Vec<u32>]) -> Vec<[u32; 2]> {
        let mut edges: Vec<[u32; 2]> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for face in faces {
            for i in 0..face.len() {
                let a = face[i];
                let b = face[(i + 1) % face.len()];
                let key = (a.min(b), a.max(b));
                if seen.insert(key) {
                    edges.push([a, b]);
                }
            }
        }
        edges
    }

    /// Faces as triangles (fan), with one flat normal per triangle corner.
    pub fn triangles(&self) -> Vec<([f32; 3], [f32; 3])> {
        let mut out = Vec::new();
        for face in &self.faces {
            if face.len() < 3 {
                continue;
            }
            let normal = face_normal(&self.verts, face);
            for i in 1..face.len() - 1 {
                for index in [face[0], face[i], face[i + 1]] {
                    out.push((self.verts[index as usize], normal));
                }
            }
        }
        out
    }

    /// Edges as pairs of positions.
    pub fn lines(&self) -> Vec<[f32; 3]> {
        self.edges
            .iter()
            .flat_map(|[a, b]| [self.verts[*a as usize], self.verts[*b as usize]])
            .collect()
    }
}

/// Newell's method: robust for non-planar and concave polygons.
fn face_normal(verts: &[[f32; 3]], face: &[u32]) -> [f32; 3] {
    let mut n = [0.0f32; 3];
    for i in 0..face.len() {
        let a = verts[face[i] as usize];
        let b = verts[face[(i + 1) % face.len()] as usize];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length < 1e-12 {
        return [0.0, 1.0, 0.0];
    }
    [n[0] / length, n[1] / length, n[2] / length]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_repeats_the_last_value() {
        assert_eq!(nth(&[1.0, 2.0], 0), 1.0);
        assert_eq!(nth(&[1.0, 2.0], 5), 2.0);
        assert_eq!(nth(&[], 3), 0.0);
    }

    #[test]
    fn quad_triangulates_into_two_triangles_with_shared_edges_once() {
        let faces = vec![vec![0, 1, 2, 3]];
        let mesh = Mesh {
            verts: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            edges: Mesh::edges_from_faces(&faces),
            faces,
            id: 0,
        };
        assert_eq!(mesh.edges.len(), 4);
        let triangles = mesh.triangles();
        assert_eq!(triangles.len(), 6);
        assert!(triangles[0].1[1].abs() > 0.99); // normal along y
    }
}
