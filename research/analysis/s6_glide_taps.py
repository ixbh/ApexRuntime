"""After each superglide: exit direction vs the approach (view, mouse still)
and every new directional press in the following airtime: angle to the view
before/after, turn, speed. usage: s6_glide_taps.py <csv> [t0 t1]"""
import csv, math, sys
rows = [r for r in csv.DictReader(open(sys.argv[1])) if r['side'] == 'client']
t0 = float(sys.argv[2]) if len(sys.argv) > 2 else 0; t1 = float(sys.argv[3]) if len(sys.argv) > 3 else 1e9
f = lambda r, k: float(r[k]); B = lambda r: int(r['buttons'], 16)
hd = lambda x, y: math.degrees(math.atan2(y, x)); wrap = lambda a: (a + 180) % 360 - 180
for i in range(1, len(rows)):
    r, p = rows[i], rows[i - 1]
    if int(r['cmd']) - int(p['cmd']) > 3 and math.hypot(f(r, 'v_x'), f(r, 'v_y')) > 440 and t0 <= f(r, 'game_time') <= t1:
        # approach direction: horizontal velocity a few ticks before the mantle gap while moving
        k = i - 1
        while k > 0 and math.hypot(f(rows[k], 'pv_x'), f(rows[k], 'pv_y')) < 100:
            k -= 1
        view = hd(f(rows[k], 'pv_x'), f(rows[k], 'pv_y'))
        ex = hd(f(r, 'v_x'), f(r, 'v_y'))
        tt = f(r, 'game_time')
        print(f"t={tt:7.2f} superglide exit {wrap(ex - view):+6.1f} deg from approach, btn {r['buttons'][-4:]}")
        j = i + 1
        while j < len(rows) and rows[j]['ground'] == '0' and f(rows[j], 'game_time') - tt < 1.0:
            q, pq = rows[j], rows[j - 1]
            new = (B(q) & 0x618) & ~(B(pq) & 0x618)
            if new:
                a0 = wrap(hd(f(q, 'v_x'), f(q, 'v_y')) - view); a1 = wrap(hd(f(q, 'pv_x'), f(q, 'pv_y')) - view)
                s0 = math.hypot(f(q, 'v_x'), f(q, 'v_y')); s1 = math.hypot(f(q, 'pv_x'), f(q, 'pv_y'))
                keys = ''.join(n for b, n in ((8, 'W'), (0x10, 'S'), (0x400, 'A'), (0x200, 'D')) if B(q) & b)
                print(f"   +{f(q,'game_time')-tt:5.3f}s press {keys:3s} vel {a0:+7.1f} -> {a1:+7.1f} (turn {wrap(a1-a0):+6.1f})  speed {s0:6.1f} -> {s1:6.1f}")
            j += 1
