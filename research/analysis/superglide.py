"""Superglide timing from MOVE-TRACE mantle gaps. A jump during the mantle
ends the traversal early, so the gap length is the press time. Ledge height
comes from where the player next stands; progress = height reached at the
press / ledge height.  usage: superglide.py <csv> [t0 t1]"""
import csv, math, sys
rows = list(csv.DictReader(open(sys.argv[1])))
t0 = float(sys.argv[2]) if len(sys.argv) > 2 else -1e9
t1 = float(sys.argv[3]) if len(sys.argv) > 3 else 1e9
f = lambda r, k: float(r[k])
H = lambda r: math.hypot(f(r, 'v_x'), f(r, 'v_y'))
print(f"{'t':>8} {'gap':>5} {'ledge':>6} {'z_frac':>6} {'t_frac':>6} {'exit|h|':>7} {'exit vz':>7}  result")
for i in range(len(rows) - 1):
    a, b = rows[i], rows[i + 1]
    if int(b['cmd']) - int(a['cmd']) <= 3 or not (t0 <= f(a, 'game_time') <= t1):
        continue
    z0 = f(a, 'po_z'); zg = f(b, 'o_z'); gap = f(b, 'game_time') - f(a, 'game_time')
    # next tick standing still on ground after the gap -> ledge top height
    zl = None
    for r in rows[i + 1:i + 1 + 400]:
        if r['ground'] == '1' and abs(f(r, 'pv_z')) < 1 and f(r, 'po_z') > z0 + 10:
            zl = f(r, 'po_z'); break
        if int(r['cmd']) - int(rows[rows.index(r) - 1]['cmd']) > 3:
            break
    ledge = (zl - z0) if zl else float('nan')
    full = 0.22 + 0.006 * ledge if zl else float('nan')
    ex_h, ex_vz = H(b), f(b, 'v_z')
    res = 'SUPERGLIDE' if ex_h > 440 and ex_vz > 250 else ('jump' if ex_vz > 200 else 'mantle')
    print(f"{f(a,'game_time'):8.2f} {gap:5.3f} {ledge:6.1f} {(zg - z0) / ledge if zl else float('nan'):6.2f} {gap / full if zl else float('nan'):6.2f} {ex_h:7.1f} {ex_vz:7.1f}  {res}")
