//! Mesh generators. World space is Y-up; flat shapes lie in the XZ (ground) plane.
//!
//! Every generator is vectorised: the number of meshes made is the length of the
//! longest parameter list, and shorter lists repeat their last value.

use std::f32::consts::TAU;

use super::{nth, Geo, Mesh};

/// How many meshes a vectorised generator makes.
fn count(params: &[&[f32]]) -> usize {
    params.iter().map(|p| p.len()).max().unwrap_or(1).max(1)
}

/// Segment counts come in as floats from wires; keep them sane.
fn segments(value: f32, minimum: u32) -> u32 {
    (value.round() as i64).clamp(minimum as i64, 1024) as u32
}

fn mesh_with_faces(verts: Vec<[f32; 3]>, faces: Vec<Vec<u32>>, id: usize) -> Mesh {
    Mesh {
        edges: Mesh::edges_from_faces(&faces),
        verts,
        faces,
        id: id as u32,
    }
}

/// A circle of `segments` points; with `fill` > 0.5 it also gets one n-gon face.
pub fn circle(radius: &[f32], segment_count: &[f32], fill: &[f32]) -> Geo {
    (0..count(&[radius, segment_count, fill]))
        .map(|i| {
            let r = nth(radius, i);
            let n = segments(nth(segment_count, i), 3);
            let verts = (0..n)
                .map(|k| {
                    let a = TAU * k as f32 / n as f32;
                    [r * a.cos(), 0.0, r * a.sin()]
                })
                .collect();
            let edges = (0..n).map(|k| [k, (k + 1) % n]).collect();
            let faces = if nth(fill, i) > 0.5 {
                vec![(0..n).rev().collect()] // wound to face up
            } else {
                Vec::new()
            };
            Mesh {
                verts,
                edges,
                faces,
                id: i as u32,
            }
        })
        .collect()
}

/// A flat grid of quads, centred on the origin.
pub fn grid(width: &[f32], depth: &[f32], columns: &[f32], rows: &[f32]) -> Geo {
    (0..count(&[width, depth, columns, rows]))
        .map(|i| {
            let (w, d) = (nth(width, i), nth(depth, i));
            let (nx, nz) = (segments(nth(columns, i), 1), segments(nth(rows, i), 1));
            let mut verts = Vec::new();
            for z in 0..=nz {
                for x in 0..=nx {
                    verts.push([
                        (x as f32 / nx as f32 - 0.5) * w,
                        0.0,
                        (z as f32 / nz as f32 - 0.5) * d,
                    ]);
                }
            }
            let row = nx + 1;
            let mut faces = Vec::new();
            for z in 0..nz {
                for x in 0..nx {
                    let a = z * row + x;
                    faces.push(vec![a, a + row, a + row + 1, a + 1]);
                }
            }
            mesh_with_faces(verts, faces, i)
        })
        .collect()
}

/// A straight line along X, centred on the origin (edges only).
pub fn line(length: &[f32], segment_count: &[f32]) -> Geo {
    (0..count(&[length, segment_count]))
        .map(|i| {
            let l = nth(length, i);
            let n = segments(nth(segment_count, i), 1);
            Mesh {
                verts: (0..=n).map(|k| [(k as f32 / n as f32 - 0.5) * l, 0.0, 0.0]).collect(),
                edges: (0..n).map(|k| [k, k + 1]).collect(),
                faces: Vec::new(),
                id: i as u32,
            }
        })
        .collect()
}

/// An arc in the ground plane between two angles in degrees (edges only).
pub fn arc(radius: &[f32], start: &[f32], end: &[f32], segment_count: &[f32]) -> Geo {
    (0..count(&[radius, start, end, segment_count]))
        .map(|i| {
            let r = nth(radius, i);
            let (a0, a1) = (nth(start, i).to_radians(), nth(end, i).to_radians());
            let n = segments(nth(segment_count, i), 1);
            Mesh {
                verts: (0..=n)
                    .map(|k| {
                        let a = a0 + (a1 - a0) * k as f32 / n as f32;
                        [r * a.cos(), 0.0, r * a.sin()]
                    })
                    .collect(),
                edges: (0..n).map(|k| [k, k + 1]).collect(),
                faces: Vec::new(),
                id: i as u32,
            }
        })
        .collect()
}

/// A UV sphere: quads, with triangle fans at the poles.
pub fn sphere(radius: &[f32], ring_count: &[f32], segment_count: &[f32]) -> Geo {
    (0..count(&[radius, ring_count, segment_count]))
        .map(|i| {
            let r = nth(radius, i);
            let rings = segments(nth(ring_count, i), 2);
            let segs = segments(nth(segment_count, i), 3);
            let mut verts = vec![[0.0, r, 0.0]]; // north pole
            for ring in 1..rings {
                let polar = std::f32::consts::PI * ring as f32 / rings as f32;
                for s in 0..segs {
                    let a = TAU * s as f32 / segs as f32;
                    verts.push([r * polar.sin() * a.cos(), r * polar.cos(), r * polar.sin() * a.sin()]);
                }
            }
            let south = verts.len() as u32;
            verts.push([0.0, -r, 0.0]);

            let ring_start = |ring: u32| 1 + (ring - 1) * segs;
            let mut faces = Vec::new();
            for s in 0..segs {
                let next = (s + 1) % segs;
                faces.push(vec![0, ring_start(1) + next, ring_start(1) + s]);
                for ring in 1..rings - 1 {
                    let (a, b) = (ring_start(ring), ring_start(ring + 1));
                    faces.push(vec![a + s, a + next, b + next, b + s]);
                }
                let last = ring_start(rings - 1);
                faces.push(vec![south, last + s, last + next]);
            }
            mesh_with_faces(verts, faces, i)
        })
        .collect()
}

/// A torus around the Y axis.
pub fn torus(major: &[f32], minor: &[f32], ring_count: &[f32], segment_count: &[f32]) -> Geo {
    (0..count(&[major, minor, ring_count, segment_count]))
        .map(|i| {
            let (big, small) = (nth(major, i), nth(minor, i));
            let rings = segments(nth(ring_count, i), 3);
            let segs = segments(nth(segment_count, i), 3);
            let mut verts = Vec::new();
            for ring in 0..rings {
                let a = TAU * ring as f32 / rings as f32;
                for s in 0..segs {
                    let b = TAU * s as f32 / segs as f32;
                    let reach = big + small * b.cos();
                    verts.push([reach * a.cos(), small * b.sin(), reach * a.sin()]);
                }
            }
            let mut faces = Vec::new();
            for ring in 0..rings {
                let next_ring = (ring + 1) % rings;
                for s in 0..segs {
                    let next = (s + 1) % segs;
                    faces.push(vec![
                        ring * segs + s,
                        ring * segs + next,
                        next_ring * segs + next,
                        next_ring * segs + s,
                    ]);
                }
            }
            mesh_with_faces(verts, faces, i)
        })
        .collect()
}

/// A cylinder along Y, centred on the origin, optionally capped.
pub fn cylinder(radius: &[f32], height: &[f32], segment_count: &[f32], caps: &[f32]) -> Geo {
    (0..count(&[radius, height, segment_count, caps]))
        .map(|i| {
            let (r, h) = (nth(radius, i), nth(height, i));
            let n = segments(nth(segment_count, i), 3);
            let mut verts = Vec::new();
            for y in [-h * 0.5, h * 0.5] {
                for k in 0..n {
                    let a = TAU * k as f32 / n as f32;
                    verts.push([r * a.cos(), y, r * a.sin()]);
                }
            }
            let mut faces: Vec<Vec<u32>> = (0..n)
                .map(|k| {
                    let next = (k + 1) % n;
                    vec![k, n + k, n + next, next]
                })
                .collect();
            if nth(caps, i) > 0.5 {
                faces.push((0..n).collect());
                faces.push((n..2 * n).rev().collect());
            }
            mesh_with_faces(verts, faces, i)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generators_are_vectorised() {
        let circles = circle(&[1.0, 2.0, 3.0], &[8.0], &[0.0]);
        assert_eq!(circles.len(), 3);
        assert_eq!(circles[2].verts.len(), 8);
        assert!((circles[2].verts[0][0] - 3.0).abs() < 1e-6);
        assert_eq!(circles.iter().map(|m| m.id).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn closed_shapes_have_consistent_topology() {
        let t = &torus(&[1.0], &[0.25], &[8.0], &[4.0])[0];
        assert_eq!(t.verts.len(), 32);
        assert_eq!(t.faces.len(), 32);
        assert_eq!(t.edges.len(), 64); // a closed quad torus: E = 2F
        let s = &sphere(&[1.0], &[4.0], &[6.0])[0];
        assert_eq!(s.verts.len(), 2 + 3 * 6);
        assert_eq!(s.faces.len(), 6 * 4);
        // Euler characteristic of a sphere: V - E + F = 2
        assert_eq!(s.verts.len() as i64 - s.edges.len() as i64 + s.faces.len() as i64, 2);
    }

    #[test]
    fn grid_and_line_counts() {
        let g = &grid(&[2.0], &[2.0], &[4.0], &[2.0])[0];
        assert_eq!(g.verts.len(), 15);
        assert_eq!(g.faces.len(), 8);
        let l = &line(&[2.0], &[4.0])[0];
        assert_eq!(l.edges.len(), 4);
        assert!(l.faces.is_empty());
    }
}
