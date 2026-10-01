# Movement measurements

The ledger of every number the runtime uses or will use. A value graduates
from *placeholder* only when it has a source, a method and a build.

**Confidence:** `none` (guess) · `low` (lineage default / old build) ·
`medium` (credible source, different build) · `high` (measured on the target build).
**Kind:** `measured` (we observed it) · `sourced` (read from config/code) ·
`inferred` (derived) · `placeholder`.

Target build for "current Apex": Steam `25522636`,
`R5pc_r5-301_J44_CL11595978` (v3.0.2.44, 2026-09-23).

## Values in use (`MovementConfig::PLACEHOLDER`)

| quantity | value | kind | source | build | confidence |
|---|---|---|---|---|---|
| gravity | 750 u/s² | inferred | Titanfall/R5 `sv_gravity` convention | S3 | low |
| jump height | 56 u | sourced | Flowstate bridge note: S3 `player_jumpHeight` = 56 | S3 | medium (S3) / low (live) |
| run speed | 200 u/s | placeholder | – | – | none |
| sprint speed | 300 u/s | placeholder | – | – | none |
| crouch speed | 100 u/s | placeholder | – | – | none |
| ground accel | 10 | placeholder | Source `sv_accelerate` | – | low |
| friction | 4 | placeholder | Source `sv_friction` | – | low |
| stop speed | 100 u/s | placeholder | Source `sv_stopspeed` | – | low |
| air accel | 10 | placeholder | Source `sv_airaccelerate` | – | low |
| air wish cap | 30 u/s | placeholder | Source | – | low |
| step height | 18 u | placeholder | Source | – | low |
| walkable normal z | 0.7 | placeholder | Source | – | low |
| standing hull | 32×32×72 | placeholder | Source-like | – | none |
| crouch hull | 32×32×48 | placeholder | – | – | none |
| eye height stand/crouch | 64 / 40 | placeholder | – | – | none |

## Values known but not yet used

| quantity | value | kind | source | build | confidence |
|---|---|---|---|---|---|
| lurch speed cutoff (`jump_grace_cutoff_speed`) | 1100 u/s horizontal | sourced | `s21-r5sdk/src/game/server/tapstrafe.cpp` convar doc | S21 | medium |
| superglide min traversal progress | 0.5 | sourced | `s21-r5sdk` mantle_boost convar doc | S21 | medium |
| superglide sprint exit multiplier | 1.5 (Sparrow 1.0) | sourced | same (`player_mantleBoostSprintSpeedMultiplier`) | S21 | medium |
| slide super-jump window | 0.3 s (mod-gated) | sourced | `slide_super_jump.cpp` | S21 | medium, not base movement |

## Still to measure

walk/run/sprint/crouch speed · ground acceleration curve · friction decay ·
gravity · jump impulse/apex · air acceleration and air speed cap ·
slide entry speed threshold, boost, decay and cooldown · lurch window,
strength and speed cost · tap-strafe response per press · wall-climb
timing/speed · mantle heights and duration · superglide window vs FPS ·
crouch transition time · hull sizes · step height.

## Measurement plan

**Never on live online servers with tools attached.** Live Apex is
protected by Easy Anti-Cheat; memory reading or injection there is out of
scope and against the rules of this project.

1. **Video analysis of the live game (allowed, passive).** Firing Range,
   capture at a fixed high FPS (e.g. 240 fps, OBS CFR). Use known geometry
   (Firing Range distance markers, known wall heights) as rulers.
   - Speeds: time to cover marked distances, frame-counted.
   - Jump apex/airtime: frame count from takeoff to landing; height against
     a known wall. Solve g and v₀ from airtime + apex (two equations).
   - Slide: speed-vs-time curve from marker crossings.
   - Lurch: **manual** keypresses (no macros; they can break Apex's ToS even
     in the Firing Range), recorded alongside an on-screen key-press overlay
     so each press has a frame timestamp. Keep yaw fixed and measure the
     heading change per press against time since takeoff. Many repetitions
     average out human timing. Precise scripted inputs belong in step 2 (R5).
   Each result → this table with `measured`, a link to the clip/notes, and
   an error bar.
2. **R5Flowstate private server (S21 client, controlled).** Install
   R5Flowstate (user action). Its `[MOVE-TRACE]` per-command dump
   (`move_sim_trace`) logs `FullWalkMove` state, which is a ready-made ground
   truth in our `TraceRow` shape. Write `tools/flowstate-trace-to-csv` to
   convert it, then run the same `InputScript` here and compare with
   `apex_telemetry::first_divergence`. This gives S21 behaviour. Diff it
   against live video to see what changed since S21.
3. **Public community analyses** (frame-by-frame lurch/superglide studies,
   patch notes for the lurch/tap-strafe nerfs). Log each as `low` with a link.
4. **Owned install, read-only** (later): player settings live in the
   game's RPaks. Reading them from the owned install with R5Flowstate
   tooling (`rsx`, run as an external program) could give exact `.set`
   values for the live build. Investigate format/legal aspects first; never
   commit extracted data.
