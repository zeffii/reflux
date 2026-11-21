//! Operations on geo lists. Parameters are per-mesh lists (the last value repeats).

use glam::{EulerRot, Quat, Vec3};

use super::{nth, Geo, Mesh};

fn map_verts(geo: &Geo, mut f: impl FnMut(usize, Vec3) -> Vec3) -> Geo {
    geo.iter()
        .enumerate()
        .map(|(i, mesh)| Mesh {
            verts: mesh.verts.iter().map(|v| f(i, Vec3::from(*v)).to_array()).collect(),
            ..mesh.clone()
        })
        .collect()
}

pub fn translate(geo: &Geo, x: &[f32], y: &[f32], z: &[f32]) -> Geo {
    map_verts(geo, |i, v| v + Vec3::new(nth(x, i), nth(y, i), nth(z, i)))
}

/// Rotation in degrees about X, then Y, then Z, around the origin.
pub fn rotate(geo: &Geo, x: &[f32], y: &[f32], z: &[f32]) -> Geo {
    map_verts(geo, |i, v| {
        let q = Quat::from_euler(
            EulerRot::ZYX,
            nth(z, i).to_radians(),
            nth(y, i).to_radians(),
            nth(x, i).to_radians(),
        );
        q * v
    })
}

pub fn scale(geo: &Geo, x: &[f32], y: &[f32], z: &[f32]) -> Geo {
    map_verts(geo, |i, v| v * Vec3::new(nth(x, i), nth(y, i), nth(z, i)))
}

/// A copy of every shape mesh at every vertex of every points mesh, as separate meshes.
pub fn array_on_verts(shape: &Geo, points: &Geo) -> Geo {
    let mut out = Vec::new();
    for point in points.iter().flat_map(|mesh| mesh.verts.iter()) {
        let offset = Vec3::from(*point);
        for mesh in shape {
            out.push(Mesh {
                verts: mesh.verts.iter().map(|v| (Vec3::from(*v) + offset).to_array()).collect(),
                id: out.len() as u32,
                ..mesh.clone()
            });
        }
    }
    out
}

/// All meshes merged into one.
pub fn join(geo: &Geo) -> Geo {
    let mut joined = Mesh::default();
    for mesh in geo {
        let base = joined.verts.len() as u32;
        joined.verts.extend_from_slice(&mesh.verts);
        joined.edges.extend(mesh.edges.iter().map(|[a, b]| [a + base, b + base]));
        joined.faces.extend(mesh.faces.iter().map(|f| f.iter().map(|i| i + base).collect()));
    }
    vec![joined]
}

/// The meshes at the given list positions (negative counts from the end; out of range is skipped).
pub fn select(geo: &Geo, indices: &[f32]) -> Geo {
    let len = geo.len() as i64;
    indices
        .iter()
        .filter_map(|&index| {
            let i = index.round() as i64;
            let i = if i < 0 { len + i } else { i };
            (0..len).contains(&i).then(|| geo[i as usize].clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::generators;

    #[test]
    fn per_mesh_parameters() {
        let circles = generators::circle(&[1.0, 1.0], &[4.0], &[0.0]);
        let moved = translate(&circles, &[0.0, 10.0], &[0.0], &[0.0]);
        assert!((moved[0].verts[0][0] - 1.0).abs() < 1e-6);
        assert!((moved[1].verts[0][0] - 11.0).abs() < 1e-6);
    }

    #[test]
    fn rotate_quarter_turn_about_y() {
        let line = generators::line(&[2.0], &[1.0]);
        let turned = rotate(&line, &[0.0], &[90.0], &[0.0]);
        let end = turned[0].verts[1]; // was (1, 0, 0)
        assert!(end[0].abs() < 1e-5 && (end[2] + 1.0).abs() < 1e-5);
    }

    #[test]
    fn array_join_select() {
        let points = generators::circle(&[2.0], &[6.0], &[0.0]);
        let shapes = generators::sphere(&[0.1], &[3.0], &[4.0]);
        let copies = array_on_verts(&shapes, &points);
        assert_eq!(copies.len(), 6);
        assert_eq!(copies[5].id, 5);
        let joined = join(&copies);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].verts.len(), 6 * shapes[0].verts.len());
        assert_eq!(select(&copies, &[0.0, -1.0, 99.0]).len(), 2);
    }
}
