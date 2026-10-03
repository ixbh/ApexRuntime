# Capturing ground-truth movement from R5Flowstate

Local install: launcher `F:\R5Flowstate\launcher`, game content
`F:\R5Flowstate\game` (see `tools/local-env.ps1`). Private/offline use only.

## How the trace works (from s21-r5sdk source)

| | |
|---|---|
| Unlock | launch the game with **`-devsdk`** (removes `FCVAR_DEVELOPMENTONLY`, turns on file logs) |
| Enable | console: **`bridge_move_trace 1`** (every command), `2` = only state changes + a heartbeat every `bridge_move_trace_heartbeat` cmds |
| Re-predictions | client only: `bridge_move_trace_replays 1` also logs re-predicted cmds (tagged `rp=1`) |
| Where | `<game>\platform\logs\<role>\<session-uuid>\message.log` (spdlog, timestamped) |
| Line | `[MOVE-TRACE] cmd= ent= ms= spr= duck= slide= ge= fwd= sd= b= msc= o=xyz v=xyz po=xyz pv=xyz [rp=1]` |
| Not logged | **view angles, per-command frame time** |

Both client and dedicated-server halves log. "Must match on client and dedi."
In a local match, set it on both (server console or `sv` passthrough; verify).

Convert: `cargo run -p apex-telemetry --bin flowstate-trace -- <message.log> --out traces/fs-<test>.csv`

## Tomorrow's checklist

1. [ ] Launcher finished; game content in `F:\R5Flowstate\game`.
2. [ ] Find how the launcher passes launch options; add `-devsdk`.
3. [ ] Start a private / local match on a flat map (Firing Range equivalent if present).
4. [ ] Console: `bridge_move_trace 1`. Check that `[MOVE-TRACE]` lines appear.
5. [ ] Locate `message.log`; run `flowstate-trace` on it; confirm `client`/`server` side detection
   (adjust `flowstate.rs` prefix matching if both show `unknown`).
6. [ ] Note the S21 class/legend used (settings vary per legend).

## Test protocol (one log per test, keep yaw fixed, note FPS cap)

Lock FPS first (`fps_max 60`, then repeat key tests at 144) so per-command dt is known.

| # | test | what it measures |
|---|---|---|
| T1 | stand still 3 s | baseline, `ms`, hull rest height |
| T2 | hold W (walk/run) 3 s, release, wait 2 s | ground accel curve, top speed, friction decay |
| T3 | same with sprint | sprint speed, accel |
| T4 | crouch-walk | crouch speed |
| T5 | jump in place ×3 | jump impulse, gravity, airtime |
| T6 | sprint + jump, no input in air | air drag (if any), landing speed loss |
| T7 | sprint → crouch (slide) on flat, no input | slide boost, slide decay, exit speed |
| T8 | slide jump | speed preservation |
| T9 | sprint, jump, tap A once at varying times after takeoff | lurch window + strength (needs yaw: keep mouse still) |
| T10 | bhop chain ×5 | speed preservation across hops |

Gravity and dt can be cross-checked: in free air `pv.z − v.z = −g·dt`.

## Session 4 protocol (mantle, superglide, wall climb, fatigue)

Same setup; copy `platform/logs/client/<session>/message*.log` to
`traces/raw/` right after (logs rotate at 10 MB and the oldest is deleted).

| # | test | measures |
|---|---|---|
| 1 | mantle onto one ledge, then STOP and check the log | does MOVE-TRACE log during TraversalMove? |
| 2 | mantle 3-4 ledge heights (waist/chest/head/max), 2 each, walking + sprinting | mantle height range, duration, exit speed |
| 3 | 10+ superglide attempts on one ledge (count your successes); repeat at fps_max 144 | superglide window, boost, fps dependence |
| 4 | sprint into a tall wall, hold W until the climb ends, x3 | wall-climb speed/height/duration |
| 5 | climb, jump off mid-climb, x3 | wall bounce |
| 6 | slide from walk (no sprint), half speed, sprint, x2 each | slide start threshold |
| 7 | slide-jump, land holding W without crouch, x3 | landing over-speed snap |
| 8 | standing bhop chain (no crouch) x6 hops, rest 5 s, repeat | jump fatigue (standing) |
| 9 | slide-jump chain x6, rest 5 s, then ONE slide-jump | jump fatigue (slide) + fresh slide-jump speed |
| 10 | single jump after 2 s / 1 s / 0.5 s rest (3 each) | fatigue recovery curve |
| 11 | standing still: spam jump as fast as possible x8 (no crouch), rest 5 s | jump fatigue baseline, no movement |
| 12 | standing still: hold crouch the whole time and spam jump x8, rest 5 s | crouch-held jump fatigue (differs from 11) |
| 13 | sprinting: hold crouch and spam jump x8 | crouch-held jump spam while moving (slide/jump interaction) |

Jump fatigue is NOT implemented until tests 8-13 are analysed together.
