"""Per-gun weapon rules from behaviour_capture.py runs: reload timings
(ammo-in, ready, animation end) as seconds and as fractions of the gun's
reload time, what each reload interruption does (cancelled? magazine
after), sprint-to-fire and sprint-to-ADS delays, ADS in/out, swap times,
inspect length, air/slide fire. Writes <content>/captures/behaviour.json.

    python research/weapons/behaviour_summary.py
"""

import glob
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from behaviour_report import frames  # noqa: E402
from paths import CONTENT  # noqa: E402

CAP = os.path.join(CONTENT, r"captures")


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


if __name__ == "__main__" and "--rules" not in sys.argv:
    main()


def plan_from(rows, start_index=0):
    """Reload plan from frames: times relative to the reload start at or
    after `start_index`: milestone changes, ammo in, ready."""
    rs = next((i for i in range(start_index, len(rows)) if rows[i]["reload"] == 1), None)
    if rs is None:
        return None, None
    t0 = rows[rs]["t"]
    weapon = rows[rs]["weapon"]
    clip0 = rows[rs - 1]["clip"] if rs > 0 else rows[rs]["clip"]
    milestones, ammo, ready, ms = [], None, None, rows[rs]["ms"]
    for r in rows[rs:]:
        if r["weapon"] != weapon:
            break
        if r["ms"] > ms:
            milestones.append(round(r["t"] - t0, 3))
        ms = r["ms"]
        if ammo is None and r["clip"] > clip0:
            ammo = round(r["t"] - t0, 3)
        if ammo is not None and r["reload"] == 0 and r["rdy"] == 1:
            ready = round(r["t"] - t0, 3)
            break
    if ammo is None or ready is None:
        return None, rs
    return {"ammo_in": ammo, "ready": ready, "milestones": milestones}, rs


def plans():
    """Per-gun gameplay timings for the IW4L host (captures/weapon_rules.json)."""
    out = {}
    for path in sorted(glob.glob(os.path.join(CAP, "*-behaviour-wlog.log"))):
        for (gun, scen), rows in frames(path).items():
            g = out.setdefault(gun, {})
            if scen in ("reload_tactical", "reload_empty"):
                p, _ = plan_from(rows)
                if p:
                    g["tactical" if scen == "reload_tactical" else "empty"] = p
            elif scen in ("reload_resume", "reload_empty_resume"):
                first_plan, rs = plan_from(rows)
                # The cancelled reload's milestone, then the resumed reload
                # after the gun comes back.
                if rs is None:
                    continue
                cancel = next((i for i in range(rs, len(rows)) if rows[i]["reload"] == 0), None)
                if cancel is None:
                    continue
                reached = max(r["ms"] for r in rows[rs:cancel])
                away = next((i for i in range(cancel, len(rows)) if rows[i]["weapon"] != rows[0]["weapon"]), None)
                back = next((i for i in range(away, len(rows)) if rows[i]["weapon"] == rows[0]["weapon"]), None) if away else None
                if back is None or reached == 0:
                    continue
                p, rs2 = plan_from(rows, back)
                if p:
                    key = "resume" if scen == "reload_resume" else "empty_resume"
                    p["after_milestone"] = reached
                    p["automatic"] = rows[rs2]["t"] - rows[back]["t"] < 1.0
                    p["sequence"] = rows[rs2]["seq"]
                    g[key] = p
            elif scen == "swap":
                t0 = rows[0]["t"]
                other = first(rows, t0, lambda r: r["weapon"] != rows[0]["weapon"])
                back = first(rows, t0, lambda r: r["weapon"] == rows[0]["weapon"], after=(other or 0) + 0.2)
                back_ready = first(rows, t0, lambda r: r["weapon"] == rows[0]["weapon"] and r["rdy"] == 1, after=(back or 0))
                if other:
                    g["holster"] = other
                if back and back_ready:
                    g["raise"] = round(back_ready - back, 3)
            elif scen == "sprint_fire":
                t0 = rows[0]["t"]
                shot = first(rows, t0, lambda r: r["clip"] < rows[0]["clip"])
                if shot is not None:
                    g["sprint_out"] = round(shot - 1.2, 3)
            elif scen == "inspect":
                t0 = rows[0]["t"]
                start = first(rows, t0, lambda r: "inspect" in r["seq"])
                end = first(rows, t0, lambda r: "inspect" not in r["seq"], after=(start or 0) + 0.05)
                if start is not None and end is not None:
                    g["inspect"] = round(end - start, 3)
    inspect_from_animations(out)
    json.dump(out, open(os.path.join(CAP, "weapon_rules.json"), "w"), indent=1)
    return out


def inspect_from_animations(out):
    """Inspects longer than the capture window: the animation's own length
    (apex-cast notes_probe example prints it)."""
    import re
    import subprocess

    exe = os.path.join(os.path.dirname(__file__), "..", "..", "target", "debug", "examples", "notes_probe.exe")
    if not os.path.exists(exe):
        return
    for g in json.load(open(os.path.join(CONTENT, r"guns.json"))):
        r = out.setdefault(g["gun"], {})
        if "inspect" in r:
            continue
        files = []
        for d in g["anims"]:
            for name in ("inspect_basic_0.cast", "inspect_0.cast"):
                f = os.path.join(CONTENT, d, name)
                if os.path.exists(f):
                    files.append(f)
        if files:
            m = re.search(r"\(([\d.]+)s @", subprocess.run([exe, files[0]], capture_output=True, text=True).stdout)
            if m:
                r["inspect"] = float(m.group(1))


if __name__ == "__main__" and "--rules" in sys.argv:
    for gun, g in plans().items():
        print(gun, json.dumps(g))
