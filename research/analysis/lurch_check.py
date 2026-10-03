"""Replay every single-key A/D lurch from a Flowstate CSV through apex-replay and
compare the heading change on the press tick.

usage: lurch_check.py <flowstate.csv> [replay_exe]

Assumes the camera points along the velocity at takeoff (mouse held still):
the script starts at yaw 0 with the velocity on +X. Only arcs with exactly one
A/D press edge and no *new* forward/back press in the air are used (W held
from the sprint is fine: held keys do not lurch).
"""
import csv, json, math, subprocess, sys, tempfile, os

src = sys.argv[1]
exe = sys.argv[2] if len(sys.argv) > 2 else r'C:\Dev\ApexRuntime\target\release\apex-replay.exe'
rows = list(csv.DictReader(open(src)))
hd = lambda x, y: math.degrees(math.atan2(y, x))
wrap = lambda a: (a + 180) % 360 - 180
B = lambda r: int(r['buttons'], 16)

results = []
i = 1
while i < len(rows):
    if rows[i]['ground'] == '0' and rows[i - 1]['ground'] == '1':
        j = i
        while j < len(rows) and rows[j]['ground'] == '0':
            j += 1
        PRE = 4                                  # ground ticks before takeoff, so the replay starts grounded
        arc = rows[i - PRE:j]                    # ..., takeoff tick (PRE-1), air ticks (PRE..)
        edges = [k for k in range(PRE, len(arc)) if (B(arc[k]) & 0x600) and not (B(arc[k - 1]) & 0x600)]
        fb = any((B(arc[k]) & 0x18) and not (B(arc[k - 1]) & 0x18) for k in range(PRE, len(arc)))
        if len(edges) == 1 and not fb and float(arc[PRE - 1]['speed_h'] or 0) > 200:
            k = edges[0]
            r = arc[k]
            ref_dh = wrap(hd(float(r['pv_x']), float(r['pv_y'])) - hd(float(r['v_x']), float(r['v_y'])))
            t_air = float(r['game_time']) - float(arc[PRE]['game_time'])
            # dt per tick from horizontal travel
            dts = []
            for q in arc:
                sp = math.hypot(float(q['pv_x']), float(q['pv_y']))
                d = math.hypot(float(q['po_x']) - float(q['o_x']), float(q['po_y']) - float(q['o_y'])) / sp if sp > 40 else 1 / 60
                dts.append(min(max(d, 0.002), 0.05))
            events = []
            for t, q in enumerate(arc):
                b = B(q)
                right = (1 if b & 0x200 else 0) - (1 if b & 0x400 else 0)
                fwd = (1 if b & 0x8 else 0) - (1 if b & 0x10 else 0)
                ev = {"tick": t, "dt": round(dts[t], 6), "forward": fwd, "right": right,
                      "press": (["jump"] if b & 2 else []) + (["sprint"] if q['sprint'] == '1' else []),
                      "release": [x for x in ("jump", "sprint", "crouch") if not ((x == "jump" and b & 2) or (x == "sprint" and q['sprint'] == '1'))]}
                events.append(ev)
            v0 = math.hypot(float(arc[0]['v_x']), float(arc[0]['v_y']))
            script = {"name": "lurch", "tick_rate": 60, "ticks": len(arc),
                      "start": {"position": [0, 0, 0], "velocity": [v0, 0, 0]}, "events": events}
            fd, path = tempfile.mkstemp(suffix='.json'); os.close(fd)
            json.dump(script, open(path, 'w'))
            outp = path + '.csv'
            subprocess.run([exe, path, '--out', outp], check=True, capture_output=True)
            port = list(csv.DictReader(open(outp)))
            pr, pp = port[k], port[k - 1]
            # sign of A/D relative to heading is convention-dependent: compare magnitudes
            port_dh = wrap(hd(float(pr['vel_x']), float(pr['vel_y'])) - hd(float(pp['vel_x']), float(pp['vel_y'])))
            results.append((t_air, abs(ref_dh), abs(port_dh), float(pr['lurch_k']), float(pp['time_in_air']), arc[k - 1]['buttons'][-4:]))
            os.remove(path); os.remove(outp)
        i = j
    i += 1

print(f"{'t_air':>6} {'game':>7} {'ours':>7}  diff   our_k our_t_prev prev_btn")
for t, a, b, k, tp, pb in sorted(results):
    print(f"{t:6.3f} {a:7.2f} {b:7.2f}  {b - a:+6.2f}  {k:.3f}  {tp:.3f}  {pb}")
if results:
    err = [abs(b - a) for _, a, b, *_ in results]
    print(f"n={len(results)} mean |err| {sum(err) / len(err):.2f} deg, max {max(err):.2f}")
