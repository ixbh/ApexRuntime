# Architecture

## The one rule

> **Movement does not know who is hosting it.**
> `apex-core` depends on `apex-math` and `apex-collision` and nothing else.
> No Bevy, no renderer, no ECS, no map format, no game.

```text
MovementState + MovementInput + &impl CollisionWorld + dt   ──step──▶   MovementState (+ StepReport)
```

This is IW4L's `TickInput → sim::step → Snapshot` funnel, cut down to one
player. As in IW4L: **state is explicit** (a plain `Copy` struct you can
clone and re-step), **time is an argument** (`dt` per call), and **the
result is the return value**.

## Crates

```text
crates/
  apex-math        Vec3, Aabb, angle helpers. f32, Z-up, Source units, libm trig.     deps: libm
  apex-collision   CollisionWorld trait (AABB hull sweep), Trace, BrushWorld
                   reference impl, built-in proving-ground map.                       deps: apex-math
  apex-core        MovementInput, MovementState, MovementConfig, step(),
                   slide/step/ground algorithms, FixedClock.                          deps: math, collision
  apex-telemetry   TraceRow, CSV/JSON output, InputScript (sparse, replayable),
                   divergence check, `apex-replay` CLI.                               deps: core, serde
  apex-sandbox     Bevy 0.19 first-person host: mouse look, WASD, HUD, recording.     deps: core, telemetry, bevy
  apex-tables      TSV/CSV tables, `preflight` (sheet completeness) and
                   `trace-diff` (golden-trace overlap).                               deps: none
sheets/            SOURCE OF TRUTH for data: constants.tsv (every tuning number,
                   with provenance; generates apex-core constants), mechanics.tsv
                   (coverage: doc/config/impl/tests/telemetry/measured per mechanic)
adapters/          (empty) future host adapters: iw4l, …
research/          references/ → junction to C:\Dev\GameReferences (git-ignored)
scripts/inputs/    input scripts (JSON) for apex-replay
tools/             local-env (ignored) + detection scripts
traces/            output of replay/sandbox (ignored)
```

Only `apex-sandbox` pulls in Bevy. `cargo test -p apex-core` builds in
seconds and never touches a GPU.

## Conventions

| | |
|---|---|
| Axes | **Z up**, yaw 0 = +X, yaw +90 = +Y, pitch + = look down (Source) |
| Units | Source units (1 u = 1 inch = 0.0254 m), seconds, u/s |
| Origin | Player origin = **bottom centre of the hull** (feet) |
| Hull | Axis-aligned box. Respawn's player hull is an AABB, so we keep the same query shape. |
| Floats | `f32` state (like the games), `f64` only for accumulated time. Trig via `libm` for cross-platform bit-equality. |
| Toolchain | Pinned in `rust-toolchain.toml` so traces are reproducible. |

Hosts convert axes and units at the adapter edge. The sandbox maps
`(x, y, z)ₛ → (x, z, −y)ᵦ × 0.0254`, the same transform the mashup uses
for Skate 3.

## Time

Apex runs `CGameMovement` once per **usercmd**, and the client sends one
usercmd per rendered frame with its own frame time. So Apex movement is
partly frame-rate dependent (superglide is the famous case). Therefore:

- `step()` takes `dt` and never assumes a rate.
- `FixedClock` gives hosts a fixed tick (sandbox: 60 Hz) so render FPS
  cannot change outcomes. A test enforces this (30 fps vs jittery 144 fps
  produce identical state).
- `InputScript` can change `dt` mid-script, to replay measured Apex traces
  with their real per-command frame times.

## Collision contract

```rust
trait CollisionWorld {
    fn trace_hull(&self, start: Vec3, end: Vec3, hull: Aabb) -> Trace; // fraction, end_pos, normal, start_solid, all_solid, surface
}
```

The same shape as IW4L's `CollisionBackend::trace(GroundTraceInput)` and
Source's `TraceHull`. An adapter only has to sweep a box. Surface tags
(`u32`) are passed through for future mantle/ladder/zipline semantics.
Traces stop `DIST_EPSILON` (1/32 u) short of planes. A hull exactly touching
a plane counts as outside (hosts spawn players flush on floors).

## Adapters (future)

```text
host input  ──▶ MovementInput        (axes, buttons, absolute view angles)
host world  ──▶ impl CollisionWorld  (box sweep over the host's collision)
MovementState ──▶ host transform/velocity (+ units/axes conversion)
```

### IW4L (first integration target)

- IW4L's movement is `crates/movement_iw4` (`no_std`, `pmove(ps, cmd, ctx,
  &impl CollisionBackend, …)`), called from `crates/sim/src/step.rs` around
  line 403, inside the one `sim::step` funnel. That funnel serves authority,
  prediction and replay alike.
- `ClipBackend` in the same file already implements box traces against
  IW4's clip map, brush models and glass. An `adapters/iw4l` crate would
  wrap it as a `CollisionWorld` and swap the `pmove` call for `apex_core::step`
  behind a flag. IW4 also uses inches and Z-up, so no unit conversion is needed.
- The mashup shows the precedent: it adds a whole foreign controller (Skate 3)
  by extracting clip triangles from `asset_world::ClipCollision` and switching
  "movement controller" per player.
- IW4L uses Bevy 0.19 and the sandbox matches it.

### What the mashup taught

Generic and reusable: IW4L's asset transport, `session` lifecycle, `sim`
funnel, render island and the controller-swap point. Game-specific:
`skate-core` (pure physics, its own workspace with no engine deps),
`render_anim/src/skate/*` (the adapter: unit/axis conversion, collision
extraction, rig) and `minecraft_terrain`. **ApexRuntime is `skate-core`'s
role, done deliberately from day one.**

## Movement pipeline (Phase 1)

See [`MOVEMENT-MODEL.md`](MOVEMENT-MODEL.md). New mechanics get **explicit
`MoveMode` variants** (Slide, Mantle, WallClimb, Zipline) instead of piles
of flags, so each mode owns its rules and its timing windows.

## Sheets (adapted from "The Spreadsheet Method")

Numbers are data, movement rules are code ("push constants into sheets, keep
logic in kernels"):

- `sheets/constants.tsv` → `apex-core/build.rs` → `apex_core::sheet::*`
  consts + `ROWS` with provenance. A malformed row fails the build.
- `sheets/mechanics.tsv`: one row per target mechanic. A cell is `y`, `n` or `-`;
  every `n` is unfinished work. `preflight` prints the matrix and checks that
  the constants sheet and `config.rs` agree. **Run preflight before every commit.**
- Not adopted: decompiling Apex (conflicts with this project's rules and the
  method's own legal section), per-row code generation for algorithms, swarm
  claim boards.

## Testing strategy

1. **Property tests** (`crates/apex-core/tests/movement.rs`): determinism,
   frame-rate independence, no wall penetration, landing, stairs, ramps.
   These must survive every rules change.
2. **Rule tests:** placeholder numbers (run speed reached, jump apex = 56 u).
   Update these when rules become measured Apex behaviour.
3. **Golden traces:** `trace-diff reference.csv port.csv` joins on a key
   and classifies every value as matching / divergent / missing / extra,
   printing the first divergent tick (fix that one; the rest is cascade).
   Reference traces come from R5Flowstate `[MOVE-TRACE]` captures.
