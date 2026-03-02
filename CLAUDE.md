# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build          # compile
cargo run            # build and launch the window
cargo test           # run tests
cargo test <name>    # run a single test by name
cargo clippy         # lint
```

## Architecture

`fale` ("waves" in Polish) is a single-file Rust interactive wave-interference simulator using `minifb` for software-rendered windowed graphics.

**`src/main.rs`** contains everything:

- **`PointSource`** — circular wave emitter at (x, y); accumulates `age` (seconds since placement), has `frequency` (Hz), `phase_offset` (rad), and `muted` (bool).
- **`LineSource`** — planar wave emitter between two endpoints; same per-source parameters including `muted`.
- **`Selected`** enum — tracks which source (by index into its respective `Vec`) is currently focused for keyboard control.

**Keyboard shortcuts:**
- Up/Down arrows — adjust frequency of selected source (`FREQ_RATE` Hz/s)
- Left/Right arrows — adjust phase offset (`PHASE_RATE` rad/s)
- Delete — remove selected source (resets `selected` to `None`)
- M — toggle `muted` on selected source (skips it in wave summation)
- C — clear all sources
- S — save current frame to `fale.png` (via `image` crate, PNG only)
- Escape — quit

**Render loop** (60 fps, fixed `dt = 1/60 s`):
1. Process keyboard input (arrows, Delete, M, C, S).
2. Process LMB click → select nearest existing source within 12 px, or create a new `PointSource`.
3. Process RMB drag-and-release → create a new `LineSource` between drag start and end; white preview line is drawn while dragging.
4. Increment `age` on every source.
5. Per-pixel wave summation: parallelised with `rayon` (`par_chunks_mut(WIDTH)` — one chunk per row). Muted sources are skipped. For each active source, contribution is `amplitude * exp(-DECAY * dist) * sin(2π * freq * (age - dist/WAVE_SPEED) + phase_offset)`, gated on `phase > 0` (wavefront hasn't arrived yet). Sum is passed through `tanh` and mapped to [0, 1].
6. `value_to_color` maps [0, 1] → blue→cyan→green→yellow gradient (4 linear segments).
7. Draw source markers: grey (unselected), yellow (selected), orange-red `0xFF4400` (muted).
8. Update window title to show selected source parameters.

**Important index-safety rule:** after `Vec::remove` the `Selected` index is always reset to `None`, because subsequent indices shift.

**Key constants** (`src/main.rs` top):
- `WIDTH`/`HEIGHT` — window pixel dimensions (1200×900).
- `WAVE_SPEED` — pixel/s propagation speed.
- `AMPLITUDE`, `DECAY` — wave envelope parameters.
- `DEFAULT_FREQUENCY`, `FREQ_RATE`, `PHASE_RATE` — starting frequency and per-second adjustment rates for arrow keys.

**Helper functions:**
- `dist_to_segment` — point-to-line-segment distance used for `LineSource` wave phase and hit-testing.
- `draw_circle`, `draw_line_pixels` — direct pixel-buffer drawing (Bresenham line algorithm).
