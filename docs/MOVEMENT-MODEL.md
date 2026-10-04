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
