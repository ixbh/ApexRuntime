# Movement model

What `apex_core::step` computes, as maths. **Phase 1 is a Source-lineage
baseline, not Apex.** Each section ends with what Apex is known or suspected
to do differently. Units: u, s, u/s. Z up.

## Per-tick pipeline (Phase 1)

```text
1. wish        f, r = yaw basis (horizontal);  w = F·f + R·r  (F, R ∈ [-1,1])
               m = min(|w|, 1);  ŵ = w/|w|;  W = m · Vmax(state, buttons)
2. crouch      instant hull swap; airborne crouch raises feet; standing needs headroom
3. categorize  grounded ⇔ v_z ≤ 140 ∧ hull sweep 2 u down hits a plane with n_z ≥ 0.7
4. jump        on a *press edge* while grounded: v_z = √(2·g·h), become airborne
               (happens BEFORE friction → a perfectly timed hop loses no speed)
5a. ground     friction → accelerate(ŵ, W, a_g) → step-slide-move → stick to ground
5b. air        v_z −= ½·g·dt → air-accelerate → slide-move → v_z −= ½·g·dt
6. categorize  again; grounded ⇒ v_z = 0
```

`Vmax`: crouch 100, sprint 300 (forward input + sprint held), else run 200.
**Placeholders.**

## Friction (Source)

```text
s = |v|,  c = max(s, s_stop),  s' = max(s − c·μ·dt, 0),  v ← v · s'/s
```
μ = 4, s_stop = 100. Below `s_stop` the deceleration is constant (μ·s_stop),
so stopping takes finite time.

## Acceleration (Quake/Source)

```text
add = W − v·ŵ                        (only the projection onto ŵ is capped)
if add > 0:  v ← v + ŵ · min(a·dt·W, add)
```
On the ground this converges to |v| = W along ŵ.

## Air acceleration (Source)

```text
add = min(W, W_cap) − v·ŵ,     W_cap = 30
if add > 0:  v ← v + ŵ · min(a_air·W·dt, add)
```
The *test* uses the capped wish speed and the *step* uses the full one. When
ŵ is nearly perpendicular to v, v·ŵ ≈ 0, so every tick adds up to 30 u/s
sideways and |v| grows. That is classic air strafing.
**Apex:** air control is far weaker and tuned per class (`airSpeed`,
`airAcceleration` in player settings). Bhop speed gain is not an Apex thing,
but speed *preservation* through hops is. To be measured.

## Gravity integration

Gravity is split around the move (½ before, ½ after). For constant g the
position update becomes `x += (v − ½g·dt)·dt`, which is exact at tick
boundaries. Jump apex = `v₀²/2g` = h exactly; tested. Airtime is `2v₀/g`
(≈ 0.773 s for h = 56, g = 750).

## Collision response

- **Slide move** (Source `TryPlayerMove`): up to 4 sweeps. On each hit,
  clip v against the plane (`v − n(v·n)`). With multiple planes, find a
  clip valid for all of them, else slide along the crease of two, else stop.
  If v turns against its original direction, stop (no corner jitter).
- **Step move:** compare the plain slide with lift by 18 u → slide → drop.
  Keep the one that went further horizontally. The drop must land on a
  walkable plane.
- **Stick to ground:** after a ground move, snap down up to 18 u to the
  floor if there is one (walk down stairs/slopes without going airborne).

## Apex rules implemented (fitted to R5Flowstate S21 traces)

All numbers live in `sheets/constants.tsv`; golden tests in
`crates/apex-core/tests/apex_golden.rs`. Replays of the recorded run,
sprint and slide tests match the game on every tick (`research/analysis/`).

### Ground acceleration (net rates, fps-independent)

```text
net = 2875        if |v| < 0.72·run_speed   (start kick)
      517.5       if |v| < run_speed (199.525)
      115         otherwise (run → sprint 299 ramp)
amount = net·dt + friction_removed_this_tick      // rates are measured net of friction
v += ŵ · min(amount, W − v·ŵ)
```

### Friction
Constant deceleration 1250 u/s² (Source friction with friction·stop_speed =
1250), ×1.2 = 1500 u/s² when fully crouched.

### Jump (fps-dependent)
V = √(2·750·56) = 289.8. On the jump tick v_z is set to V − ½g·dt before the
split-gravity air move, so the position advances (V − g·dt)·dt and the tick
ends at V − 1.5·g·dt. Rise: 53.5 @60 fps, 55.0 @144 fps.

No jump buffer: the press must arrive on a tick that *starts* on the ground
(the first tick after the landing tick). A press on the landing tick itself
is lost (0 of 23 such presses jumped in S21 traces). Jumping happens before
friction, so a first-ground-tick jump loses nothing to friction.

### Bunnyhop penalty (S21 traces; wiki "skip_time")
A jump taken within `bhop_skip_time` = 1.0 s of landing cuts horizontal
speed above `sprint_speed` (299) by up to `bhop_speed_penalty` = 100, never
below 299: 443.3 → 343.3, 460 → 360, 337 → 299, 297 kept; jumps at 1.13 s+
keep their speed. The slide long-jump raise to 350 only applies outside the
window (300.9 → 299 at 0.8 s). This also explains the old "−100 on landing"
and "354 → 299" puzzles.

### Jump fatigue (session 4, 361 jumps, rms 2.9 u/s)
After *landing from a jump* (falls, walk-offs and mantles do not count), the
next jump's height is `0.30 × jump_height` if taken within 0.14 s on the
ground, recovering linearly to full height at 0.735 s. Holding crouch does
not change it. Jumps out of a slide multiply launch speed by 0.944 (fresh
273.5, fatigued 150.0). Landing with crouch held re-enters the slide on the
landing tick, so spam-jumping while crouched at speed chains slide-jumps.

### Crouch
Ground: `crouch_frac` 0→1 in 0.383 s, 1→0 in 0.217 s; max speed blends
run/sprint → 92; crouched hull once fully down; standing needs headroom.
Air: instant.

### Slide
Entry: on ground, crouch held and newly pressed (or just landed), |v| ≥
`slide_start_min_speed` (placeholder 110). Boost +174.4 capped at 460, not
again within 2.4 s. While sliding: no friction/accel, decel = 98.9 + 0.3556·|v|
u/s². Ends when crouch is released or |v| < 125. Jumping out and landing with
crouch held re-enters the slide (slide-hop), usually without boost.

### Lurch (the basis of tap-strafing)

```text
on a NEW directional press while airborne (keys held at takeoff don't count),
if |v_h| ≤ 1100:
    t   = time since the first airborne tick (jump tick = 0)
    k   = 0.492                          t ≤ 0.278
          0.492·(0.498 − t)/(0.220)      0.278 < t < 0.498
          0                              otherwise
    v_h ← |v_h| · normalize((1 − k)·v̂_h + k·ŵ)      // turn, keep speed
    (no normal air acceleration on this tick)
```

Same per press at 60 and 144 fps. Tap-strafing with a scroll-bound forward is
many presses, each lurching toward the camera. Within ~1° of 27 recorded
presses. **Unresolved:** presses made while W is still held from the sprint
turn more than the model (k ≈ 0.6–0.7 for a 45° wish); a speed loss seen on
some presses at 0.15–0.25 s; the very early back-press 299 → 82 (session 1).

### Air control
Source air acceleration with `air_accel` 2.48 (×199.525 air max speed =
495 u/s²) and wish cap 70 u/s: holding A/D with a fixed camera gains until
v·ŵ = 70, then nothing; moving the mouse keeps ŵ ahead of v (air strafing).

## Still hypotheses

### Mantle and superglide

Mantle = scripted `TraversalMove` with progress 0→1 over an animation.
Superglide = jump pressed after progress > 0.5 (plus a camera-pitch gate)
and within a frame-tight window → exit with boost (sprint mult 1.5) instead
of a normal mantle jump. Must be modelled per usercmd because the window is
about a frame long.

### Wall bounce / wall jump

Apex has no free wall-jump. "Wall bounces" are a jump input timed against
wall-climb/hang state. They come after wall climb is modelled.

## Traversal (implemented: `crates/apex-core/src/traverse.rs`)

Checked against the S21 traces with geometry-matched replays
(`research/analysis/climb_check.py`, `mantle_check.py`) and golden tests
(`tests/traversal_golden.rs`).

### Mantle
Triggered at the **start of a tick** after the player touched a wall on the
previous tick and is pushing into it: any input toward it (A/D or a scroll-W
tap count, W is not required), moving into it, or last tick's impact
(> 10 u/s). Sideways speed along the wall is **carried through the mantle
while A/D is held that way** (otherwise it bleeds off at 1250 u/s²) and kept
on the exit, so angled mantles flow instead of snapping straight. Needs a ledge with standing room `mantle_forward`
(19 u) past the wall, `step_height < height ≤ 80` above the feet (≤ 60
from the ground). Scripted up-and-over arc onto the
ledge over `0.162 + 0.00659 × height` s (the game does not log inside a
mantle; the arc may overlap the ledge edge like Apex's animation-driven
traversal, and a jump-off mid-mantle steps back out to the wall plane). Exit: forward × current max
speed if W is held, else standing still. Mantles don't cause jump fatigue.
Replays: 37/39 recorded mantles reproduced (the other 2 start from a wall
climb the replay doesn't cover), end height within 1 u on 36, duration error
+0.003..+0.008 s mean (game spread 0.023 s), start within a tick.

### Jumping during a mantle
* in the **grab** (before the last `mantle_pull_time` = 0.3 s, ledges taller
  than 42 u only) → jump off (full jump, no horizontal speed; S21 traces);
* in the **pull** (last 0.3 s, or any time on a ≤ 42 u ledge) but before the
  window → nothing happens (wiki: "if you jump too early you simply won't
  jump"), so scroll-spamming into a ledge still mantles;
* in the last 0.15 s, **crouch exactly one tick later** → **classic
  superglide**: a slide-jump from the ledge top, `min(base + 174.4, 460)`
  horizontal + full jump (base = sprint/run by input). Tick-based, so the
  jump→crouch gap is one frame: easy at 30 fps, 6 ms at 165 fps;
* in the last 0.15 s without that crouch → **S21 mantle boost**:
  1.3 × base horizontal, 90 u jump (`mantle_boost_enabled`; the real S21 gate
  also needs a view-pitch sweet spot, not modelled).

### Wall climb
Airborne, pushing into a wall, once per airtime. Contact does not need W
(recorded bounces attached with only A held); **climbing up needs W**:
without it the contact only lasts the 0.1 s bounce window. Decided **mid-tick**: if
`vz − ½·g·dt ≤ 90` the climb attaches on that tick (second half of gravity
skipped), then vz rises 505 u/s² to 182.5, then decays `exp(−1.753·t)`.
After 51 climb ticks (0.85 s) the next tick pushes 255.5 up / 61.7 away.
Reaching a ledge during the climb mantles over it. Replays: 3 recorded
climbs match every tick (81/81 within 5 u/s, worst 1.3); climbs with A held
differ (A/D lengthen the climb and strengthen the push: not modelled).

**Climb kinds by input (apexmovement.tech wiki, 2026-10-04):** the input
relative to the wall decides the climb each tick.
* W into the wall: the measured upward climb above; along-the-wall speed ≤ 115.
* A/D pointing into the wall (view turned slightly away): **sideways climb /
  wallrun**: along-the-wall speed → 258 (faster carried speed kept), falling
  is held off and upward speed runs out under gravity, so the run stays level.
* A/D pointing away from the wall: slip (gravity only).
* Non-upward climbing (vz < 10) lets go 1 s after attaching.
* Crouch, Back, or looking > 45.57° from the wall (arccos 0.7) detach; the
  look limit also gates attaching.
* A climb turns into a mantle only with input within 50° of the wall (else a
  "No-ma climb"). Speed along the wall is carried into the climb and out of
  wall jumps and the end push. The player is pushed into the wall each tick
  (`climb_suction`) and follows the wall's surface.

**Climb space and zones (wiki + S21, implemented 2026-10-04).** Baseline =
feet at the last ground contact (dragged down when falling below it).
* The climb ends with the end push at **147 u above the baseline** (7/7 S21
  W climbs end at 147.2-147.6; the "51 ticks" was the time to get there) or
  100 u above the attach point.
* Wall jump by zone: **mini** (0-19) +28.21 u hop, 188 out; **green**
  (19-47) vz = sqrt(244864 − 3855.5·z) (9 S21 bounces, rms 4.5 u/s; the old
  contact-time ramp had 15.5); **neutral** keeps the climb vz minus a tick
  of gravity, 258 out.
* Climb space drops 128 on a non-jump detach (crouch, back, look away, 1 s
  timer, end push) or a mini-zone jump, 256 on a green/neutral jump, 0 when
  the climb becomes a mantle or you slip off the side/top.
* Reattach in the air: below the last attach point, or on a wall whose
  normal differs by ≥ 25.8°. Attaching also works while falling.
* No input on the wall: slip (gravity), until the 1 s non-upward timer.
* Measured on the t=388.3 climb (W+A → A → W+A): along-the-wall speed rises
  114 u/s² while climbing up (to 115 × A/D share), 231 u/s² sideways (to
  258); sideways while rising dvz/dt = −750 + 1.5·vz; a late W re-press
  climbs only up to a decaying ceiling (game 95, ours 76: open).

### Wall jump / wall bounce
Jump while climbing: 258 u/s along the wall normal, plus 258 along the
wall toward the A/D input if held (bounce |h| = 365). vz: within 0.1 s of
contact 260 → 373 by contact time; later the climb speed minus one tick of
gravity. Replays: 2 of 3 recorded wall jumps match every tick.

### Not modelled yet
A/D during a climb, mantle-boost view-pitch gate, slopes (and the 350/400
slide clamps seen on slopes), the occasional 299 landing clamp, ziplines.

### Lurch window across traversal, superglide direction, No-ma (2026-10-04)
* The lurch clock restarts on every jump (ground, wall jump, mantle jump-off,
  superglide) and runs through mantles/climbs (wiki; S21: a press 0.017 s
  after a mantle jump-off turned 33°). The S21 mantle boost disables lurch
  until landing (S21: 0.0° at 0.25 s). Superglides keep it: tap strafe out.
* Superglide / mantle boost exit along the movement input (sideways
  superglide; no backwards) at sprint speed if sprinting into the mantle.
* A mantle starts only with no input or an input within 50° of the wall.
  A ledge in reach with the input further off (A/D, or W+A/D turned 5°+)
  is the "no-mantle state": the climb stops and hangs, for No-ma bounces.
  Which zone the hang ends in on a 48 u block is OPEN (session 6).

### Coyote time (wiki + S21, 2026-10-04)
After walking/sliding off the ground without a jump, a jump still works for
0.2 s. It adds a full, never-fatigued jump to the current vertical speed
(S21: all 10 coyote jumps end at vz_in + V − 1.5·g·dt; −31.2 → +239.9,
−131.2 → +139.8), ×0.944 if you left in a slide (V 273.9), and the bhop
penalty uses the ground time before leaving (361.8 → 299). A mantle or climb
ends it.
