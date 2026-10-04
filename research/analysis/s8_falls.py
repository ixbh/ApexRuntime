"""Session 8 walk-offs: drop height, landing velocity, and the speed / max
speed (`ms`) recovery after landing. usage: s8_falls.py <csv> <client log> <bot logs...>"""
import csv, json, math, re, sys, bisect
from datetime import datetime
rows = [r for r in csv.DictReader(open(sys.argv[1])) if r['side'] == 'client']
f = lambda r, k: float(r[k]); H = lambda r, pre: math.hypot(f(r, 'v_x' if pre else 'pv_x'), f(r, 'v_y' if pre else 'pv_y'))
for line in open(sys.argv[2], encoding='utf-8', errors='ignore'):
    m = re.match(r'\[(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d+)\] \[([\d.]+)\].*MOVE-TRACE', line)
    if m:
        wall0 = datetime.strptime(m.group(1), '%Y-%m-%d %H:%M:%S.%f').timestamp(); game0 = float(m.group(2)); break
times = [f(r, 'game_time') for r in rows]
for path in sys.argv[3:]:
    for a in json.load(open(path)):
        tg = a['start_unix_ms'] / 1000 - wall0 + game0
        i = bisect.bisect_left(times, tg); end = bisect.bisect_left(times, tg + 6.5)
        off = next((k for k in range(i, end) if rows[k - 1]['ground'] == '1' and rows[k]['ground'] == '0'), None)
        if off is None:
            print(path[-30:], 'no takeoff'); continue
        land = next((k for k in range(off, end) if rows[k]['ground'] == '1' and rows[k - 1]['ground'] == '0'), None)
        if land is None:
            print(path[-30:], 'no landing'); continue
        z0 = f(rows[off - 1], 'po_z'); z1 = f(rows[land], 'o_z'); L = rows[land - 1]
        t0 = f(rows[land], 'game_time')
        prof = []
        for dt in (0, 0.05, 0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0):
            k = bisect.bisect_left(times, t0 + dt)
            if k < len(rows): prof.append(f"{H(rows[k], False):5.0f}/{float(rows[k]['max_speed']):3.0f}")
        print(f"{a['label'][:22]:22s} drop {z0 - z1:6.1f}  land vz {f(L, 'pv_z'):7.1f}  |h| {H(L, False):5.1f} -> {H(rows[land], False):5.1f}  slide {rows[land]['slide']}")
        print('      speed/ms after landing @0,.05,.1,.2,.3,.5,.75,1,1.25,1.5,2s:', ' '.join(prof))
