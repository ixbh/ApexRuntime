# R5 / Flowstate research — where Apex movement lives

Status: first pass (2026-10-01). Sources: `r5sdk@c49e0d2` (Season 3),
`s21-r5sdk@71d365a` / `s21-platform@db0d226` (Season 21 bridge). Both are
reference-only (see `LICENSE-REFERENCES.md`). Nothing below is copied code.

## The three Apex builds in play

| build | what it is | how we see it |
|---|---|---|
| **S3** `r5apex_ds.exe` | The 2019-era build R5Reloaded/R5SDK target. Its dedicated server is what Flowstate still runs. | R5SDK headers (reversed structs, function signatures) |
| **S21** `r5apex.exe` | Retail client from ~2024. Flowstate runs it as the client. | `s21-r5sdk` client half + the bridge detours on the S3 server |
| **Live** (this machine) | Steam build `25522636`, `R5pc_r5-301_J44_CL11595978` (gameversion v3.0.2.44, 2026-09-23). About Season 30. | Observation only. Not moddable; protected by EAAC. |

**Key insight:** s21-r5sdk is a *bridge*. The S21 client predicts movement
with S21 rules and the S3 dedicated server simulates with S3 rules. Every
place they disagree causes a misprediction, so Flowstate added a server
detour for it. **Those detours list the S3→S21 movement changes**, each with
a comment explaining the S21 behaviour. That is the closest thing to an
S21 movement changelog we have.

## Where movement is computed

All movement is **native C++** in the game module. Apex is monolithic: the
client and server halves live in one exe, and the dedi binary contains both.

```text
CUserCmd (from client, per client frame)
  -> CPlayer::PhysicsSimulate        (server: drains queued usercmds)
  -> CPlayer::PlayerRunCommand
  -> CPlayerMove::RunCommand(player, ucmd, IMoveHelper*)
  -> CGameMovement::ProcessMovement  (fills CMoveData from the player)
       -> FullWalkMove / AirMove / … / TraversalMove (mantle) / wallrun / zipline …
  -> results copied back to CPlayer
```

- `CUserCmd` (S21 layout in `shared/usercmd.h`) has `command_number`,
  `tick_count`, `command_time`, `viewangles`, float `forwardmove` /
  `sidemove` / `upmove`, `buttons`, plus Apex extras (zipline vars, an
  extended cmd). **Analog float move axes, absolute view angles, and a
  per-command time.** Our `MovementInput` has the same shape on purpose.
- `CMoveData` holds per-command scratch: velocity at `mv+304` (S3) and
  `m_nButtonsPressed` at `mv+0x2C` (S3). Press **edges** are first-class;
  lurch is gated on them.
- `IMoveHelper` (`movehelper_server/client`) is the engine callback
  interface (traces, touch, sounds), the counterpart of our `CollisionWorld`.
- **Prediction:** the client runs the same `CGameMovement` on its own
  usercmds. Flowstate has `pred_authority*`, `pred_diag` and a per-command
  `[MOVE-TRACE]` dump hooked on `FullWalkMove` (`move_sim_trace.*`). That
  tracer is the model for our future trace-capture tooling.
- **Squirrel scripts do not run the per-tick movement.** They configure it:
  player settings (class `.set` files carrying `airSpeed`,
  `airAcceleration`, `jumpHeight`, `wallrun*`, `climb*`, `slide*`, …),
  status effects (speed scales), abilities (jump pads, launches, Octane
  double jump), and triggers (gravity lifts, updrafts). `cfg/speed.cfg` in
  s21-platform shows setting overrides by name: `airSpeed`,
  `slideSpeedBoost`, `slideSpeedBoostCap`, `landSlowdownDuration`,
  `player_slideBoostCooldown`, `wallrunJumpInputDirSpeed`.
- **Animation:** mantles/traversals are driven partly by animation data
  (`TraversalMove`, `m_traversalProgress`, baked traversal-camera curves in
  `shared/mantle_boost_curves.h`). This matters for superglide timing.

## Player state fields (S3, `r5sdk/src/game/server/player.h`)

Names indicate what the native code tracks (layouts are S3-specific):

- **Air:** `m_airSpeed`, `m_airAcceleration`, `m_lastJumpHeight`
- **Duck:** `m_duckState`, `m_DuckHullMin/Max`
- **Wall run / climb** (Titanfall heritage, mostly disabled in Apex but the
  climb is used): `m_wallRunStartTime`, `m_wallRunCount`,
  `m_wallrunFrictionScale`, `m_wallrunRetryTime/Pos/Normal`,
  `m_wallClimbSetUp`, `m_wallHanging`, `m_wallHangStartTime`
- **Traversal (mantle):** `m_traversalType`, `m_traversalState`,
  `m_traversalBegin/Mid/End`, `m_traversalProgress`,
  `m_traversalStartTime`, `m_traversalReleaseTime`,
  `m_traversalBlendOutStartTime`, `m_wallDangle*`
- **Slide:** `m_slideLongJumpAllowed`
- **Sprint:** `m_sprintStartedTime/Frac`, `m_sprintEndedTime/Frac`,
  `m_sprintTiltFrac`
- **Zipline:** `m_ziplineState`, `m_zipline` (`PlayerZiplineData`),
  `m_ziplineStartTime/EndTime`, `m_ziplineReverse`, …
- **Grapple, skydive, gravity** fields exist too (later phases).

## Mechanic-by-mechanic: what the bridge tells us about S21

| mechanic | native location | S21 facts (from bridge comments / convar docs) |
|---|---|---|
| **Lurch / tap-strafe** | Inline in `CGameMovement::AirMove`. S21 has a separate `AirMove_TapStrafe`. | Triggered by **newly pressed** directional bits (`IN_FORWARD/BACK/MOVELEFT/MOVERIGHT`) in `m_nButtonsPressed`. **S21 skips the lurch above `jump_grace_cutoff_speed`** (bridge default **1100 u/s** horizontal); S3 had no speed gate. Suppressed during mantle-boost state 4 and during gravity-cannon flights (which also restrict the wish direction). |
| **Superglide** ("mantle boost") | Hooked on `TraversalMove` (mantle). | FSM: 0 idle, 1 hang, 3 mantle jump (failed glide), 4 boost. Default activation input: **Jump**. The press counts only if the traversal is **past `min_valid_traversal_frac` = 0.5**, plus a view-pitch "sweet spot" derived from baked traversal-camera curves. Boost exit speed is multiplied by `player_mantleBoostSprintSpeedMultiplier` = **1.5** (29/30 legends; Sparrow 1.0). Failed glide uses `player_jumpHeight` = **56** (S3). The boost jump uses its own height with `sqrt(2·g·h)`. |
| **Slide super-jump** | Wraps `Jump`. | Only with `player_slideSuperJumpEnabled` (a "boosted_slide_jump" mod): after a jump from a slide, a jump/dodge press within **0.3 s** of leaving the ground adds `sqrt(2·g·slideSuperJumpHeight)` once per cooldown. Not base movement. |
| **Wall climb / wall run** | Native; settings fields `wallrunMaxSpeedVertical`, `wallrunAccelerateVertical`, `wallrunAllowedWallDistance`, `wallrun_timeLimit`, `wallrun_hangTimeLimit`, `wallrunUpWallBoost`, `wallrunSameWall*`, `wallrunCeilingLimit`, `climbEnabled`, `climbHeight`, `climbSpeedStart`, `climbFinalJumpUpHeight`. | The S3 and S21 settings layouts differ; Flowstate remaps offsets. The Apex wall climb is Titanfall's wall run with vertical-only tuning. |
| **Wall launch** (Sparrow) | `shared/wall_launch_math.h` | Legend-specific: high jump off a climb plus edge air control. Not base movement. Useful only as an example of air-control scaling. |
| **Zipline** | Native `ZiplineMove` + entity `c_zipline` | Cooldowns, exit parity, half-duck parity fixes, so S3/S21 differ. Later phase. |
| **Jump pads, gravity lifts, updrafts, trigger cannons** | Triggers + script | Later phase. Map features. |

## Constants spotted (S3/S21 context, not live)

| name | value | context |
|---|---|---|
| `player_jumpHeight` | 56 | S3 (bridge comment) |
| `player_mantleBoostSprintSpeedMultiplier` | 1.5 | S21 (bridge convar doc) |
| `jump_grace_cutoff_speed` (lurch gate) | 1100 u/s | S21 client value mirrored by `bridge_lurch_cutoff_speed` |
| superglide `min_valid_traversal_frac` | 0.5 | S21 |
| slide super-jump window | 0.3 s | S21, mod-gated |

All go into `MOVEMENT-MEASUREMENTS.md` at low/medium confidence until checked
against the live build.

## Open questions

1. Is `sv_gravity` still 750 on live, and is it player-scaled anywhere?
2. What are the full lurch formula and decay window? The bridge only shows the
   gate. The magnitude lives in native S21 code that is not documented in
   source. Get it by measurement (Phase 3).
3. Live Season 30 has had multiple tap-strafe / lurch nerfs since S21 (lurch
   strength, controller vs MnK). We need live measurement or patch notes.
4. Slide: initial boost, decay and cooldown (`player_slideBoostCooldown`,
   `slideSpeedBoost`, `slideSpeedBoostCap`): need values.
5. Mantle: heights, `TraversalMove` timing and exit velocities.
6. Movement runs per usercmd at **client frame rate** (`command_time`).
   Which mechanics depend on the frame rate (superglide certainly)?

## Next research steps

- Read the S21 client-half hooks in `s21-r5sdk/src/game/client/` for
  `AirMove_TapStrafe` details (offsets and gates only; no copying).
- Install R5Flowstate locally (user action) and use its `[MOVE-TRACE]`
  output on private servers as the first **ground-truth trace source**. It
  logs per-command `FullWalkMove` state, which matches our `TraceRow`.
- Collect public Apex movement analyses (community frame-by-frame lurch
  and superglide studies) and log them as low-confidence sources.
