"""Weapon behaviour rules from offline Flowstate: what cancels a reload and
what a cancelled reload leaves in the magazine, sprint/ADS/fire timing,
swaps, inspect. Each scenario starts from the same spot with fresh guns,
runs timed console actions (no OS input) while the WeaponLog mod logs every
client frame (magazine, reloading, reload milestone, ready-to-fire, weapon
activity, viewmodel sequence, sprint/ground/crouch/slide/mantle, speed).

    python research/weapons/behaviour_capture.py [--guns r97,wingman]

Writes <content>/captures/<run>-behaviour-wlog.log (frames, tagged
gun:scenario) and <run>-behaviour.json (scenarios and step times).
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from capture import Tail, connect, newest_dir  # noqa: E402
from paths import CONTENT  # noqa: E402

OUT = os.path.join(CONTENT, r"captures")
GUNS = "r97,rspn101,wingman,energy_shotgun,sniper,lmg,g2,shotgun"
RELEASE = "-forward;-speed;-duck;-jump;-zoom;-attack;-melee;-reload"


def press(name, hold=0.1):
    return [(0.0, "cmd", "+" + name), (hold, "cmd", "-" + name)]


def scenarios(reload_t, reload_e):
    """name -> (magazine at start, [(time, kind, arg)])."""
    late = max(0.3, reload_t - 0.25)
    mid = 0.45 * reload_t
    mid_e = 0.45 * reload_e
    run = [(0.0, "cmd", "+forward"), (0.0, "cmd", "+speed")]
    s = {
        "reload_tactical": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (reload_t + 1.0, "end", "")]),
        "reload_empty": ("empty", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (reload_t + 1.5, "end", "")]),
        "reload_sprint": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload")] + [(0.3 + t, k, a) for t, k, a in run] + [(reload_t + 1.0, "end", "")]),
        "reload_swap_early": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (0.3, "cmd", "weaponSelectPrimary1"), (1.8, "cmd", "weaponSelectPrimary0"), (3.5, "end", "")]),
        "reload_swap_late": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (late, "cmd", "weaponSelectPrimary1"), (late + 1.5, "cmd", "weaponSelectPrimary0"), (late + 3.2, "end", "")]),
        "reload_melee": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (0.3, "cmd", "+melee"), (0.4, "cmd", "-melee"), (reload_t + 1.5, "end", "")]),
        "reload_ads": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (0.3, "cmd", "+zoom"), (reload_t + 0.5, "cmd", "-zoom"), (reload_t + 1.2, "end", "")]),
        "reload_fire": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (0.3, "cmd", "+attack"), (0.6, "cmd", "-attack"), (reload_t + 1.0, "end", "")]),
        "reload_jump": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (0.3, "cmd", "+jump"), (0.4, "cmd", "-jump"), (reload_t + 1.0, "end", "")]),
        "reload_slide": ("half", run + [(0.6, "cmd", "+reload"), (0.75, "cmd", "-reload"), (0.9, "cmd", "+duck"), (reload_t + 1.6, "end", "")]),
        # Cancel mid-reload (after the first milestone, before the ammo),
        # then reload again: does it resume from the milestone?
        "reload_resume": ("half", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (mid, "cmd", "weaponSelectPrimary1"), (mid + 1.2, "cmd", "weaponSelectPrimary0"), (mid + 2.2, "cmd", "+reload"), (mid + 2.35, "cmd", "-reload"), (mid + 2.2 + reload_t + 1.0, "end", "")]),
        "reload_empty_resume": ("empty", [(0.0, "cmd", "+reload"), (0.15, "cmd", "-reload"), (mid_e, "cmd", "weaponSelectPrimary1"), (mid_e + 1.2, "cmd", "weaponSelectPrimary0"), (mid_e + 2.2, "cmd", "+reload"), (mid_e + 2.35, "cmd", "-reload"), (mid_e + 2.2 + reload_e + 1.0, "end", "")]),
        "sprint_fire": ("full", run + [(1.2, "cmd", "+attack"), (1.6, "cmd", "-attack"), (2.4, "end", "")]),
        "sprint_ads": ("full", run + [(1.2, "cmd", "+zoom"), (2.2, "cmd", "-zoom"), (2.8, "end", "")]),
        "ads_in_out": ("full", [(0.0, "cmd", "+zoom"), (1.0, "cmd", "-zoom"), (1.8, "end", "")]),
        "swap": ("full", [(0.0, "cmd", "weaponSelectPrimary1"), (1.5, "cmd", "weaponSelectPrimary0"), (3.0, "end", "")]),
        "inspect": ("full", [(0.0, "cmd", "weapon_inspect"), (9.0, "end", "")]),
        "inspect_fire": ("full", [(0.0, "cmd", "weapon_inspect"), (1.0, "cmd", "+attack"), (1.2, "cmd", "-attack"), (2.5, "end", "")]),
        "inspect_sprint": ("full", [(0.0, "cmd", "weapon_inspect")] + [(1.0 + t, k, a) for t, k, a in run] + [(2.5, "end", "")]),
        "air_fire": ("full", [(0.0, "cmd", "+jump"), (0.1, "cmd", "-jump"), (0.15, "cmd", "+attack"), (0.5, "cmd", "-attack"), (1.5, "end", "")]),
        "slide_fire": ("full", run + [(0.8, "cmd", "+duck"), (1.0, "cmd", "+attack"), (1.4, "cmd", "-attack"), (2.2, "end", "")]),
    }
    return s


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--guns", default=GUNS)
    ap.add_argument("--second", default="mp_weapon_semipistol")
    ap.add_argument("--wait", type=float, default=1200.0)
    ap.add_argument("--scenarios", default="", help="comma list (default all)")
    args = ap.parse_args()
    only = set(filter(None, args.scenarios.split(",")))
    guns = [g if g.startswith("mp_weapon_") else "mp_weapon_" + g for g in args.guns.split(",")]
    settings = {}
    wpath = os.path.join(OUT, "weapons.json")
    if os.path.exists(wpath):
        settings = {g: r.get("settings", {}) for g, r in json.load(open(wpath)).items()}

    server = connect(37019, "agentlink.token", args.wait)
    client = connect(37020, "agentlink_client.token", args.wait)
    deadline = time.time() + args.wait
    while True:
        ok, reply, _ = server.eval("server", "WLog_Setup( 400.0 )")
        if ok and reply and reply.get("ok"):
            break
        if time.time() > deadline:
            raise SystemExit("no player")
        time.sleep(3)
    server.eval("server", 'Agent_God( "me", true )')

    run = time.strftime("%Y%m%d-%H%M%S")
    tail = Tail(newest_dir("client"), os.path.join(OUT, f"{run}-behaviour-wlog.log"))
    doc = {"guns": {}}
    for gun in guns:
        s = settings.get(gun, {})
        num = lambda k, d: float(s[k]) if k in s and s[k] not in ("?", "<missing>") else d  # noqa: E731
        reload_t = num("reload_time", 2.0)
        reload_e = num("reloadempty_time", reload_t)
        clip = int(num("ammo_clip_size", 10))
        doc["guns"][gun] = {"reload_time": reload_t, "clip": clip, "scenarios": {}}
        for name, (mag, steps) in scenarios(reload_t, reload_e).items():
            if only and name not in only:
                continue
            server.eval("server", f'WLog_Give( "{gun}" )')
            server.eval("server", f'WLog_GiveSecond( "{args.second}" )')
            server.eval("server", "WLog_Aim()")
            time.sleep(1.6)
            want = {"half": max(1, clip // 2), "empty": 0, "full": clip}[mag]
            server.eval("server", f"WLog_SetClip( {want} )")
            time.sleep(0.2)
            client.eval("client", f'WLog_Start( "{gun}:{name}" )')
            t0 = time.time()
            done = []
            for at, kind, arg in sorted(steps, key=lambda x: x[0]):
                wait = t0 + at - time.time()
                if wait > 0:
                    time.sleep(wait)
                if kind == "cmd":
                    client.cmd(arg)
                done.append([round(time.time() - t0, 3), kind, arg])
            client.cmd(RELEASE)
            client.eval("client", "WLog_Stop()")
            tail.pump()
            doc["guns"][gun]["scenarios"][name] = {"magazine": want, "steps": done}
            print(f"{gun} {name}")
        json.dump(doc, open(os.path.join(OUT, f"{run}-behaviour.json"), "w"), indent=1)
    print("done", run)


if __name__ == "__main__":
    main()
