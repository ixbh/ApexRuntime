"""Replay recorded mantles against a ledge of the same height and distance.

usage: mantle_check.py <flowstate.csv> [replay_exe]

MOVE-TRACE does not log inside a mantle, so each mantle is a gap. For every
mantle that finishes normally (player standing on the ledge right after):
  * replay from ~0.4 s before the gap with the recorded inputs and dt,
  * fill the gap with the same number of ticks (its average dt) holding the
    last recorded inputs,
  * world = flat floor + a ledge whose top is the recorded end height and
    whose face is where the hull touched it,
and compare: did our mantle start, its length in ticks vs the gap, and the
exit position (height) and speed.
"""
import csv, json, math, os, subprocess, sys, tempfile

src = sys.argv[1]
exe = sys.argv[2] if len(sys.argv) > 2 else r'C:\Dev\ApexRuntime\target\release\apex-replay.exe'
rows = list(csv.DictReader(open(src)))
f = lambda r, k: float(r[k])
B = lambda r: int(r['buttons'], 16)
HULL = 16.0 + 0.03125
PRE = 25

def inputs(b, sprint):
    return {"forward": (1 if b & 8 else 0) - (1 if b & 0x10 else 0),
            "right": (1 if b & 0x200 else 0) - (1 if b & 0x400 else 0),
            "press": (["jump"] if b & 2 else []) + (["crouch"] if b & 4 else []) + (["sprint"] if sprint else []),
            "release": [x for x, bit in (("jump", 2), ("crouch", 4)) if not b & bit] + ([] if sprint else ["sprint"])}

results = []
STATS = []
for i in range(PRE + 1, len(rows) - 5):
    a, b = rows[i], rows[i + 1]
    gap = int(b['cmd']) - int(a['cmd'])
    if gap <= 3 or not (b['ground'] == '1' and abs(f(b, 'v_z')) < 1):
        continue
    seg = rows[i - PRE:i + 1]
    # Floor = where the player stood at the replay start (or the lowest ground
    # tick); the recorded ground can slope slightly, the replay floor cannot.
    grounds = [f(r, 'po_z') for r in seg if r['ground'] == '1']
    floor_z = f(seg[0], 'o_z') if seg[0]['ground'] == '1' else (min(grounds) if grounds else f(seg[0], 'o_z'))
    top = f(b, 'o_z') - floor_z
    # direction: from the gap start toward the gap end, horizontal
    dx, dy = f(b, 'o_x') - f(a, 'po_x'), f(b, 'o_y') - f(a, 'po_y')
    dl = math.hypot(dx, dy)
    if dl < 1:
        continue
    d = (dx / dl, dy / dl)
    start = seg[0]
    face = (f(a, 'po_x') - f(start, 'o_x')) * d[0] + (f(a, 'po_y') - f(start, 'o_y')) * d[1] + HULL
    # start velocity projected on the mantle direction (replay runs along +X)
    v0 = f(start, 'v_x') * d[0] + f(start, 'v_y') * d[1]
    events = []
    for t, q in enumerate(seg):
        dt = max(min(f(q, 'game_time') - f(rows[i - PRE - 1 + t], 'game_time'), 0.05), 0.002)
        events.append({"tick": t, "dt": round(dt, 6), **inputs(B(q), q['sprint'] == '1')})
    gdt = (f(b, 'game_time') - f(a, 'game_time')) / gap
    last = B(a)
    for k in range(gap):
        ev = {"tick": len(seg) + k, "dt": round(gdt, 6), **inputs(last & ~2, a['sprint'] == '1')}
        events.append(ev)
    script = {"name": "mantle", "tick_rate": 60, "ticks": len(seg) + gap + 3,
              "start": {"position": [0, 0, max(f(start, 'o_z') - floor_z, 0.0)], "velocity": [max(v0, 0.0), 0, f(start, 'v_z')]},
              "events": events}
    fd, path = tempfile.mkstemp(suffix='.json'); os.close(fd)
    json.dump(script, open(path, 'w'))
    out = path + '.csv'
    subprocess.run([exe, path, '--out', out, '--block', f'{face:.3f}', f'{top:.3f}'], check=True, capture_output=True)
    port = list(csv.DictReader(open(out)))
    os.remove(path); os.remove(out)
    m_ticks = [n for n, p in enumerate(port) if p['mode'] == 'mantle']
    if not m_ticks:
        results.append(None)
        print(f"t={f(a,'game_time'):8.2f} ledge {top:5.1f}: NO MANTLE in replay")
        continue
    ours_len = len(m_ticks)
    end = port[m_ticks[-1] + 1] if m_ticks[-1] + 1 < len(port) else port[-1]
    game_exit_h = math.hypot(f(b, 'v_x'), f(b, 'v_y'))
    our_exit_h = math.hypot(f(end, 'vel_x'), f(end, 'vel_y'))
    start_shift = m_ticks[0] - len(seg)
    results.append((ours_len - gap, abs(f(end, 'pos_z') - top), abs(our_exit_h - game_exit_h), start_shift))
    STATS.append((top, (ours_len - gap) * gdt, start_shift * gdt, gdt))
    print(f"t={f(a,'game_time'):8.2f} ledge {top:5.1f}: mantle ticks ours {ours_len:2d} vs gap {gap:2d} (start {start_shift:+d}), "
          f"end z {float(end['pos_z']):6.1f} vs {top:6.1f}, exit |h| {our_exit_h:6.1f} vs {game_exit_h:6.1f}")
ok = [r for r in results if r]
print(f"\n{len(ok)}/{len(results)} mantles reproduced; |ticks diff| <= 2: {sum(abs(r[0]) <= 2 for r in ok)}, "
      f"end height within 1 u: {sum(r[1] < 1 for r in ok)}, exit speed within 2: {sum(r[2] < 2 for r in ok)}")

import statistics as st
if STATS:
    print("duration error (ours - game, s): mean %+.4f  median %+.4f" % (st.mean(x[1] for x in STATS), st.median(x[1] for x in STATS)))
    print("start shift (ours - game, s):    mean %+.4f  median %+.4f" % (st.mean(x[2] for x in STATS), st.median(x[2] for x in STATS)))
    for lo, hi in ((0, 50), (50, 60), (60, 75), (75, 200)):
        sel = [x for x in STATS if lo <= x[0] < hi]
        if sel: print(f"   ledge {lo}-{hi}: n={len(sel):2d} duration err mean {st.mean(x[1] for x in sel):+.4f}s")
