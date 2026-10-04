"""Replay every recorded wall climb against a wall at the same distance and
compare vertical speed tick by tick.

usage: climb_check.py <flowstate.csv> [t0 t1] [replay_exe]

For each climb: rows from 4 ticks before takeoff to 10 ticks after the climb
ends; inputs from the button bits; per-tick dt from the recording; the wall is
placed where the hull stopped (contact tick position + hull half-width +
DIST_EPSILON along the takeoff direction).
"""
import csv, json, math, os, subprocess, sys, tempfile

src = sys.argv[1]
t0 = float(sys.argv[2]) if len(sys.argv) > 3 else -1e9
t1 = float(sys.argv[3]) if len(sys.argv) > 3 else 1e9
exe = sys.argv[4] if len(sys.argv) > 4 else r'C:\Dev\ApexRuntime\target\release\apex-replay.exe'
rows = list(csv.DictReader(open(src)))
f = lambda r, k: float(r[k])
B = lambda r: int(r['buttons'], 16)
HULL = 16.0 + 0.03125

def dt_of(prev, r):
    # Free fall gives dt exactly (pv_z = v_z - g*dt), unaffected by wall contact.
    # (Not on the climb-attach tick: it applies only half the gravity.)
    clock = f(r, 'game_time') - f(prev, 'game_time')
    if r['ground'] == '0' and prev['ground'] == '0' and f(r, 'v_z') - f(r, 'pv_z') > 1:
        d = (f(r, 'v_z') - f(r, 'pv_z')) / 750.0
        if 0.002 < d < 0.05 and 0.75 * clock < d < 1.35 * clock:
            return d
    sp = math.hypot(f(r, 'pv_x'), f(r, 'pv_y'))
    if sp > 40:
        d = math.hypot(f(r, 'po_x') - f(r, 'o_x'), f(r, 'po_y') - f(r, 'o_y')) / sp
        if 0.002 < d < 0.05:
            return d
    return max(min(f(r, 'game_time') - f(prev, 'game_time'), 0.05), 0.002)

i = 1
summary = []
while i < len(rows):
    r = rows[i]
    climbing = r['ground'] == '1' and 30 < f(r, 'pv_z') < 230 and not (f(r, 'v_z') < 1 and f(r, 'pv_z') > 200)
    if climbing and t0 <= f(r, 'game_time') <= t1:
        j = i
        while j < len(rows) and rows[j]['ground'] == '1' and 1 < f(rows[j], 'pv_z') < 230:
            j += 1
        k = i - 1
        while k > 0 and rows[k]['ground'] == '0':
            k -= 1
        take = k                          # jump tick (ground=1, launches)
        seg = rows[take - 4:j + 10]
        if j - i < 20 or len(seg) < 30:
            i = j + 1
            continue
        start = seg[0]
        d0 = (f(rows[take], 'pv_x'), f(rows[take], 'pv_y'))
        n0 = math.hypot(*d0) or 1.0
        d0 = (d0[0] / n0, d0[1] / n0)
        # contact: first air tick whose horizontal speed collapses
        contact = next((r2 for r2 in rows[take + 1:i] if math.hypot(f(r2, 'pv_x'), f(r2, 'pv_y')) < 0.5 * n0), None)
        if contact is None:
            i = j + 1
            continue
        cx = (f(contact, 'po_x') - f(start, 'o_x')) * d0[0] + (f(contact, 'po_y') - f(start, 'o_y')) * d0[1]
        wall_x = cx + HULL
        events = []
        prev = rows[take - 5]
        for t, q in enumerate(seg):
            b = B(q)
            ev = {"tick": t, "dt": round(dt_of(prev, q), 6),
                  "forward": (1 if b & 8 else 0) - (1 if b & 0x10 else 0),
                  "right": (1 if b & 0x200 else 0) - (1 if b & 0x400 else 0),
                  "press": (["jump"] if b & 2 else []) + (["crouch"] if b & 4 else []) + (["sprint"] if q['sprint'] == '1' else []),
                  "release": [x for x, bit in (("jump", 2), ("crouch", 4)) if not b & bit] + ([] if q['sprint'] == '1' else ["sprint"])}
            events.append(ev)
            prev = q
        v0 = math.hypot(f(start, 'v_x'), f(start, 'v_y'))
        script = {"name": "climb", "tick_rate": 60, "ticks": len(seg),
                  "start": {"position": [0, 0, 0], "velocity": [v0, 0, 0]}, "events": events}
        fd, path = tempfile.mkstemp(suffix='.json'); os.close(fd)
        json.dump(script, open(path, 'w'))
        out = path + '.csv'
        subprocess.run([exe, path, '--out', out, '--block', f'{wall_x:.3f}', '2000'], check=True, capture_output=True)
        port = list(csv.DictReader(open(out)))
        os.remove(path); os.remove(out)
        diffs = [abs(f(p, 'vel_z') - f(q, 'pv_z')) for p, q in zip(port, seg)]
        modes = ''.join('C' if p['mode'] == 'climb' else ('M' if p['mode'] == 'mantle' else '.') for p in port)
        game_modes = ''.join('C' if (q['ground'] == '1' and 1 < f(q, 'pv_z') < 230) else '.' for q in seg)
        within = sum(d < 5 for d in diffs)
        summary.append(within / len(diffs))
        print(f"t={f(r,'game_time'):8.2f} wall at {wall_x:5.1f}: vz within 5 u/s on {within}/{len(diffs)} ticks, worst {max(diffs):6.1f}")
        if os.environ.get('VERBOSE'):
            for p, q in zip(port, seg):
                print(f"     tick {p['tick']:>3} game vz {f(q,'pv_z'):7.1f} h {math.hypot(f(q,'pv_x'), f(q,'pv_y')):6.1f} btn {q['buttons'][-4:]} | ours vz {f(p,'vel_z'):7.1f} h {f(p,'speed_h'):6.1f} {p['mode']}")
        print(f"   game climb ticks: {game_modes}")
        print(f"   ours climb ticks: {modes}")
        i = j
    i += 1
if summary:
    print(f"mean match {100 * sum(summary) / len(summary):.1f}% over {len(summary)} climbs")
