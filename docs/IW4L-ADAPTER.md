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

## Next

2. **Hook it into IW4L (a local branch of IW4L, not upstream yet).** In
   `sim::step`, behind a cvar/feature `apex_movement`: keep a per-player
   `apex_core::MovementState` (an ECS component next to the player row),
   call `adopt_from_playerstate` -> `apex_core::step(…, Iw4Collision::new(&backend), cmd_dt)`
   -> `sync_to_playerstate`, instead of `pmove`. Keep IW4's view-angle,
   weapon, ADS and event code running around it.
3. **Prediction / networking.** The client predicts with the same step, so
   the apex state must survive snapshot adoption: either add the few fields
   that matter (lurch clock, slide/coyote/stun timers, climb/mantle) to what
   IW4L snapshots, or re-derive them. This is the main engineering job.
4. **Game feel glue:** jump/land events and footsteps, sprint flag for weapon
   lowering, mantle/climb animations (IW4 has a mantle anim set), HUD speed.
   Prone and IW4 ladders disabled or mapped.
5. **Playtest** on MW2 maps (Rust, Terminal, Highrise…), tune what the maps
   need (hull height, ledge heights vs Apex mantle limits).

## Open questions for the user
- Do you own MW2 (2009) multiplayer on PC? IW4L needs its files.
- Local-only fork for you, or eventually a contribution upstream?
