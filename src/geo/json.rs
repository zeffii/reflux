//! Geometry written as JSON: one or more named objects, each holding a mesh.
//!
//! ```json
//! {
//!   "obj1": { "mesh": { "verts": [[0,0,0], [1,0,0], [1,1,0]], "faces": [[0,1,2]] } },
//!   "obj2": { "mesh": { "verts": [...], "edges": [[0,1]] } }
//! }
//! ```
//!
//! Each object becomes one mesh on the geo wire, in file order. Accepted names:
//! `verts`/`vertices`, `edges`, `faces`/`polygons`/`polys`. Vertices may be nested
//! (`[[x,y,z],...]`) or flat (`[x,y,z,x,y,z,...]`). Missing edges are derived from
//! the faces. A file that is a single mesh (`{"verts": ...}` or `{"mesh": ...}`) also works.

use serde_json::{Map, Value};

use super::{Geo, Mesh};

/// The meshes in the file, with the name each came under.
pub fn parse(text: &str) -> Result<Vec<(String, Mesh)>, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    let root = root.as_object().ok_or("the top level should be an object { ... }")?;

    // A bare mesh, or a single {"mesh": ...}.
    if let Some(mesh) = mesh_object(root) {
        return Ok(vec![("mesh".to_string(), parse_mesh(mesh, "mesh", 0)?)]);
    }

    let mut meshes = Vec::new();
    for (name, value) in root {
        let object = value
            .as_object()
            .ok_or_else(|| format!("'{name}' should be an object"))?;
        let mesh = mesh_object(object).ok_or_else(|| format!("'{name}' has no mesh in it"))?;
        meshes.push((name.clone(), parse_mesh(mesh, name, meshes.len())?));
    }
    if meshes.is_empty() {
        return Err("the file has no objects".to_string());
    }
    Ok(meshes)
}

/// Just the meshes, optionally only the named objects (in the order asked for).
pub fn parse_geo(text: &str, only: &[String]) -> Result<Geo, String> {
    let named = parse(text)?;
    if only.is_empty() {
        return Ok(named.into_iter().map(|(_, mesh)| mesh).collect());
    }
    only.iter()
        .enumerate()
        .map(|(id, wanted)| {
            named
                .iter()
                .find(|(name, _)| name == wanted)
                .map(|(_, mesh)| Mesh {
                    id: id as u32,
                    ..mesh.clone()
                })
                .ok_or_else(|| format!("no object called '{wanted}'"))
        })
        .collect()
}

/// The object holding vertex data: either this one, or its "mesh" entry.
fn mesh_object(object: &Map<String, Value>) -> Option<&Map<String, Value>> {
    if field(object, &["verts", "vertices"]).is_some() {
        return Some(object);
    }
    object.get("mesh")?.as_object()
}

fn field<'a>(object: &'a Map<String, Value>, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|name| object.get(*name))
}

fn parse_mesh(object: &Map<String, Value>, name: &str, id: usize) -> Result<Mesh, String> {
    let fail = |what: &str| format!("'{name}': {what}");
    let verts_value = field(object, &["verts", "vertices"]).ok_or_else(|| fail("no verts"))?;
    let verts = parse_verts(verts_value).map_err(|e| fail(&e))?;

    let faces = match field(object, &["faces", "polygons", "polys"]) {
        Some(value) => parse_index_lists(value, verts.len(), 3).map_err(|e| fail(&format!("faces: {e}")))?,
        None => Vec::new(),
    };
    let edges = match object.get("edges") {
        Some(value) => parse_index_lists(value, verts.len(), 2)
            .map_err(|e| fail(&format!("edges: {e}")))?
            .into_iter()
            .map(|pair| {
                if pair.len() == 2 {
                    Ok([pair[0], pair[1]])
                } else {
                    Err(fail("every edge needs exactly two indices"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => Mesh::edges_from_faces(&faces),
    };

    Ok(Mesh {
        verts,
        edges,
        faces,
        id: id as u32,
    })
}

fn number(value: &Value) -> Result<f32, String> {
    value
        .as_f64()
        .map(|v| v as f32)
        .ok_or_else(|| format!("{value} is not a number"))
}

fn parse_verts(value: &Value) -> Result<Vec<[f32; 3]>, String> {
    let list = value.as_array().ok_or("verts should be a list")?;
    if list.first().is_some_and(Value::is_number) {
        // Flat: x, y, z, x, y, z, ...
        if list.len() % 3 != 0 {
            return Err("a flat verts list needs a multiple of 3 numbers".to_string());
        }
        return list
            .chunks(3)
            .map(|c| Ok([number(&c[0])?, number(&c[1])?, number(&c[2])?]))
            .collect();
    }
    list.iter()
        .map(|v| {
            let xyz = v.as_array().ok_or("each vert should be [x, y, z]")?;
            if xyz.len() < 3 {
                return Err("each vert needs 3 numbers".to_string());
            }
            Ok([number(&xyz[0])?, number(&xyz[1])?, number(&xyz[2])?])
        })
        .collect()
}

fn parse_index_lists(value: &Value, vert_count: usize, minimum: usize) -> Result<Vec<Vec<u32>>, String> {
    let list = value.as_array().ok_or("should be a list of lists")?;
    list.iter()
        .map(|entry| {
            let indices = entry.as_array().ok_or("each entry should be a list of indices")?;
            if indices.len() < minimum {
                return Err(format!("an entry has fewer than {minimum} indices"));
            }
            indices
                .iter()
                .map(|i| {
                    let index = i.as_u64().ok_or_else(|| format!("{i} is not an index"))? as usize;
                    if index >= vert_count {
                        return Err(format!("index {index} is past the last vert ({})", vert_count - 1));
                    }
                    Ok(index as u32)
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = r#"{
        "obj1": { "mesh": { "verts": [[0,0,0],[1,0,0],[1,1,0],[0,1,0]], "faces": [[0,1,2,3]] } },
        "obj2": { "mesh": { "vertices": [0,0,0, 0,0,2], "edges": [[0,1]] } }
    }"#;

    #[test]
    fn objects_become_meshes_in_file_order() {
        let meshes = parse(TWO).unwrap();
        assert_eq!(meshes.len(), 2);
        assert_eq!(meshes[0].0, "obj1");
        assert_eq!(meshes[0].1.edges.len(), 4); // derived from the quad
        assert_eq!(meshes[1].1.verts[1], [0.0, 0.0, 2.0]); // flat list
        assert_eq!(meshes[1].1.id, 1);
    }

    #[test]
    fn picking_objects_by_name() {
        let geo = parse_geo(TWO, &["obj2".to_string()]).unwrap();
        assert_eq!(geo.len(), 1);
        assert_eq!(geo[0].edges, vec![[0, 1]]);
        assert!(parse_geo(TWO, &["nope".to_string()]).is_err());
    }

    #[test]
    fn a_bare_mesh_and_bad_input() {
        let geo = parse(r#"{"verts": [[0,0,0],[1,0,0],[0,0,1]], "polygons": [[0,1,2]]}"#).unwrap();
        assert_eq!(geo[0].1.faces.len(), 1);
        assert!(parse("[1,2]").is_err());
        assert!(parse(r#"{"a": {"mesh": {"verts": [[0,0,0]], "faces": [[0,1,2]]}}}"#)
            .unwrap_err()
            .contains("past the last vert"));
    }
}
