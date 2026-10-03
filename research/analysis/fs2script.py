"""Turn a slice of a Flowstate MOVE-TRACE CSV into an apex-replay input script
plus a reference CSV (tick, speed_h, vel_z, ...) for trace-diff.

usage: fs2script.py <flowstate.csv> <first_cmd> <last_cmd> <out_prefix> [tick_rate|real]

`real` uses each command's own dt: distance moved / post-move horizontal
speed when moving (exact to the log's 3 decimals), else the logged game clock
(ms resolution).

Inputs come from the S21 button bits (0x2 jump, 0x4 crouch, 0x8 fwd, 0x10 back,
0x200 right, 0x400 left) and the logged sprint flag (auto-sprint is not a
button). Yaw is not logged: the script uses yaw 0 and the start velocity is
rotated onto +X, so compare speeds, not headings.
"""
import csv, json, math, sys

src, c0, c1, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
mode = sys.argv[5] if len(sys.argv) > 5 else '60'
real = mode == 'real'
rate = 60.0 if real else float(mode)
allrows = list(csv.DictReader(open(src)))
i0 = next(i for i, r in enumerate(allrows) if int(r['cmd']) >= c0)
rows = [r for r in allrows[i0:] if int(r['cmd']) <= c1]
gt = [float(r['game_time']) for r in allrows[max(i0 - 1, 0):i0 + len(rows)]]
dts = [max(min(b - a, 0.1), 0.001) for a, b in zip(gt, gt[1:])] if i0 > 0 else None
if dts:
    for t, r in enumerate(rows):
        dx = float(r['po_x']) - float(r['o_x']); dy = float(r['po_y']) - float(r['o_y'])
        sp = math.hypot(float(r['pv_x']), float(r['pv_y']))
        if sp > 40:
            d = math.hypot(dx, dy) / sp
            if 0.002 < d < 0.05:
                dts[t] = d

def inputs(r):
    b = int(r['buttons'], 16)
    fwd = (1 if b & 0x8 else 0) - (1 if b & 0x10 else 0)
    right = (1 if b & 0x200 else 0) - (1 if b & 0x400 else 0)
    press = []
    if b & 0x2: press.append('jump')
    if b & 0x4: press.append('crouch')
    if int(r['sprint']) or b & 0x8000: press.append('sprint')
    return fwd, right, press

events, prev = [], None
for t, r in enumerate(rows):
    cur = inputs(r)
    ev = None
    if cur != prev:
        fwd, right, press = cur
        ev = {"tick": t, "forward": fwd, "right": right, "press": press,
              "release": [b for b in ("jump", "crouch", "sprint") if b not in press]}
        prev = cur
    if real and dts:
        ev = ev or {"tick": t}
        ev["dt"] = round(dts[t], 6)
    if ev:
        events.append(ev)

r0 = rows[0]
v0 = math.hypot(float(r0['v_x']), float(r0['v_y']))
script = {"name": f"fs-{c0}-{c1}", "description": f"from {src} cmds {c0}..{c1}",
          "tick_rate": rate, "ticks": len(rows),
          "start": {"position": [0, 0, 0], "velocity": [v0, 0, float(r0['v_z'])]},
          "events": events}
json.dump(script, open(out + '.input.json', 'w'), indent=1)
with open(out + '.ref.csv', 'w') as o:
    o.write('tick,speed_h,vel_z\n')
    for t, r in enumerate(rows):
        o.write(f"{t + 1},{r['speed_h']},{r['pv_z']}\n")
print(f"{len(rows)} ticks, {len(events)} input changes -> {out}.input.json / {out}.ref.csv")
