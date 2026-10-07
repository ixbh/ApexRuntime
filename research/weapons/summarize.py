"""Summarize a capture.py run: per gun, the live settings, measured shot
interval, recoil path (camera angles at each shot, relative to the first)
and damage per hit group, next to what our engine (guns.json defs) uses.

    python research/weapons/summarize.py [run prefix, default newest]
"""

import glob
import json
import os
import re
import statistics
import sys
from collections import defaultdict
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import CONTENT  # noqa: E402

CAP = os.path.join(CONTENT, r"captures")

F_RE = re.compile(r"\[WLOG\] f (\S+):(hip|ads) ([\d.]+) (\S+) (-?\d+) (\d) ([\d.]+) (\S+) (\S+) (\S+) (\S+) (\S+) (\S+)")
SET_RE = re.compile(r"\[WLOG\] set (\S+) (\S+) (.*)$", re.M)
DMG_RE = re.compile(r"\[WLOG\] dmg ([\d.]+) (\S+) ([\d.]+) (-?\d+) ([\d.-]+)")


def main():
    if len(sys.argv) > 1:
        prefix = sys.argv[1]
    else:
        plans = sorted(glob.glob(os.path.join(CAP, "*-plan.json")))
        prefix = os.path.basename(plans[-1])[: -len("-plan.json")]
    wlog = os.path.join(CAP, f"{prefix}-client-wlog.log")
    if not os.path.exists(wlog):
        wlog = os.path.join(CAP, f"{prefix}-client-script.log")
    client = open(wlog, encoding="utf-8", errors="replace").read()
    server = open(os.path.join(CAP, f"{prefix}-server-script.log"), encoding="utf-8", errors="replace").read()

    settings = defaultdict(dict)
    for m in SET_RE.finditer(server):
        settings[m.group(1)][m.group(2)] = m.group(3).strip()
    damage = defaultdict(lambda: defaultdict(list))
    for m in DMG_RE.finditer(server):
        damage[m.group(2)][int(m.group(4))].append(float(m.group(3)))

    frames = defaultdict(list)
    for m in F_RE.finditer(client):
        gun, mode = m.group(1), m.group(2)
        frames[(gun, mode)].append(
            {
                "t": float(m.group(3)),
                "clip": int(m.group(5)),
                "reload": int(m.group(6)),
                "zoom": float(m.group(7)),
                "cam": (float(m.group(10)), float(m.group(11))),
                "seq": m.group(13),
            }
        )

    out = {}
    for gun in sorted({g for g, _ in frames} | set(settings)):
        row = {"settings": settings.get(gun, {}), "damage": {str(k): sorted(set(v)) for k, v in damage[gun].items()}}
        for mode in ("hip", "ads"):
            fr = frames.get((gun, mode), [])
            pairs = [(a, b) for a, b in zip(fr, fr[1:]) if b["clip"] < a["clip"]]
            shots = [b for _, b in pairs]
            times = [s["t"] for s in shots]
            gaps = [b - a for a, b in zip(times, times[1:])]
            # Angles just before the first shot (after the aim snap).
            base = pairs[0][0]["cam"] if pairs else (0.0, 0.0)
            path = [[round(s["cam"][0] - base[0], 3), round(s["cam"][1] - base[1], 3)] for s in shots]
            seqs = []
            for f in fr:
                if not seqs or seqs[-1] != f["seq"]:
                    seqs.append(f["seq"])
            row[mode] = {
                "shots": len(shots),
                "interval_median": round(statistics.median(gaps), 4) if gaps else None,
                "rate_measured": round(1.0 / statistics.median(gaps), 2) if gaps else None,
                "recoil_path": path,
                "sequences": seqs,
            }
        out[gun] = row
    path = os.path.join(CAP, f"{prefix}-summary.json")
    json.dump(out, open(path, "w"), indent=1)

    lines = ["| gun | fire_rate | measured hip | clip | dmg (hitgroup: values) | reload / empty |", "|---|---|---|---|---|---|"]
    for gun, row in out.items():
        s = row["settings"]
        dmg = "; ".join(f"{k}: {','.join(str(round(v)) for v in vals)}" for k, vals in sorted(row["damage"].items()))
        lines.append(
            f"| {gun} | {s.get('fire_rate', '?')} | {row['hip']['rate_measured']} ({row['hip']['shots']} shots) | "
            f"{s.get('ammo_clip_size', '?')} | {dmg} | {s.get('reload_time', '?')} / {s.get('reloadempty_time', '?')} |"
        )
    open(os.path.join(CAP, f"{prefix}-summary.md"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    print("wrote", path)


if __name__ == "__main__":
    main()
