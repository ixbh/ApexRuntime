"""Export Apex guns (viewmodel + textures + animations) from the user's
offline Flowstate install with RSX, driven by the exported weapon
definitions, and write <content>/guns.json for the game. Personal use only:
everything goes to the content folder (outside the repos), never committed.

    python tools/export_guns.py            # whole roster: export + manifest
    python tools/export_guns.py --manifest # manifest only (no RSX)
    python tools/export_guns.py mp_weapon_r97 ...   # export just these

Needs the definitions first (RSX `wepn` export of `mp_weapon_` into
<content>/game, see docs/ASSET-PIPELINE.md). Paths come from env:
IW4L_APEX_CONTENT (default F:/ApexContent), APEX_RSX (rsx_nogui.exe),
APEX_PAKS (Flowstate paks/Win64).
"""
import json
import os
import re
import subprocess
import sys

CONTENT = os.environ.get("IW4L_APEX_CONTENT", "F:/ApexContent")
RSX = os.environ.get("APEX_RSX", "F:/Tools/rsx_2.3.0/rsx_nogui.exe")
PAKS = os.environ.get("APEX_PAKS", "F:/R5Flowstate/paks/Win64")
GAME = os.path.join(CONTENT, "game")
DEFS = os.path.join(GAME, "weapon")

# Internal name -> display name (the battle-royale roster), menu order.
ROSTER = {
    "mp_weapon_rspn101": "R-301",
    "mp_weapon_vinson": "Flatline",
    "mp_weapon_energy_ar": "Havoc",
    "mp_weapon_hemlok": "Hemlok",
    "mp_weapon_nemesis": "Nemesis",
    "mp_weapon_r97": "R-99",
    "mp_weapon_volt_smg": "Volt",
    "mp_weapon_alternator_smg": "Alternator",
    "mp_weapon_pdw": "Prowler",
    "mp_weapon_car": "C.A.R.",
    "mp_weapon_lmg": "Spitfire",
    "mp_weapon_esaw": "Devotion",
    "mp_weapon_lstar": "L-STAR",
    "mp_weapon_dragon_lmg": "Rampage",
    "mp_weapon_g2": "G7 Scout",
    "mp_weapon_doubletake": "Triple Take",
    "mp_weapon_3030": "30-30 Repeater",
    "mp_weapon_bow": "Bocek",
    "mp_weapon_dmr": "Longbow",
    "mp_weapon_sniper": "Kraber",
    "mp_weapon_sentinel": "Sentinel",
    "mp_weapon_defender": "Charge Rifle",
    "mp_weapon_shotgun": "EVA-8",
    "mp_weapon_mastiff": "Mastiff",
    "mp_weapon_energy_shotgun": "Peacekeeper",
    "mp_weapon_shotgun_pistol": "Mozambique",
    "mp_weapon_autopistol": "RE-45",
    "mp_weapon_semipistol": "P2020",
    "mp_weapon_wingman": "Wingman",
}

# Menu category per gun (also picks a fitting carrier weapon class in hosts).
CATEGORY = {
    "mp_weapon_rspn101": "assault", "mp_weapon_vinson": "assault", "mp_weapon_energy_ar": "assault",
    "mp_weapon_hemlok": "assault", "mp_weapon_nemesis": "assault",
    "mp_weapon_r97": "smg", "mp_weapon_volt_smg": "smg", "mp_weapon_alternator_smg": "smg",
    "mp_weapon_pdw": "smg", "mp_weapon_car": "smg",
    "mp_weapon_lmg": "lmg", "mp_weapon_esaw": "lmg", "mp_weapon_lstar": "lmg", "mp_weapon_dragon_lmg": "lmg",
    "mp_weapon_g2": "marksman", "mp_weapon_doubletake": "marksman", "mp_weapon_3030": "marksman",
    "mp_weapon_bow": "marksman",
    "mp_weapon_dmr": "sniper", "mp_weapon_sniper": "sniper", "mp_weapon_sentinel": "sniper",
    "mp_weapon_defender": "sniper",
    "mp_weapon_shotgun": "shotgun", "mp_weapon_mastiff": "shotgun", "mp_weapon_energy_shotgun": "shotgun",
    "mp_weapon_shotgun_pistol": "shotgun",
    "mp_weapon_autopistol": "pistol", "mp_weapon_semipistol": "pistol", "mp_weapon_wingman": "pistol",
}

# Guns whose viewmodel moved to the newer "techart" layout keep their
# sequences under the older internal folder.
ANIM_OVERRIDE = {
    "mp_weapon_vinson": "weapons/vinson/ptpov_vinson",
    "mp_weapon_nemesis": "weapons/nemesis/ptpov_nemesis",
    "mp_weapon_dragon_lmg": "weapons/dragon_lmg/ptpov_dragon_lmg",
    "mp_weapon_pdw": "weapons/prowler_smg/ptpov_prowler_smg",
    "mp_weapon_car": "weapons/car/car_base_v",
    "mp_weapon_mastiff": "weapons/mastiff_stgn/ptpov_mastiff",
    "mp_weapon_g2": "weapons/g2/ptpov_g2a4",
}


def model_of(gun, key):
    """Top-level `key` of a definition (one tab deep; mods are deeper)."""
    path = os.path.join(DEFS, gun + ".txt")
    try:
        text = open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        return None
    m = re.search(r'^	"' + key + r'"\s*"([^"]+)"', text, re.M)
    return m.group(1).replace("\\", "/") if m else None


def viewmodel_of(gun):
    return model_of(gun, "viewmodel")


def rsx(args, tag):
    log = os.path.join(CONTENT, "lists", f"rsx_{tag}.txt")
    with open(log, "w", encoding="utf-8") as out:
        p = subprocess.run([RSX, "-nogui", *args], cwd=os.path.dirname(RSX),
                           stdout=out, stderr=subprocess.STDOUT)
    print(f"{tag}: exit {p.returncode} (log {log})")
    return p.returncode


def win(path):
    return path.replace("/", "\\")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    manifest_only = "--manifest" in sys.argv
    guns = args or list(ROSTER)
    models, anims, manifest = [], [], []
    for g in guns:
        vm = viewmodel_of(g)
        if not vm:
            print(f"{g}: no definition or viewmodel; skipped")
            continue
        folder, stem = vm.rsplit("/", 1)
        stem = stem.rsplit(".", 1)[0]
        weapon_dir = folder.split("/", 1)[1] if "/" in folder else folder
        anim_dir = ANIM_OVERRIDE.get(g, f"{weapon_dir}/{stem}")
        models.append(win(vm))
        wm = model_of(g, "playermodel")
        if wm:
            models.append(win(wm))
        anims.append(win(anim_dir) + "\\")
        manifest.append({
            "gun": g,
            "name": ROSTER.get(g, g),
            "category": CATEGORY.get(g, "assault"),
            "definition": f"game/weapon/{g}.txt",
            "model": f"game/{folder}/{stem}_LOD0.cast",
            "world_model": (f"game/{wm.rsplit('.', 1)[0]}_LOD0.cast" if wm else None),
            "anims": [f"game/animseq/{anim_dir}", f"game/animseq_derived/{anim_dir}"],
        })
        print(f"{g:32s} {ROSTER.get(g, '?'):15s} {vm}")
    if not args:
        with open(os.path.join(CONTENT, "guns.json"), "w", encoding="utf-8") as f:
            json.dump(manifest, f, indent=1)
        print(f"wrote {len(manifest)} guns to guns.json")
    if manifest_only:
        return
    common = os.path.join(PAKS, "common.rpak")
    rsx(["-export", "-exportfullpaths", "-matltextures", "--loadwhitelist", "mdl_,matl,txtr",
         "--exporttypes", "mdl_", "--exportfilter", ",".join(models), "--exportdir", GAME, common],
        "guns_models")
    rsx(["-export", "-exportfullpaths", "--loadwhitelist", "mdl_,arig,aseq,asqd",
         "--exporttypes", "aseq", "--exportfilter", ",".join(anims), "--exportdir", GAME, common],
        "guns_anims")


if __name__ == "__main__":
    main()
