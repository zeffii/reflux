# Reflux

A demo tool driven by Wax. Songs carry an animation lane (rows of `CC VVVV`
commands); Reflux reads the events, sends them through a node graph and renders a
raymarched scene on the GPU. Everything happens in one window: the render is
always the backdrop, and overlays (graph, lane view, later script editors) float
on top of it.

The demo song is still built in (written in Wax row notation); reading `.wax` files
comes later. The graph is loaded from a `.reflux` text file and reloads whenever that
file is saved, so it can be edited by hand in any editor while Reflux runs.

## Running

```
cargo run --release                       # opens scenes/demo.reflux
cargo run --release -- --scene my.reflux
```

| Key | |
|---|---|
| Tab | graph overlay |
| L | animation lane overlay |
| Esc | close overlay |
| D | step the dim level for the current overlay (0, 35, 60, 85%) |
| S | soft backdrop (desaturate and blur) for the current overlay |
| F | freeze the render as a still backdrop |
| P (hold) | peek: full-brightness render while held |
| Q | quality: standard / eco (half resolution, fewer march steps) |
| C | show / hide the SDF core |
| Space | play / pause |
| Home | back to the start |
| Ctrl+S | write node positions into the scene file's `layout` lines |

Drop an `.obj` (text format) on the window, or start with `--obj assets/knot.obj`, to put
a mesh in the scene; it intersects the raymarched shapes correctly.

Each overlay remembers its own dim setting. Nodes in the graph can be dragged by
their title.

## Screenshots without a window

```
reflux --shot out.png --time 9.4 --size 1280x720 --overlay graph
```

`reflux --help` lists the options. This renders headless, so it also works on a
software GPU driver.

## Layout

```
src/events/     command table, row decoding, timing, timeline, the demo song
src/scene.rs    song -> scene values (stand-in for the graph's wiring)
src/geo/        meshes, generators and operations
src/graph/      node types, .reflux parsing, evaluation
src/project.rs  the open scene file: hot reload, saving the layout
src/mesh/       .obj import
src/camera.rs   camera and mesh matrices shared by raymarch and raster
src/render/     raymarch + mesh pass, composite (dim/soft) pass, overlay drawing
src/shaders/    WGSL
src/overlay/    graph, lane view, style
src/view.rs     overlays and backdrop settings
src/app.rs      the window
src/shot.rs     headless PNG rendering
```

## The .reflux format

```
reflux 1
# comments start with #
node radii series start=1.45 step=0.25 count=3
node rings torus minor=0.02            # inputs not wired take these values
wire radii.out -> rings.major          # a wire overrides the value
node draw render mode=wire
wire rings.geo -> draw.geo
layout rings 300 120                   # written by Ctrl+S
```

A geo wire carries a list of meshes. Generators are vectorised: every input is a list,
the longest list decides how many meshes come out, and shorter lists repeat their last
value. Operations work per mesh the same way.

| Node | Inputs | Outputs |
|---|---|---|
| `number` | value | out |
| `series` | start, step, count | out |
| `time` | | seconds, beat |
| `wax commands=28,29,60` | | one per command, named from the command table |
| `map` | value, from_min, from_max, to_min, to_max | out |
| `circle` | radius, segments, fill | geo |
| `grid` | width, depth, columns, rows | geo |
| `line` | length, segments | geo |
| `arc` | radius, start, end (degrees), segments | geo |
| `sphere` | radius, rings, segments | geo |
| `torus` | major, minor, rings, segments | geo |
| `cylinder` | radius, height, segments, caps | geo |
| `move` / `rotate` / `scale` | geo, x, y, z (rotate in degrees) | geo |
| `array` | shape, points: a copy of each shape at every point vertex | geo |
| `join` | geo: all meshes merged into one | geo |
| `select` | geo, index (negative counts from the end) | geo |
| `render mode=solid\|wire\|both` | geo | |
| `camera` | angle (turns), radius, height, fov | |

World space is Y-up; flat shapes lie on the ground (XZ) plane.

## Commands

Rows are read left to right. `00–0F` are modifiers that apply to the next command
on the row only: `01` glide (ticks), `02` curve (0 step, 1 linear, 2 ease in,
3 ease out, 4 smooth, 5 spring), `03` relative. `10–7F` is the fixed standard set,
and `80–FF` are per-song auxiliary commands with a declared kind and field layout
(e.g. `80` pulse_ring, `XXYY` = speed, intensity).
