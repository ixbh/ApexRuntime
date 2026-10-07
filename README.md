# ApexRuntime

A standalone, portable re-implementation of **Apex Legends-style player
movement**, written from scratch in Rust, designed to be embedded in other
games or run in its own first-person sandbox.

The long-term goal is **behavioural 1:1 fidelity**: identical start state,
geometry, inputs and tick timing should produce the same velocity, position,
jump arc, wall response and movement-tech results as Apex. We get there by
measurement and independent re-implementation, **not** by copying Respawn's
code or assets.

> **Status:** movement is measured against offline R5Flowstate and replays it
> tick for tick (sprint, slide, lurch, tap-strafe, bhop, mantle, superglide,
> mantle boost, wall climb, fall stun, slopes). Weapon behaviour (fire
> modes, recoil, reloads and what cancels them, swaps, shared ammo) runs in
> `apex-weapons`. It plays inside MW2 through an IW4L fork.
>
> **To play it, see [INSTALL.md](INSTALL.md).**

## Layout

| crate | role |
|---|---|
| `apex-math` | f32 vectors and angles (Z-up, Source units, deterministic trig) |
| `apex-collision` | `CollisionWorld` trait (AABB hull sweeps), a brush-world reference implementation, built-in test map |
| `apex-core` | **the movement**: `step(state, input, config, world, dt) -> state`. Depends on nothing else. |
| `apex-telemetry` | trace rows, CSV/JSON, input scripts, `apex-replay` CLI |
| `apex-sandbox` | Bevy first-person test bed (the only crate using Bevy) |
| `apex-weapons` | Apex weapon engine: fire modes, spread, recoil, reloads, swaps, ammo |
| `apex-cast` | reader for Cast model/animation files (your own exports) |
| `apex-iw4l` | glue to IW4L's player state and collision |
| `apex-tables` | `preflight` (what is unfinished?) and `trace-diff` (does our trace match the reference?) |

All tuning numbers live in `sheets/constants.tsv` and per-mechanic progress in
`sheets/mechanics.tsv`. The sheets are the source of truth; code is generated
from or checked against them.

## Quick start (Windows)

```powershell
cargo test --workspace
cargo run -p apex-tables --bin preflight
cargo run -p apex-sandbox --release
cargo run -p apex-telemetry --bin apex-replay -- scripts/inputs/bhop.json --out traces/bhop.csv
```

Sandbox controls: **click** to capture the mouse · WASD · Space jump ·
Ctrl/C/L crouch · Shift sprint · scroll up = W tap · scroll down = jump tap ·
F1 HUD · F2 always-sprint · F3 first/third person (hull-sized body) · F4 camera
feel (bob, tilt, landing dip) · F5 start/stop recording (writes a CSV trace and
a replayable input script to `traces/`) · F6 movement trail · R respawn · Esc
release the mouse. Surfaces carry a measuring grid: thin line every 16 u, bold
every 64 u (walls double as height rulers).
`cargo run -p apex-sandbox --release -- --hz 165 --fov 110` simulates at 165
ticks/s (default 144; superglide timing is per tick, so practise at your real
frame rate) with Apex's FOV slider value (default 110).

## Ground rules

- **Your game files stay yours.** The runtime may later *read* an owned Apex
  install (path in the git-ignored `tools/local-env.ps1`) but never modifies
  it, and no game assets are ever committed or redistributed.
- **No online tooling.** No anti-cheat bypasses, no live-game memory access,
  nothing for ranked/competitive play. Measurement is passive (video) or
  done in private/community environments (R5Flowstate).
- **Reference code is reference.** See `docs/LICENSE-REFERENCES.md`.

## Docs

| | |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | crate boundaries, conventions, time model, collision contract, adapter plan (IW4L) |
| [`docs/MOVEMENT-MODEL.md`](docs/MOVEMENT-MODEL.md) | the maths of every rule, plus hypotheses for lurch/slide/superglide |
| [`docs/MOVEMENT-MEASUREMENTS.md`](docs/MOVEMENT-MEASUREMENTS.md) | every constant with source, method, build, confidence, plus the measurement plan |
| [`docs/R5-RESEARCH.md`](docs/R5-RESEARCH.md) | where movement lives in R5 (S3 vs S21 vs live) |
| [`docs/LICENSE-REFERENCES.md`](docs/LICENSE-REFERENCES.md) | licence audit of every reference repo |
| [`docs/ENVIRONMENT.md`](docs/ENVIRONMENT.md) | toolchain, installs, everyday commands |
| [`docs/FLOWSTATE-CAPTURE.md`](docs/FLOWSTATE-CAPTURE.md) | capturing ground-truth movement traces from R5Flowstate, test protocol |

## Licence

`MIT OR Apache-2.0` (see `LICENSE-MIT`, `LICENSE-APACHE`). Apex Legends is a
trademark of Electronic Arts / Respawn Entertainment; this project is not
affiliated with or endorsed by them.
