"""Generate apex-inputbot scripts for automated R5Flowstate captures.

usage: python research/bot/make_scripts.py [--crouch c] [--sens 1.63]
Writes research/bot/scripts/*.json. Each attempt ends with a 180° turn so the
next one runs back the other way (a long flat lane is enough; no teleport).
Run with:  cargo run --release -p apex-inputbot -- research/bot/scripts/<name>.json
"""
import json, os, sys

args = sys.argv[1:]
def opt(name, default):
    return args[args.index(name) + 1] if name in args else default

CROUCH = opt('--crouch', 'c')      # a HOLD crouch key (C or ctrl); L is toggle
SENS = float(opt('--sens', '1.63'))
FPS = 165.0
FRAME = 1000.0 / FPS
OUT = os.path.join(os.path.dirname(__file__), 'scripts')
os.makedirs(OUT, exist_ok=True)


def turn_back(at):
    return {"at_ms": at, "turn_right_deg": 180.0, "over_ms": 250}


def deadslide():
    """Sprint 1.2 s, slide, jump after N frames (0..40 step 2), 2 runs each.
    Measures when a slide-jump keeps the slide boost (wiki: below 350 u/s or
    after 0.24 s) and the deadslide loss."""
    attempts = []
    for n in range(0, 41, 2):
        for rep in range(2):
            slide = 1500.0
            jump = slide + n * FRAME
            attempts.append({
                "label": f"deadslide jump {n} frames ({n * FRAME:.0f} ms) after crouch, run {rep + 1}",
                "steps": [
                    {"at_ms": 0, "down": "w"},
                    {"at_ms": slide, "down": CROUCH},
                    {"at_ms": jump, "tap": "space"},
                    {"at_ms": jump + 900, "up": CROUCH},
                    {"at_ms": jump + 1500, "up": "w"},
                    turn_back(jump + 1700),
                ],
            })
    return {"name": "deadslide", "sens": SENS, "countdown_s": 5, "gap_ms": 700, "attempts": attempts}


def lurch_angles():
    """Slide-jump straight ahead, release W, flick the view right by an exact
    angle at time T after the jump, then one W wheel notch 2 frames later.
    Known wish angle -> lurch turn and speed change vs angle and time."""
    attempts = []
    for t_ms in (150, 300, 400):
        for ang in (15, 30, 45, 60, 90, 120):
            slide = 1500.0
            jump = slide + 25 * FRAME           # well-boosted slide jump
            flick = jump + t_ms - 2 * FRAME
            attempts.append({
                "label": f"lurch W notch at {t_ms} ms, view {ang} deg right",
                "steps": [
                    {"at_ms": 0, "down": "w"},
                    {"at_ms": slide, "down": CROUCH},
                    {"at_ms": jump, "tap": "space"},
                    {"at_ms": jump + 20, "up": "w"},
                    {"at_ms": jump + 40, "up": CROUCH},
                    {"at_ms": flick, "turn_right_deg": float(ang), "over_ms": 0},
                    {"at_ms": jump + t_ms, "wheel": 1},
                    # turn back to face the way we came (180 - ang more).
                    {"at_ms": jump + 1400, "turn_right_deg": 180.0 - ang, "over_ms": 250},
                ],
            })
    return {"name": "lurch_angles", "sens": SENS, "countdown_s": 5, "gap_ms": 900, "attempts": attempts}


for script in (deadslide(), lurch_angles()):
    path = os.path.join(OUT, script["name"] + ".json")
    json.dump(script, open(path, "w"), indent=1)
    print(f"{path}: {len(script['attempts'])} attempts")
