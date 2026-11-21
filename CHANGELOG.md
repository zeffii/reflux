# Changelog

## 001 · 2026-11-21

First prototype.
- Rust + wgpu window with a raymarched SDF scene (twisting ribbed tunnel, smooth-union core) as the backdrop.
- Built-in demo song written in Wax animation-lane notation; rows decode left to right with modifiers (`01` glide, `02` curve, `03` relative) applying to the next command only.
- Timeline: target commands glide (step, linear, ease in/out, smooth, spring) and continue smoothly when interrupted; trigger commands pulse and decay; aux command `80` splits `XXYY` by its declared layout.
- Graph overlay (Tab) with live hex values and activity on ports, draggable nodes; lane overlay (L) with a thin red playhead.
- Backdrop treatment per overlay: dim levels, soft (desaturate + blur), freeze, hold-to-peek; standard/eco quality.
- `--shot` renders a PNG headless.
