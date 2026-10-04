"""Session 7 (inputbot) known-angle lurches: for each attempt, the W notch's
turn and speed change vs the exact wish angle and time since the jump.
usage: s7_lurch.py <flowstate.csv> <client message.log> <bot attempt log>"""
import csv, json, math, re, sys
from datetime import datetime
rows = [r for r in csv.DictReader(open(sys.argv[1])) if r['side'] == 'client']
f = lambda r, k: float(r[k]); B = lambda r: int(r['buttons'], 16)
hd = lambda x, y: math.degrees(math.atan2(y, x)); wrap = lambda a: (a + 180) % 360 - 180
# wall clock <-> game time from the first MOVE-TRACE line
for line in open(sys.argv[2], encoding='utf-8', errors='ignore'):
    m = re.match(r'\[(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d+)\] \[([\d.]+)\].*MOVE-TRACE', line)
    if m:
        wall0 = datetime.strptime(m.group(1), '%Y-%m-%d %H:%M:%S.%f').timestamp(); game0 = float(m.group(2)); break
attempts = json.load(open(sys.argv[3]))
times = [f(r, 'game_time') for r in rows]
import bisect
out = []
for a in attempts:
    tg = a['start_unix_ms'] / 1000 - wall0 + game0
    ang, t_ms = int(re.search(r'view (\d+) deg', a['label']).group(1)), int(re.search(r'at (\d+) ms', a['label']).group(1))
    i = bisect.bisect_left(times, tg)
    # the jump: first tick in the attempt that leaves the ground with vz > 200
    while i < len(rows) and not (rows[i]['ground'] == '1' and f(rows[i], 'pv_z') > 200):
        i += 1
    jt = f(rows[i], 'game_time'); hj = hd(f(rows[i], 'pv_x'), f(rows[i], 'pv_y'))
    wish = hj - ang   # turned right by `ang` from the run direction (view = run direction)
    j = i + 1
    while j < len(rows) and rows[j]['ground'] == '0':
        q, p = rows[j], rows[j - 1]
        if B(q) & 8 and not B(p) & 8:
            v0 = hd(f(q, 'v_x'), f(q, 'v_y')); v1 = hd(f(q, 'pv_x'), f(q, 'pv_y'))
            s0 = math.hypot(f(q, 'v_x'), f(q, 'v_y')); s1 = math.hypot(f(q, 'pv_x'), f(q, 'pv_y'))
            dv = math.hypot(f(q, 'pv_x') - f(q, 'v_x'), f(q, 'pv_y') - f(q, 'v_y'))
            th = wrap(wish - v0)
            out.append((t_ms, ang, f(q, 'game_time') - jt, th, wrap(v1 - v0), s0, s1, dv))
            break
        j += 1
    else:
        out.append((t_ms, ang, None, None, None, None, None, None))
for o in out:
    if o[2] is None:
        print(f"notch {o[0]} ms view {o[1]:3d}: NO W PRESS FOUND IN THE AIR"); continue
    t_ms, ang, t, th, turn, s0, s1, dv = o
    print(f"notch {t_ms} ms view {ang:3d}: press at +{t:.3f}s  angle to wish {th:+6.1f}  turn {turn:+6.1f}  speed {s0:6.1f} -> {s1:6.1f}  |dv| {dv:6.1f}")
