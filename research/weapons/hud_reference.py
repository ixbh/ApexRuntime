"""Flowstate HUD / inventory reference screenshots (game frames only, via
the game's own `screenshot` command) for rebuilding the Apex HUD in IW4L.

    python research/weapons/hud_reference.py
"""

import glob
import json
import os
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from capture import ROOT, connect  # noqa: E402
from paths import CONTENT  # noqa: E402

OUT = os.path.join(CONTENT, r"captures\hud")


def shot(client, name, seen):
    client.cmd("screenshot")
    time.sleep(2.0)
    files = [f for f in glob.glob(os.path.join(ROOT, "**", "*.tga"), recursive=True) if f not in seen]
    files += [f for f in glob.glob(os.path.join(ROOT, "**", "*.jpg"), recursive=True) if f not in seen]
    for f in files:
        seen.add(f)
        dst = os.path.join(OUT, f"{name}{os.path.splitext(f)[1]}")
        shutil.copy(f, dst)
        print("saved", dst)
        return dst
    print("no screenshot file for", name)
    return None


def main():
    os.makedirs(OUT, exist_ok=True)
    server = connect(37019, "agentlink.token", 900)
    client = connect(37020, "agentlink_client.token", 900)
    # Wait for the player.
    while True:
        ok, reply, raw = server.eval("server", "WLog_Setup( 400.0 )")
        if ok and reply and reply.get("ok"):
            break
        time.sleep(5)
    seen = set(glob.glob(os.path.join(ROOT, "**", "*.tga"), recursive=True))
    seen |= set(glob.glob(os.path.join(ROOT, "**", "*.jpg"), recursive=True))

    ok, cat, raw = server.eval("server", 'Agent_Catalog( "loot" )')
    items = (cat or {}).get("items", [])
    json.dump(items, open(os.path.join(OUT, "loot_catalog.json"), "w"), indent=1)
    print(len(items), "loot items")

    # Two guns, ammo, meds, gear.
    server.eval("server", 'Agent_TakeWeapons( "me" )')
    server.eval("server", 'Agent_Give( "me", "mp_weapon_r97" )')
    server.eval("server", 'Agent_Give( "me", "mp_weapon_wingman" )')
    want = ("ammo", "health", "shield", "syringe", "medkit", "battery", "cell", "armor", "helmet", "backpack", "knockdown", "incapshield", "grenade", "thermite", "frag", "arc")
    given = 0
    for it in items:
        ref = it.get("ref", "")
        if any(w in ref.lower() for w in want) and given < 40:
            server.eval("server", f'Agent_Give( "me", "{ref}", 2 )')
            given += 1
    server.eval("server", 'Agent_SetHealth( "me", 70, 50 )')
    time.sleep(1.0)
    server.eval("server", "WLog_Aim()")
    time.sleep(1.0)
    shot(client, "hud_idle", seen)
    client.cmd("+zoom")
    time.sleep(0.8)
    shot(client, "hud_ads", seen)
    client.cmd("-zoom")
    client.cmd("+attack")
    time.sleep(0.25)
    shot(client, "hud_firing", seen)
    client.cmd("-attack")
    time.sleep(0.5)
    client.cmd("+reload")
    time.sleep(0.1)
    client.cmd("-reload")
    time.sleep(0.5)
    shot(client, "hud_reloading", seen)
    time.sleep(3.0)
    ok, _, raw = client.eval("client", "OpenSurvivalInventory( GetLocalClientPlayer() )")
    print("inventory:", ok, raw.get("log") if isinstance(raw, dict) else raw)
    time.sleep(1.5)
    shot(client, "inventory", seen)
    client.cmd("escape")
    print("done")


def optics(guns):
    """ADS reference shots for scoped guns (`--optics r97,sniper`)."""
    os.makedirs(OUT, exist_ok=True)
    server = connect(37019, "agentlink.token", 900)
    client = connect(37020, "agentlink_client.token", 900)
    while True:
        ok, reply, raw = server.eval("server", "WLog_Setup( 400.0 )")
        if ok and reply and reply.get("ok"):
            break
        time.sleep(5)
    seen = set(glob.glob(os.path.join(ROOT, "**", "*.tga"), recursive=True))
    for g in guns:
        ref = g if g.startswith("mp_weapon_") else "mp_weapon_" + g
        server.eval("server", f'WLog_Give( "{ref}" )')
        server.eval("server", "WLog_Aim()")
        time.sleep(2.0)
        shot(client, f"hip_{g}", seen)
        client.cmd("+zoom")
        time.sleep(1.2)
        shot(client, f"ads_{g}", seen)
        client.cmd("-zoom")
        time.sleep(0.5)


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[1] == "--optics":
        optics(sys.argv[2].split(","))
    else:
        main()
