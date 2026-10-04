"""Session 7 deadslide sweep: per attempt, slide start (boost), jump time
after the slide started, speed before/after the jump.
usage: s7_deadslide.py <csv> <client message.log> <bot attempt log>"""
import csv, json, math, re, sys, bisect
from datetime import datetime
rows = [r for r in csv.DictReader(open(sys.argv[1])) if r['side'] == 'client']
f = lambda r, k: float(r[k]); B = lambda r: int(r['buttons'], 16)
H = lambda r, pre: math.hypot(f(r, 'v_x' if pre else 'pv_x'), f(r, 'v_y' if pre else 'pv_y'))
for line in open(sys.argv[2], encoding='utf-8', errors='ignore'):
    m = re.match(r'\[(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d+)\] \[([\d.]+)\].*MOVE-TRACE', line)
    if m:
        wall0 = datetime.strptime(m.group(1), '%Y-%m-%d %H:%M:%S.%f').timestamp(); game0 = float(m.group(2)); break
times = [f(r, 'game_time') for r in rows]
for a in json.load(open(sys.argv[3])):
    tg = a['start_unix_ms'] / 1000 - wall0 + game0
    i = bisect.bisect_left(times, tg)
    end = bisect.bisect_left(times, tg + 3.5)
    s = next((k for k in range(i, end) if rows[k]['slide'] == '1' and rows[k - 1]['slide'] == '0'), None)
    j = next((k for k in range(i, end) if rows[k]['ground'] == '1' and f(rows[k], 'pv_z') > 100), None)
    n = int(re.search(r'jump (\d+) frames', a['label']).group(1))
    if s is None or j is None:
        print(f"N={n:2d}: slide {s} jump {j} (missing)"); continue
    ts = f(rows[s], 'game_time'); tj = f(rows[j], 'game_time')
    print(f"N={n:2d}: slide {H(rows[s], True):5.1f}->{H(rows[s], False):5.1f}  jump +{tj - ts:5.3f}s ({j - s:2d} ticks) "
          f"speed {H(rows[j], True):5.1f} -> {H(rows[j], False):5.1f} ({H(rows[j], False) - H(rows[j], True):+6.1f})  "
          f"vz {f(rows[j], 'pv_z'):5.1f}  duck {rows[j]['duck']} slide {rows[j]['slide']}")
