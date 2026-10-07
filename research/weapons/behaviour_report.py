"""Readable timelines from a behaviour_capture.py run: per gun and scenario,
the frames where something changed (magazine, reloading, reload milestone,
ready-to-fire, weapon activity, viewmodel sequence, sprint, slide, ground,
ADS), with times from the scenario start.

    python research/weapons/behaviour_report.py [run] [--gun r97] [--scenario reload_swap_early]
"""

import sys
import argparse
import glob
import os
import re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from paths import CONTENT  # noqa: E402

CAP = os.path.join(CONTENT, r"captures")
F_RE = re.compile(
    r"\[WLOG\] f (\S+):(\S+) ([\d.]+) (\S+) (-?\d+) (\d) ([\d.]+) \S+ \S+ \S+ \S+ \S+ (\S+)"
    r" (-?\d+) (-?\d+) (-?\d+) (-?\d+) (-?\d+) (-?\d+) (-?\d+) (-?\d+) ([\d.]+)"
)
KEYS = ["weapon", "clip", "reload", "zoom", "seq", "spr", "gnd", "crh", "sld", "mnt", "rdy", "act", "ms", "speed"]


def frames(path):
    out = {}
    for line in open(path, encoding="utf-8", errors="replace"):
        m = F_RE.search(line)
        if not m:
            continue
        g = m.groups()
        row = {
            "t": float(g[2]),
            "weapon": g[3].replace("mp_weapon_", ""),
            "clip": int(g[4]),
            "reload": int(g[5]),
            "zoom": round(float(g[6]), 1),
            "seq": g[7].rsplit("/", 1)[-1].replace(".rseq", ""),
            "spr": int(g[8]),
            "gnd": int(g[9]),
            "crh": int(g[10]),
            "sld": int(g[11]),
            "mnt": int(g[12]),
            "rdy": int(g[13]),
            "act": int(g[14]),
            "ms": int(g[15]),
            "speed": float(g[16]),
        }
        out.setdefault((g[0], g[1]), []).append(row)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run", nargs="?")
    ap.add_argument("--gun", default="")
    ap.add_argument("--scenario", default="")
    a = ap.parse_args()
    run = a.run or os.path.basename(sorted(glob.glob(os.path.join(CAP, "*-behaviour-wlog.log")))[-1])[: -len("-behaviour-wlog.log")]
    for (gun, scen), rows in frames(os.path.join(CAP, f"{run}-behaviour-wlog.log")).items():
        if a.gun and a.gun not in gun or a.scenario and a.scenario != scen:
            continue
        print(f"== {gun} {scen}")
        t0 = rows[0]["t"]
        prev = None
        for r in rows:
            cur = {k: r[k] for k in KEYS if k != "speed"}
            if cur != prev:
                diff = " ".join(f"{k}={r[k]}" for k in KEYS if prev is None or r[k] != prev.get(k) or k == "speed")
                print(f"  {r['t'] - t0:6.3f} {diff}")
                prev = cur


if __name__ == "__main__":
    main()
