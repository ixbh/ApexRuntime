# Movement measurements

The ledger of every number the runtime uses or will use. A value graduates
from *placeholder* only when it has a source, a method and a build.

**Confidence:** `none` (guess) · `low` (lineage default / old build) ·
`medium` (credible source, different build) · `high` (measured on the target build).
**Kind:** `measured` (we observed it) · `sourced` (read from config/code) ·
`inferred` (derived) · `placeholder`.

Target build for "current Apex": Steam `25522636`,
`R5pc_r5-301_J44_CL11595978` (v3.0.2.44, 2026-09-23).

## The values themselves: `sheets/constants.tsv`

Every number lives in **`sheets/constants.tsv`** (one row per constant:
`id, type, value, unit, kind, confidence, build, used, source, notes`).
`apex-core/build.rs` validates it and generates the Rust constants, so the
sheet *is* the config. This doc does not repeat the values (one source of
truth). `cargo run -p apex-tables --bin preflight` prints which constants the
movement currently rests on, grouped by confidence.

To graduate a value: change `value`, set `kind=measured`,
`confidence`, `build` and a `source` pointing at the trace/clip, then run
preflight and the tests.

## Session 1 — R5Flowstate S21 client, 2026-10-02

Local offline match on `mp_rr_district`, `fps_max 60`, `bridge_move_trace 1`,
18,783 client commands, 10-test protocol (`FLOWSTATE-CAPTURE.md`) split by
triple-crouch markers (`research/analysis/seg.py`). Raw CSV:
`traces/flowstate-session1.csv` (local, not committed). All values are in
`sheets/constants.tsv` with `kind=measured`.

Model-level findings (what Apex does differently from Source):

- **Gravity 750, no air drag.** Per-command dt varies (≈1/60 in groups of
  3); dt is recoverable in air as `-(pv_z - v_z)/750`.
- **Jump** v0 = 283.35 u/s (rise 53.5 u), full gravity applied on the jump tick.
- **Ground acceleration is two-phase:** 3 ticks of +47.9/tick from
  standstill (to ≈0.72·max), then 517.5 u/s² net to run speed 199.525; sprint
  then ramps at only ~117 u/s² to 299 (0.8 s).
- **Friction is a constant 1250 u/s² deceleration** (linear, not
  exponential) for speeds ≤ 299. Reproduced with Source friction by
  `friction × stop_speed = 1250`.
- **Over-speed on ground snaps to 299** within 2–3 ticks; releasing
  forward while sprinting drops 299 → 120.7 in one tick (unexplained).
- **Crouch is not instant:** ≈0.38 s down, ≈0.22 s up. Crouch speed 92.
- **Slide:** +174.4 u/s on start, capped at 460, cooldown ≈2.2–2.6 s, decel
  ≈ 88.6 u/s² + 0.403·v, ends near 124 u/s.
- **Bhop:** jumping on the first grounded tick loses 0; each extra sliding
  tick costs ≈3.4 u/s.
- **Lurch:** present only early in the air (seen at 0.08–0.12 s, absent from
  0.32 s). Back press 299 → 82 (k ≈ 0.363 if `lerp(v, |v|·ŵ, k)`); forward taps
  rotate heading 17–21° with no speed loss (k ≈ 0.5–0.58). Model unresolved:
  needs a clean A/D test.
- **Button bits (S21):** 0x2 jump, 0x4 crouch, 0x8 forward, 0x10 back,
  0x400 side, 0x8000 sprint. The log's `fwd/sd` fields lag the buttons by 1 cmd.

Replay check (test 2, run + stop): 191/211 ticks within 1 u/s of the game;
the 20 divergent ticks are the unmodelled start kick.

## Still to measure

lurch (clean A/D taps at fixed times after takeoff; exact window and
strength model) · air acceleration / air speed cap (strafe with A/D held) ·
sprint-release drop (299 → 120.7) · slide entry speed threshold and exact
cooldown · start-kick threshold · frame-rate dependence (repeat at
`fps_max 144`) · wall-climb timing/speed · mantle heights and duration ·
superglide window · hull sizes · step height.

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
