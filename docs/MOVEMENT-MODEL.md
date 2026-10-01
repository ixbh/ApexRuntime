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

## Planned Apex mechanics (hypotheses to test)

These are **models to fit against measurements**, written so the parameters
are named and measurable. None is implemented yet.

### Lurch (the basis of tap-strafing)

Known (R5 research): triggered by a **new press** of a direction key while
airborne. S21 skips it above 1100 u/s horizontal speed. Some states suppress
it (superglide boost, cannon flight). It is part of `AirMove`, not a separate
"tap-strafe" feature.

Working model:

```text
on directional press edge, airborne, |v_h| ≤ v_cut, t_air ≤ T_lurch:
    k   = L_max · decay(t_air / T_lurch)          // strength fades after leaving ground
    v_h ← |v_h| · (1 − ρ·k) · normalize( lerp(v̂_h, ŵ, k) )
```

Parameters to measure: `T_lurch` (window), `L_max` (max redirect fraction),
`decay` shape, `ρ` (speed cost), `v_cut` = 1100 (S21; check live). It must
work per press edge, so many presses in a short window (scroll-wheel forward
binds) compound. That compounding is what tap-strafing *is*. Frame-rate
dependence: if `k` is applied per usercmd rather than per second, high FPS
changes the result. Test that explicitly.

### Slide

Enter: crouch while sprinting above a speed threshold. Gets an entry boost
(`slideSpeedBoost`, capped by `slideSpeedBoostCap`, cooldown
`player_slideBoostCooldown`). Reduced friction, steered by gravity on slopes.
Exit below a speed or on release. **Slide jump** = jump during a slide;
friction does not apply on the jump tick (same as a bhop).

### Mantle and superglide

Mantle = scripted `TraversalMove` with progress 0→1 over an animation.
Superglide = jump pressed after progress > 0.5 (plus a camera-pitch gate)
and within a frame-tight window → exit with boost (sprint mult 1.5) instead
of a normal mantle jump. Must be modelled per usercmd because the window is
about a frame long.

### Wall bounce / wall jump

Apex has no free wall-jump. "Wall bounces" are a jump input timed against
wall-climb/hang state. They come after wall climb is modelled.
