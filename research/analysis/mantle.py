"""Mantles appear as gaps in MOVE-TRACE (TraversalMove does not log). For each
gap: state before, state after, height gained, duration, exit velocity.
usage: mantle.py <csv> [t0 t1]"""
import csv, math, sys
rows = list(csv.DictReader(open(sys.argv[1])))
t0 = float(sys.argv[2]) if len(sys.argv) > 2 else -1e9
t1 = float(sys.argv[3]) if len(sys.argv) > 3 else 1e9
H = lambda r, pre=False: math.hypot(float(r['v_x' if pre else 'pv_x']), float(r['v_y' if pre else 'pv_y']))
print(f"{'t':>7} {'dur':>5} {'dz':>6} {'dxy':>5} | pre: {'|h|in':>6} ge b    | post: {'|h|':>6} {'vz':>6} ge b")
for a, b in zip(rows, rows[1:]):
    gap = int(b['cmd']) - int(a['cmd'])
    t = float(a['game_time'])
    if gap > 3 and t0 <= t <= t1:
        dz = float(b['o_z']) - float(a['po_z'])
        dxy = math.hypot(float(b['o_x']) - float(a['po_x']), float(b['o_y']) - float(a['po_y']))
        dur = float(b['game_time']) - t
        print(f"{t:7.2f} {dur:5.3f} {dz:6.1f} {dxy:5.1f} | {H(a, True):6.1f} {a['ground']}  {a['buttons'][-4:]} | {H(b, True):6.1f} {float(b['v_z']):6.1f} {b['ground']}  {b['buttons'][-4:]}")
