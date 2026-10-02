# ApexRuntime

A standalone, portable re-implementation of **Apex Legends-style player
movement**, written from scratch in Rust, designed to be embedded in other
games or run in its own first-person sandbox.

The long-term goal is **behavioural 1:1 fidelity**: identical start state,
geometry, inputs and tick timing should produce the same velocity, position,
jump arc, wall response and movement-tech results as Apex. We get there by
measurement and independent re-implementation, **not** by copying Respawn's
code or assets.

> **Status, Phase 1:** architecture proven. A deterministic, engine-independent
> core steps a player against an abstract collision interface, with tests,
> replayable input scripts, CSV traces and a Bevy sandbox. The movement
> *rules* are still a Source-lineage placeholder, not Apex.

## Layout

| crate | role |
|---|---|
| `apex-math` | f32 vectors and angles (Z-up, Source units, deterministic trig) |
| `apex-collision` | `CollisionWorld` trait (AABB hull sweeps), a brush-world reference implementation, built-in test map |
| `apex-core` | **the movement**: `step(state, input, config, world, dt) -> state`. Depends on nothing else. |
| `apex-telemetry` | trace rows, CSV/JSON, input scripts, `apex-replay` CLI |
| `apex-sandbox` | Bevy first-person test bed (the only crate using Bevy) |

## Quick start (Windows)

```powershell
cd C:\Dev\ApexRuntime
cargo test --workspace
cargo run -p apex-sandbox --release
cargo run -p apex-telemetry --bin apex-replay -- scripts/inputs/bhop.json --out traces/bhop.csv
```

Sandbox controls: **click** to capture the mouse · WASD · Space jump ·
Ctrl/C crouch · Shift sprint · F1 HUD · F5 start/stop recording (writes a CSV
trace and a replayable input script to `traces/`) · R respawn · Esc
release the mouse.

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
