"""Session 6 timeline: mantle exits (superglide / boost / jump-off / normal),
wall jumps, and the player's crouch markers (crouch taps standing still:
2 = hit, 1 = miss). usage: s6_timeline.py <csv>"""
import csv, math, sys
rows = [r for r in csv.DictReader(open(sys.argv[1])) if r['side'] == 'client']
f = lambda r, k: float(r[k]); B = lambda r: int(r['buttons'], 16)
H = lambda r, pre: math.hypot(f(r, 'v_x' if pre else 'pv_x'), f(r, 'v_y' if pre else 'pv_y'))
ev = []
taps = []
for i in range(1, len(rows)):
    r, p = rows[i], rows[i - 1]
    t = f(r, 'game_time')
    if int(r['cmd']) - int(p['cmd']) > 3:
        h, vz = H(r, True), f(r, 'v_z')
        kind = ('SUPERGLIDE' if h > 440 and vz > 250 else 'BOOST' if h > 380 and vz > 330
                else 'jump-off' if vz > 200 else 'mantle')
        ev.append((t, f"{kind:10s} exit |h| {h:6.1f} vz {vz:6.1f} btn {r['buttons'][-4:]}"))
    if B(r) & 4 and not B(p) & 4 and r['ground'] == '1' and H(r, True) < 5:
        taps.append(t)
    # wall jump: leaving a climb/contact with a big push
    if r['ground'] == '1' and f(r, 'v_z') != 0 and B(r) & 2 and not B(p) & 2 and H(r, False) > 150:
        ev.append((t, f"wall jump  vz {f(r,'pv_z'):6.1f} |h| {H(r, False):6.1f} btn {r['buttons'][-4:]}"))
# cluster crouch taps into markers
k = 0
while k < len(taps):
    j = k
    while j + 1 < len(taps) and taps[j + 1] - taps[j] < 1.0:
        j += 1
    n = j - k + 1
    ev.append((taps[k], f"---- MARKER {n} crouch tap(s) {'HIT' if n == 2 else 'MISS' if n == 1 else ''}"))
    k = j + 1
for t, s in sorted(ev):
    print(f"{t:8.2f}  {s}")
