"""Give exported Apex UI images (RSX names them by asset GUID) their asset
paths, so the HUD can load e.g. rui/weapon_icons/r5/weapon_r97.

Candidate paths are harvested from text (weapon defs, datatables, the
reference scripts) and hashed with the engine's asset-name hash of
"ui_image/<path>.rpak". The hash comes from the bspconv reference tool
(AGPL), used as an external tool: it is imported from its own folder, not
copied here.

    python tools/name_ui_images.py

Writes <content>/ui/named/<path>.png and names.json (personal).
"""

import json
import os
import re
import shutil
import sys

CONTENT = os.environ.get("IW4L_APEX_CONTENT", r"F:\ApexContent")
# bspconv checkout (reference tool, AGPL): its tools/tf2common/guid.py.
BSPCONV = os.environ.get("BSPCONV_DIR", r"F:\Dev\GameReferences\bspconv")
sys.path.insert(0, os.path.join(BSPCONV, "tools", "tf2common"))
from guid import string_to_guid  # noqa: E402

SRC = os.path.join(CONTENT, "ui", "uiia", "ui_image")
DST = os.path.join(CONTENT, "ui", "named")
FLOWSTATE = os.environ.get("R5_FLOWSTATE", r"F:\R5Flowstate")
ROOTS = [
    os.path.join(os.path.dirname(BSPCONV), "s21-platform"),
    os.path.join(CONTENT, "game"),
    os.path.join(FLOWSTATE, "platform"),
    os.path.join(CONTENT, "lists"),
]
EXTS = (".nut", ".gnut", ".txt", ".csv", ".rson", ".json", ".menu", ".res", ".vdf", ".set")


def main():
    files = {f[:-4].upper(): f for f in os.listdir(SRC)}
    pat = re.compile(rb"rui/[A-Za-z0-9_\-/]+")
    names = set()
    for root in ROOTS:
        for dp, _dn, fn in os.walk(root):
            if "logs" in dp:
                continue
            for f in fn:
                if not f.lower().endswith(EXTS):
                    continue
                try:
                    data = open(os.path.join(dp, f), "rb").read()
                except OSError:
                    continue
                names.update(m.decode().rstrip("/") for m in pat.findall(data))
    hit = {}
    for n in names:
        g = "0X%016X" % string_to_guid(f"ui_image/{n}.rpak")
        if g in files:
            hit[n] = files[g]
    for n, f in hit.items():
        p = os.path.join(DST, *n.split("/")) + ".png"
        os.makedirs(os.path.dirname(p), exist_ok=True)
        shutil.copy(os.path.join(SRC, f), p)
    json.dump(hit, open(os.path.join(DST, "names.json"), "w"), indent=0, sort_keys=True)
    print(f"{len(names)} names harvested; {len(hit)} of {len(files)} images named")


if __name__ == "__main__":
    main()
