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
2. **Asset pipeline + Apex viewmodel** — working (IW4L 83869e5..563abaa,
   see docs/ASSET-PIPELINE.md). `apex-cast` reads RSX Cast exports;
   `render_anim::anim::apex_viewmodel` replaces MW2's gun/arms with the
   R-99 + Wattson arms (`IW4L_APEX_CONTENT` in `.env`), Apex's own
   sequences driven by Apex movement state; `render_gpu` draws it in its
   own pass (viewmodel depth band, before post). Apex sequences are full
   poses (sprint, sprintslide, lower, reload, ads_in) or additive layers
   (idle, fire, jump, land) on the hip pose = first frame of `ads_in`.
   Framing live-tunable in `<content>/viewmodel.txt` (defaults fov 72,
   right 1.5, down 1.0 — user approved). Untextured until the HD texture
   pack finishes downloading (RSX crashes on missing `.opt.starpak` data).
   R-99 weapon definition exported (`weapons/r99/settings`): zoom_fov 60,
   sway/bob/jolt values for stage 3.
3. **Apex guns** - our engine (IW4L 70563be..7cbc3e0). 29-gun roster from
   `<content>/guns.json` (ApexRuntime `tools/export_guns.py`), each on its own
   MW2 carrier weapon (`asset_game::apex_weapons`); loadout menu on B
   (`console::apex_menu`, gives the carrier). Shooting: `sim::apex_guns` runs
   `apex-weapons` per command (fire modes, cadence, pellets/spread, recoil
   index), MW2's weapon step no longer fires; server flies projectiles and
   sends each tick's segment through MW2's lag-compensated hit test
   (`Emission::travelled` keeps falloff). MW2 keeps magazine/reload timing,
   ADS, switching on the Apex numbers. Verified: bots kill with Apex damage
   (Spitfire 18/hit). Open: L-STAR heat, Charge Rifle beam, Bocek draw
   anims; projectile tracers (client tracer is MW2's instant one); sounds
   for 26 guns (recipes only R-99/PK/Wingman); blast-pattern units and drag
   model are estimates (wall test); exact spray size; shields.
4. Characters + animations driven by Apex movement (slide/climb/mantle poses).
5. Abilities (Pathfinder grapple, Octane pad first).
6. Items/loot, Apex HUD, shields/heals, SFX.
Known gaps: no slide/climb/mantle animations yet (IW4 anims follow MW2
movement); MW2 ladders removed with its movement.
