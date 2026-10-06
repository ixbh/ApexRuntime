"""Per-gun weapon rules from behaviour_capture.py runs: reload timings
(ammo-in, ready, animation end) as seconds and as fractions of the gun's
reload time, what each reload interruption does (cancelled? magazine
after), sprint-to-fire and sprint-to-ADS delays, ADS in/out, swap times,
inspect length, air/slide fire. Writes F:/ApexContent/captures/behaviour.json.

    python research/weapons/behaviour_summary.py
"""

import glob
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from behaviour_report import frames  # noqa: E402

CAP = r"F:\ApexContent\captures"


def first(rows, t0, cond, after=0.0):
    for r in rows:
        if r["t"] - t0 >= after and cond(r):
            return round(r["t"] - t0, 3)
    return None


def reload_times(rows):
    t0 = rows[0]["t"]
    start = first(rows, t0, lambda r: r["reload"] == 1)
    clip0 = rows[0]["clip"]
    ammo_in = first(rows, t0, lambda r: r["weapon"] == rows[0]["weapon"] and r["clip"] > clip0)
    ready = first(rows, t0, lambda r: r["reload"] == 0 and r["rdy"] == 1, after=(start or 0) + 0.05)
    end = first(rows, t0, lambda r: "reload" not in r["seq"], after=(start or 0) + 0.05)
    return start, ammo_in, ready, end


def main():
    runs = sorted(glob.glob(os.path.join(CAP, "*-behaviour-wlog.log")))
    wpath = os.path.join(CAP, "weapons.json")
    settings = {g: r.get("settings", {}) for g, r in json.load(open(wpath)).items()} if os.path.exists(wpath) else {}
    data = {}
    for path in runs:
        run = os.path.basename(path)[: -len("-behaviour-wlog.log")]
        meta_path = os.path.join(CAP, f"{run}-behaviour.json")
        meta = json.load(open(meta_path)) if os.path.exists(meta_path) else {"guns": {}}
        for (gun, scen), rows in frames(path).items():
            info = meta["guns"].get(gun, {})
            data.setdefault(gun, {"reload_time": info.get("reload_time"), "clip": info.get("clip"), "scen": {}})
            steps = info.get("scenarios", {}).get(scen, {}).get("steps", [])
            data[gun]["scen"][scen] = (rows, steps)

    out = {}
    for gun, g in sorted(data.items()):
        st = settings.get(gun, {})
        rt = float(st.get("reload_time") or g["reload_time"] or 1.0)
        rte = float(st.get("reloadempty_time") or rt)
        res = {}
        sc = g["scen"]

        def frac(x, total):
            return round(x / total, 3) if x is not None and total else None

        for name, total in (("reload_tactical", rt), ("reload_empty", rte)):
            if name in sc:
                s, a, r, e = reload_times(sc[name][0])
                res[name] = {"ammo_in": a, "ready": r, "anim_end": e, "ammo_in_frac": frac(a, total), "ready_frac": frac(r, total), "anim_end_frac": frac(e, total)}
        # Interruptions: did the reload stop before the ammo went in, and
        # what is in the magazine at the end?
        for name, (rows, _) in sc.items():
            if not name.startswith("reload_") or name in ("reload_tactical", "reload_empty"):
                continue
            mine = [r for r in rows if r["weapon"] == rows[0]["weapon"]]
            t0 = rows[0]["t"]
            began = first(rows, t0, lambda r: r["reload"] == 1)
            stop = first(rows, t0, lambda r: r["reload"] == 0, after=(began or 0) + 0.02) if began is not None else None
            ammo = first(mine, t0, lambda r: r["clip"] > rows[0]["clip"])
            res[name] = {
                "reload_began": began,
                "cancelled": stop is not None and (ammo is None or stop < ammo),
                "stopped_at": stop,
                "ammo_in": ammo,
                "clip_start": rows[0]["clip"],
                "clip_end": mine[-1]["clip"] if mine else None,
                "zoom_max": max(r["zoom"] for r in rows),
            }

        def step_time(steps, arg):
            return next((t for t, k, a in steps if a == arg), None)

        if "sprint_fire" in sc:
            rows, steps = sc["sprint_fire"]
            t0 = rows[0]["t"]
            press = step_time(steps, "+attack") or 1.2
            shot = first(rows, t0, lambda r: r["clip"] < rows[0]["clip"])
            res["sprint_to_fire"] = round(shot - press, 3) if shot is not None else None
        if "sprint_ads" in sc:
            rows, steps = sc["sprint_ads"]
            t0 = rows[0]["t"]
            press = step_time(steps, "+zoom") or 1.2
            full = first(rows, t0, lambda r: r["zoom"] >= 1.0)
            res["sprint_to_ads"] = round(full - press, 3) if full is not None else None
        if "ads_in_out" in sc:
            rows, steps = sc["ads_in_out"]
            t0 = rows[0]["t"]
            zin = first(rows, t0, lambda r: r["zoom"] >= 1.0)
            out_press = step_time(steps, "-zoom") or 1.0
            zout = first(rows, t0, lambda r: r["zoom"] <= 0.0, after=out_press)
            res["ads_in"] = zin
            res["ads_out"] = round(zout - out_press, 3) if zout is not None else None
        if "swap" in sc:
            rows, _ = sc["swap"]
            t0 = rows[0]["t"]
            other = first(rows, t0, lambda r: r["weapon"] != rows[0]["weapon"])
            other_ready = first(rows, t0, lambda r: r["weapon"] != rows[0]["weapon"] and r["rdy"] == 1)
            back = first(rows, t0, lambda r: r["weapon"] == rows[0]["weapon"], after=(other or 0) + 0.2)
            back_ready = first(rows, t0, lambda r: r["weapon"] == rows[0]["weapon"] and r["rdy"] == 1, after=(back or 0))
            res["swap"] = {
                "holster": other,
                "second_ready_after": round(other_ready - other, 3) if other and other_ready else None,
                "draw_ready_after": round(back_ready - back, 3) if back and back_ready else None,
            }
        if "inspect" in sc:
            rows, _ = sc["inspect"]
            t0 = rows[0]["t"]
            start = first(rows, t0, lambda r: "inspect" in r["seq"])
            end = first(rows, t0, lambda r: "inspect" not in r["seq"], after=(start or 0) + 0.05)
            res["inspect"] = {"start": start, "end": end, "sequence": next((r["seq"] for r in rows if "inspect" in r["seq"]), None)}
        for name, arg in (("inspect_fire", "+attack"), ("air_fire", "+attack"), ("slide_fire", "+attack")):
            if name in sc:
                rows, steps = sc[name]
                t0 = rows[0]["t"]
                press = step_time(steps, arg) or 0.0
                shot = first(rows, t0, lambda r: r["clip"] < rows[0]["clip"])
                res[name + "_delay"] = round(shot - press, 3) if shot is not None else None
        out[gun] = res
    json.dump(out, open(os.path.join(CAP, "behaviour.json"), "w"), indent=1)
    for gun, res in out.items():
        print("==", gun)
        for k, v in res.items():
            print("  ", k, v)


if __name__ == "__main__":
    main()
