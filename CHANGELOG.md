# Changelog

## 002 · 2026-11-21

- Import Wavefront .obj (text): `--obj FILE.obj`, or drop a file on the window. Handles `v`/`vn`/`f` with any corner form, negative indices, polygons (fan-triangulated) and `#` comments; computes smooth normals when the file has none. The mesh is centred, fitted to the scene and turns slowly.
- Meshes are rasterised into the same depth buffer the raymarcher writes, so they intersect SDFs correctly, and are lit with the same light, fog, glow and flash.
- Loading a mesh hides the SDF core; C toggles the core back (`--keep-core` for shots).
- Graph: an `obj` node feeding the render node, named after the loaded file.
- Lane overlay: the playhead moves down until it reaches the middle, then stays still while the rows scroll.
- `assets/knot.obj` sample mesh.

## 001 · 2026-11-21

First prototype.
- Rust + wgpu window with a raymarched SDF scene (twisting ribbed tunnel, smooth-union core) as the backdrop.
- Built-in demo song written in Wax animation-lane notation; rows decode left to right with modifiers (`01` glide, `02` curve, `03` relative) applying to the next command only.
- Timeline: target commands glide (step, linear, ease in/out, smooth, spring) and continue smoothly when interrupted; trigger commands pulse and decay; aux command `80` splits `XXYY` by its declared layout.
- Graph overlay (Tab) with live hex values and activity on ports, draggable nodes; lane overlay (L) with a thin red playhead.
- Backdrop treatment per overlay: dim levels, soft (desaturate + blur), freeze, hold-to-peek; standard/eco quality.
- `--shot` renders a PNG headless.
