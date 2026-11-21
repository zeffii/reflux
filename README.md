# Reflux

A demo tool driven by Wax. Songs carry an animation lane (rows of `CC VVVV`
commands); Reflux reads the events, sends them through a node graph and renders a
raymarched scene on the GPU. Everything happens in one window: the render is
always the backdrop, and overlays (graph, lane view, later script editors) float
on top of it.

This is the first prototype: the demo song is built in, written in Wax row notation,
and the graph mirrors hard-wired mappings. It doesn't read `.wax` or `.reflux` files yet.

## Running

```
cargo run --release
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
| Space | play / pause |
| Home | back to the start |

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
src/render/     raymarch pass, composite (dim/soft) pass, overlay drawing
src/shaders/    WGSL
src/overlay/    graph, lane view, style
src/view.rs     overlays and backdrop settings
src/app.rs      the window
src/shot.rs     headless PNG rendering
```

## Commands

Rows are read left to right. `00–0F` are modifiers that apply to the next command
on the row only: `01` glide (ticks), `02` curve (0 step, 1 linear, 2 ease in,
3 ease out, 4 smooth, 5 spring), `03` relative. `10–7F` is the fixed standard set,
and `80–FF` are per-song auxiliary commands with a declared kind and field layout
(e.g. `80` pulse_ring, `XXYY` = speed, intensity).
