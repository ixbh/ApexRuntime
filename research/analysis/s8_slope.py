"""Session 8 slope runs: per sliding tick, slope angle (from dz per horizontal
distance), 3D/horizontal speed and its rate of change; fit the slide decel
on slopes. usage: s8_slope.py <csv> <client log> <bot logs...>"""
import csv, json, math, re, sys, bisect
from datetime import datetime
import numpy as np
rows = [r for r in csv.DictReader(open(sys.argv[1])) if r['side'] == 'client']
f = lambda r, k: float(r[k])
for line in open(sys.argv[2], encoding='utf-8', errors='ignore'):
    m = re.match(r'\[(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d+)\] \[([\d.]+)\].*MOVE-TRACE', line)
    if m:
        wall0 = datetime.strptime(m.group(1), '%Y-%m-%d %H:%M:%S.%f').timestamp(); game0 = float(m.group(2)); break
times = [f(r, 'game_time') for r in rows]
X = []
for path in sys.argv[3:]:
    a = json.load(open(path))[0]
    tg = a['start_unix_ms'] / 1000 - wall0 + game0
    i = bisect.bisect_left(times, tg); end = bisect.bisect_left(times, tg + 7.5)
    seg = []
    for k in range(i + 1, end):
        r = rows[k]
        if r['slide'] != '1' or r['ground'] != '1' or int(r['cmd']) - int(rows[k - 1]['cmd']) != 1:
            continue
        dxy = math.hypot(f(r, 'po_x') - f(r, 'o_x'), f(r, 'po_y') - f(r, 'o_y'))
        dz = f(r, 'po_z') - f(r, 'o_z')
        if dxy < 0.3:
            continue
        dt = f(r, 'game_time') - f(rows[k - 1], 'game_time')
        h0 = math.hypot(f(r, 'v_x'), f(r, 'v_y')); h1 = math.hypot(f(r, 'pv_x'), f(r, 'pv_y'))
        v0 = math.hypot(h0, f(r, 'v_z')); v1 = math.hypot(h1, f(r, 'pv_z'))
        ang = math.degrees(math.atan2(dz, dxy))     # + = uphill
        seg.append((f(r, 'game_time') - tg, ang, h0, h1, v0, v1, f(r, 'pv_z'), dt))
        X.append((ang, h0, h1, v0, v1, dt))
    if seg:
        down = [s for s in seg if s[1] < -2]; up = [s for s in seg if s[1] > 2]
        d = lambda L: f"{len(L)} ticks, angle {np.mean([s[1] for s in L]):+.1f}, |h| {L[0][2]:.0f}->{L[-1][3]:.0f}" if L else '-'
        print(f"{path[-18:-5]}: down {d(down)} | up {d(up)}")
A = np.array(X)
ang, h0, h1, v0, v1, dt = A.T
acc = (h1 - h0) / dt
flat = 98.9 + 0.3556 * h0
# residual vs flat decel, as a function of slope
res = acc + flat
s = np.sin(np.radians(ang))
c, *_ = np.linalg.lstsq(np.column_stack([s]), res, rcond=None)
print(f"horizontal accel = -(98.9 + 0.3556 v) + {c[0]:.1f} * sin(angle)   rms {np.sqrt(np.mean((res - c[0]*s)**2)):.1f}  n={len(s)}")
for lo, hi in ((-30, -15), (-15, -8), (-8, -3), (-3, 3), (3, 8), (8, 15), (15, 30)):
    m = (ang >= lo) & (ang < hi)
    if m.sum():
        print(f"  angle {lo:+3d}..{hi:+3d}: n {m.sum():4d}  measured accel {np.mean(acc[m]):8.1f}  flat model {np.mean(-flat[m]):8.1f}  residual {np.mean(res[m]):7.1f}  speed {np.mean(h0[m]):5.0f}")
