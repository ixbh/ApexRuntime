"""Wall climbs in MOVE-TRACE show as ground=1 ticks with upward velocity.
Per climb: takeoff->attach time, attach vz, peak vz and when, decay per tick
(as a fraction of vz), climb duration, end impulse (vz, |h|), height gained.
usage: climb.py <csv> [t0 t1]"""
import csv, math, sys, statistics as st
rows = list(csv.DictReader(open(sys.argv[1])))
t0 = float(sys.argv[2]) if len(sys.argv) > 2 else -1e9
t1 = float(sys.argv[3]) if len(sys.argv) > 3 else 1e9
f = lambda r, k: float(r[k])
i = 1
while i < len(rows):
    r = rows[i]
    if r['ground'] == '1' and f(r, 'pv_z') > 30 and not (f(r, 'v_z') < 1 and f(r, 'pv_z') > 200) \
            and t0 <= f(r, 'game_time') <= t1:
        j = i
        while j < len(rows) and rows[j]['ground'] == '1' and f(rows[j], 'pv_z') > 1 and f(rows[j], 'pv_z') < 230:
            j += 1
        climb = rows[i:j]
        if len(climb) < 5:
            i = j + 1; continue
        # takeoff: last ground->air before the climb
        k = i - 1
        while k > 0 and rows[k]['ground'] == '0':
            k -= 1
        take = rows[k]
        vz = [f(q, 'pv_z') for q in climb]
        pk = max(range(len(vz)), key=lambda n: vz[n])
        decay = [(vz[n] - vz[n + 1]) / vz[n] for n in range(pk + 2, len(vz) - 1) if vz[n] > 20]
        end = rows[j] if j < len(rows) else climb[-1]
        dur = f(climb[-1], 'game_time') - f(climb[0], 'game_time')
        print(f"t={f(climb[0],'game_time'):7.2f} attach {f(climb[0],'game_time') - f(take,'game_time'):.3f}s after takeoff, "
              f"vz_in {f(climb[0],'v_z'):6.1f} peak {vz[pk]:6.1f} at +{f(climb[pk],'game_time') - f(climb[0],'game_time'):.3f}s, "
              f"decay/tick {st.median(decay) if decay else float('nan'):.4f}, climb {dur:.3f}s, "
              f"rise {f(climb[-1],'po_z') - f(take,'po_z'):6.1f}, end: vz {f(end,'pv_z'):6.1f} |h| {math.hypot(f(end,'pv_x'), f(end,'pv_y')):5.1f} ge {end['ground']} b {end['buttons'][-4:]}")
        i = j
    i += 1
