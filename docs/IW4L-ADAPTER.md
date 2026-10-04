# IW4L adapter plan

Goal: play MW2 maps in [IW4L](https://github.com/vladtrc/iw4L) (open-source
MW2 runtime, Rust + Bevy 0.19, Apache-2.0) with Apex movement from
`apex-core`. IW4L reads a copy of MW2 multiplayer you own; it ships no assets.

## Where it plugs in (IW4L commit bfe3c2a)

- `crates/sim/src/step.rs` calls `movement_iw4::pmove(ps, &mut cmd, context,
  &backend, …)` once per player per usercmd. Server authority, client
  prediction and replay all run this same `sim::step` (docs/SIM-STEP.md there).
- `backend: ClipBackend` implements `movement_iw4::CollisionBackend::trace`
  (box sweep, start/end/mins/maxs/mask -> fraction, normal, startsolid), the
  same shape as our `CollisionWorld`. Player mask `0x0281_0011`, hull
  ±15 x 70 (crouch 50). Units: inches, Z-up, like ours.
- Input: `UserCmd` (forwardmove/rightmove ±127, buttons JUMP 0x400, CROUCH
  0x200, SPRINT 0x2) + `PlayerState::viewangles`.

## Done (milestone 1): `crates/apex-iw4l`

Depends on IW4L's `movement_iw4`, `playerstate_iw4`, `trace_iw4` by git URL,
pinned to the commit above (no local paths).
- `Iw4Collision` — `CollisionWorld` over any IW4L `CollisionBackend`.
- `input_from_cmd`, `cmd_dt` — usercmd -> `MovementInput`, per-cmd dt.
- `config_for_iw4` — IW4 hull sizes and view heights.
- `sync_to_playerstate` / `adopt_from_playerstate` — origin, velocity,
  ground entity, crouch/mantle flags, view height; host teleports adopted.
- Tests run apex movement through a stand-in IW4L backend (settle, sprint to
  299, jump, playerState sync).

## Done (milestones 2-3, IW4L working copy `F:\Dev\iw4l-apex`, branch `apex-movement`)

`C:\Dev\...` are junctions to `F:\Dev\...` (moved for disk space).
- **Apex replaces IW4 movement entirely** (`IW4L_APEX_MOVEMENT=1`):
  `crates/sim/src/apex.rs::pmove` runs instead of `movement_iw4::pmove` for
  normal players; IW4 keeps only view angles, ADS, hold breath, weapon sprint
  state, timers, footsteps. Other pm_types (spectator, dead, linked, last
  stand) still run IW4.
- **Prediction rewind**: per-client Apex state history by command_time,
  restored when the client adopts a snapshot. Scripted run: 81 prediction
  deviations (max 26.9 u) -> 2 (max 2.2 u). Fixed the rubber-banding.
- **Input**: wheel notches count as a full frame (scroll-up +forward taps
  move -> tap strafing); Apex auto-sprint on (`IW4L_APEX_AUTOSPRINT=0` gives
  press-to-sprint latch, never MW2 hold-to-sprint).
- **Player settings** (`target\play\iw4l-artifacts\settings.cfg`, not in
  git): fov 120; sensitivity 0.600 (= Apex 1.63: IW4 scales hipfire by
  tan(fov/2)/tan(32.5 deg) = 2.72 at 120); MWHEELUP +forward, MWHEELDOWN
  +gostand, CTRL/C +movedown, L togglecrouch.
- **Run**: `.\run-apex.ps1` (Rust, no bots; `-Map`, `-Bots N`, `-Classic`,
  `-Build`). `.env` holds `IW4L_GAMES=D:\SteamLibrary\steamapps\common\Call of Duty Modern Warfare 2`.
  PowerShell gotcha: Set-Location does not change a launched exe's cwd.

## Next (the "Apex inside MW2" plan; personal-use assets in `F:\ApexContent`)

1. **Feel without assets**: first pass done (IW4L bfbd79b). `sim::apex`
   publishes the move kind (slide/mantle/climb) + mantle progress in the
   unused IW4 mantle fields (`playerstate_iw4::apex_view`);
   `render_anim::anim::apex_feel` adds slide lean + gun cant, mantle pitch
   dip + dropped gun, climb bob + lowered gun, landing camera punch, FOV
   boost at speed (sens unchanged). Amplitudes hand-tuned in `Tuning`;
   `IW4L_APEX_FEEL=0` disables. Tune from play tests.
2. **Asset pipeline**: primary source is the offline Flowstate install
   (`F:\R5Flowstate\paks\Win64`, modern content), exported by the user
   with a community tool into `F:\ApexContent`; never the Steam install
   (fallback: copy one file out with the game closed). Sketchfab glTFs
   optional for early tests.
3. Apex gun behaviour (fire rate, recoil, damage, mags, attachments).
4. Characters + animations driven by Apex movement (slide/climb/mantle poses).
5. Abilities (Pathfinder grapple, Octane pad first).
6. Items/loot, Apex HUD, shields/heals, SFX.
Known gaps: no slide/climb/mantle animations yet (IW4 anims follow MW2
movement); MW2 ladders removed with its movement.
