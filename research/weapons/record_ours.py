"""Record our game's (IW4L) audio output while it fires scripted bursts, to
compare gun sounds with the Flowstate recordings.

    python research/weapons/record_ours.py --carrier ump45_mp --seconds 150

Writes <content>/captures/sound/ours-<time>.wav.
"""

import argparse
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from sound_capture import OUT, Recorder  # noqa: E402
from paths import IW4L  # noqa: E402

IW4L = IW4L


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--carrier", default="ump45_mp")
    ap.add_argument("--seconds", type=float, default=150.0)
    ap.add_argument("--hold-s", type=float, default=1.0, help="seconds the trigger is held per burst")
    a = ap.parse_args()
    # `wait` counts tenths of a second.
    bursts = "; ".join([f"press +attack {a.hold_s}; wait 6; give ammo; wait 4"] * 3)
    extra = f"give weapon/iw4:{a.carrier}; wait 50; {bursts}"
    subprocess.run(["powershell", "-NoProfile", "-Command", "Get-Process iw4l -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false"])
    rec = Recorder()
    # Full volume for the test (a copy of the player's settings).
    env = dict(os.environ, IW4L_SETTINGS_PATH=os.path.join(OUT, "test_settings.cfg"))
    subprocess.Popen(
        ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", os.path.join(IW4L, "run-apex.ps1"), "-Extra", extra],
        cwd=IW4L,
        env=env,
    )
    time.sleep(a.seconds)
    path = os.path.join(OUT, time.strftime("ours-%Y%m%d-%H%M%S.wav"))
    rec.save(path)
    subprocess.run(["powershell", "-NoProfile", "-Command", "Get-Process iw4l -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false"])
    print("wrote", path)


if __name__ == "__main__":
    main()
