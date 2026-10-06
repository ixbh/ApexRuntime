"""Recoil tracks from the Flowstate weapon captures (capture.py).

In the capture the eye angles never move; the aim the bullets follow is the
camera, which every shot pushes by a fixed step and which settles back to
the eye after the trigger is released. Per gun and stance (hip / ads) this
writes the per-shot kick steps (degrees, IW/Source convention: pitch
negative = up, yaw positive = left) and the measured recovery, to
F:/ApexContent/captures/recoil.json (personal data, not committed).

    python research/weapons/recoil.py
"""

import glob
import json
import math
import os
import re
import statistics
from collections import defaultdict

CAP = r"F:\ApexContent\captures"
F_RE = re.compile(
    r"\[WLOG\] f (\S+):(hip|ads) ([\d.]+) \S+ (-?\d+) (\d) ([\d.]+) (\S+) (\S+) (\S+) (\S+)"
)
SETTLE = 0.06  # seconds after a shot when its kick is read


def runs():
    """(gun, mode) -> list of frame lists, one per capture file."""
    out = defaultdict(list)
    files = glob.glob(os.path.join(CAP, "*-client-script*.log")) + glob.glob(os.path.join(CAP, "*-client-wlog.log"))
    for path in files:
        frames = defaultdict(list)
        text = open(path, encoding="utf-8", errors="replace").read()
        for m in F_RE.finditer(text):
            gun, mode, t, clip, _rl, zoom, ep, ey, cp, cy = m.groups()
            frames[(gun, mode)].append((float(t), int(clip), float(cp) - float(ep), float(cy) - float(ey), float(zoom)))
        for key, fr in frames.items():
            fr.sort()
            out[key].append(fr)
    return out


def sprays(fr):
    """Shot frame indices, split into sprays. A shot is any small change of
    the magazine count (the client's prediction corrections land on the
    shot cadence too); a refill (+3 or more) is not."""
    shots = [i for i in range(1, len(fr)) if 0 < abs(fr[i][1] - fr[i - 1][1]) <= 2]
    out, cur = [], []
    for i in shots:
        if cur and fr[i][0] - fr[cur[-1]][0] > 0.7:
            out.append(cur)
            cur = []
        cur.append(i)
    if cur:
        out.append(cur)
    return out


def analyse(fr, shots):
    kicks = []
    for n, i in enumerate(shots):
        pre = fr[i - 1]
        end = shots[n + 1] - 1 if n + 1 < len(shots) else len(fr) - 1
        j = i
        while j < end and fr[j][0] < fr[i][0] + SETTLE:
            j += 1
        kicks.append([round(fr[j][2] - pre[2], 4), round(fr[j][3] - pre[3], 4)])
    # Recovery after the last shot: hold, then exponential rate.
    last = shots[-1]
    t0 = fr[last][0]
    tail = [(f[0] - t0, math.hypot(f[2], f[3])) for f in fr[last:] if f[0] - t0 < 3.0]
    peak = max(m for _, m in tail[: max(1, len(tail) // 4)]) if tail else 0.0
    rec = None
    if peak > 0.3:
        top = next(t for t, m in tail if m >= peak)
        hold = next((t for t, m in tail if t > top and m < peak * 0.95), None)
        t70 = next((t for t, m in tail if t > top and m < peak * 0.7), None)
        t30 = next((t for t, m in tail if t > top and m < peak * 0.3), None)
        if hold is not None and t70 is not None and t30 is not None and t30 > t70:
            rec = {"hold": round(hold, 3), "rate": round(math.log(0.7 / 0.3) / (t30 - t70), 3), "peak": round(peak, 3)}
    return {"kicks": kicks, "shots": len(shots), "recovery": rec}


def main():
    out = {}
    recs = []
    for (gun, mode), candidates in sorted(runs().items()):
        best = None
        for fr in candidates:
            for shots in sprays(fr):
                a = analyse(fr, shots)
                if best is None or a["shots"] > best["shots"]:
                    best = a
        if best:
            out.setdefault(gun, {})[mode] = best
            if best["recovery"]:
                recs.append(best["recovery"])
    hold = statistics.median(r["hold"] for r in recs) if recs else 0.1
    rate = statistics.median(r["rate"] for r in recs) if recs else 5.0
    doc = {"recovery": {"hold": round(hold, 3), "rate": round(rate, 3)}, "guns": out}
    path = os.path.join(CAP, "recoil.json")
    json.dump(doc, open(path, "w"), indent=1)
    for gun, modes in out.items():
        for mode, a in modes.items():
            total = [round(sum(k[0] for k in a["kicks"]), 2), round(sum(k[1] for k in a["kicks"]), 2)]
            print(f"{gun:28s} {mode} shots {a['shots']:3d} total {total} rec {a['recovery']}")
    print("recovery median:", doc["recovery"], "from", len(recs))
    print("wrote", path)


if __name__ == "__main__":
    main()
