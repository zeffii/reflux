//! Reading Wavefront .obj (the text format).
//!
//! Understood: `v`, `vn`, `f` (with `v`, `v/vt`, `v//vn`, `v/vt/vn` corners, negative
//! indices, and polygons of any size, which are fan-triangulated). Texture
//! coordinates, materials, groups and smoothing groups are skipped for now; all
//! objects in the file become one mesh.

use super::MeshData;

#[derive(Debug, PartialEq)]
pub struct ObjError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ObjError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// One face corner: index into positions and optionally into normals (both 0-based).
#[derive(Clone, Copy)]
struct Corner {
    position: usize,
    normal: Option<usize>,
}

pub fn parse(text: &str) -> Result<MeshData, ObjError> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut triangles: Vec<[Corner; 3]> = Vec::new();

    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let error = |message: String| ObjError {
            line: line_number,
            message,
        };
        let line = raw_line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        let Some(keyword) = parts.next() else {
            continue;
        };
        match keyword {
            "v" => positions.push(parse_vector(&mut parts).map_err(error)?),
            "vn" => normals.push(parse_vector(&mut parts).map_err(error)?),
            "f" => {
                let corners = parts
                    .map(|token| parse_corner(token, positions.len(), normals.len()))
                    .collect::<Result<Vec<Corner>, String>>()
                    .map_err(error)?;
                if corners.len() < 3 {
                    return Err(error("a face needs at least 3 corners".to_string()));
                }
                for i in 1..corners.len() - 1 {
                    triangles.push([corners[0], corners[i], corners[i + 1]]);
                }
            }
            _ => {} // vt, o, g, s, usemtl, mtllib, ...
        }
    }

    if triangles.is_empty() {
        return Err(ObjError {
            line: 0,
            message: "the file has no faces".to_string(),
        });
    }

    let smooth_normals = vertex_normals(&positions, &triangles);
    let mut mesh = MeshData::default();
    for triangle in &triangles {
        for corner in triangle {
            mesh.positions.push(positions[corner.position]);
            let normal = match corner.normal {
                Some(n) => normals[n],
                None => smooth_normals[corner.position],
            };
            mesh.normals.push(normal);
        }
    }
    Ok(mesh)
}

/// Positions and polygon faces only (no triangulation, no normals), for geo wires.
pub fn parse_polygons(text: &str) -> Result<(Vec<[f32; 3]>, Vec<Vec<u32>>), ObjError> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut faces: Vec<Vec<u32>> = Vec::new();
    for (index, raw_line) in text.lines().enumerate() {
        let error = |message: String| ObjError {
            line: index + 1,
            message,
        };
        let line = raw_line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => positions.push(parse_vector(&mut parts).map_err(error)?),
            Some("f") => {
                let face = parts
                    // Normal indices aren't needed here, so any count accepts them.
                    .map(|token| parse_corner(token, positions.len(), i64::MAX as usize).map(|c| c.position as u32))
                    .collect::<Result<Vec<u32>, String>>()
                    .map_err(error)?;
                if face.len() < 3 {
                    return Err(error("a face needs at least 3 corners".to_string()));
                }
                faces.push(face);
            }
            _ => {}
        }
    }
    if faces.is_empty() {
        return Err(ObjError {
            line: 0,
            message: "the file has no faces".to_string(),
        });
    }
    Ok((positions, faces))
}

fn parse_vector<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Result<[f32; 3], String> {
    let mut vector = [0.0; 3];
    for component in &mut vector {
        let token = parts.next().ok_or("expected three numbers")?;
        *component = token.parse().map_err(|_| format!("not a number: {token}"))?;
    }
    Ok(vector)
}

/// Resolves a 1-based (or negative, counting back from the end) obj index.
fn resolve_index(token: &str, count: usize) -> Result<usize, String> {
    let value: i64 = token.parse().map_err(|_| format!("bad index: {token}"))?;
    let resolved = if value > 0 {
        value - 1
    } else {
        count as i64 + value
    };
    if value == 0 || resolved < 0 || resolved >= count as i64 {
        return Err(format!("index {token} is out of range"));
    }
    Ok(resolved as usize)
}

fn parse_corner(token: &str, position_count: usize, normal_count: usize) -> Result<Corner, String> {
    let mut fields = token.split('/');
    let position = resolve_index(fields.next().unwrap_or(""), position_count)?;
    let _texture = fields.next();
    let normal = match fields.next() {
        Some(n) if !n.is_empty() => Some(resolve_index(n, normal_count)?),
        _ => None,
    };
    Ok(Corner { position, normal })
}

/// Area-weighted normals per position, for files that don't carry their own.
fn vertex_normals(positions: &[[f32; 3]], triangles: &[[Corner; 3]]) -> Vec<[f32; 3]> {
    let mut sums = vec![[0.0f32; 3]; positions.len()];
    for triangle in triangles {
        let [a, b, c] = triangle.map(|corner| positions[corner.position]);
        let face = cross(sub(b, a), sub(c, a)); // length = 2 x area
        for corner in triangle {
            let sum = &mut sums[corner.position];
            for axis in 0..3 {
                sum[axis] += face[axis];
            }
        }
    }
    sums.into_iter().map(normalise).collect()
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalise(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length < 1e-12 {
        return [0.0, 1.0, 0.0];
    }
    [v[0] / length, v[1] / length, v[2] / length]
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUAD: &str = "\
# a unit quad in the xy plane
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
vt 0 0
f 1/1 2/1 3/1 4/1
";

    #[test]
    fn quads_are_fan_triangulated_with_computed_normals() {
        let mesh = parse(QUAD).unwrap();
        assert_eq!(mesh.positions.len(), 6);
        assert_eq!(mesh.positions[3], [0.0, 0.0, 0.0]);
        for normal in &mesh.normals {
            assert!((normal[2] - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn explicit_normals_and_negative_indices() {
        let text = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 -1\nf -3//1 -2//1 -1//1\n";
        let mesh = parse(text).unwrap();
        assert_eq!(mesh.positions, vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        assert_eq!(mesh.normals[0], [0.0, 0.0, -1.0]);
    }

    #[test]
    fn polygons_keep_their_corners() {
        let (positions, faces) = parse_polygons(QUAD).unwrap();
        assert_eq!(positions.len(), 4);
        assert_eq!(faces, vec![vec![0, 1, 2, 3]]);
    }

    #[test]
    fn errors_name_the_line() {
        let error = parse("v 0 0 0\nv 1 0 0\nf 1 2 9\n").unwrap_err();
        assert_eq!(error.line, 3);
        assert!(parse("v 0 0 0\n").is_err());
        assert_eq!(parse("v 0 zero 0\nf 1 1 1").unwrap_err().line, 1);
    }
}
