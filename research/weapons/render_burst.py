"""Render offline what IW4L plays for a held burst (same sound files, shots
at the engine's fire rate, start times quantised to game frames) with a
few playback methods, next to Flowstate's recorded burst, and measure the
level dips between shots (stutter) and the cadence.

    python research/weapons/render_burst.py --gun mp_weapon_r97 --rate 18 --shots 17
"""

import argparse
import json
import os

import numpy as np

from sound_slice import SOUND, envelope, read, write

FPS = 165.0


def mix(clips_at, length, rate):
    out = np.zeros((length, 2), dtype=np.float32)
    for t, clip, speed in clips_at:
        if speed != 1.0:
            idx = np.arange(0, len(clip) - 1, speed)
            clip = np.stack([np.interp(idx, np.arange(len(clip)), clip[:, c]) for c in range(2)], axis=1)
        i = int(t * rate)
        n = min(len(clip), length - i)
        if n > 0:
            out[i : i + n] += clip[:n]
    return out


def shot_times(n, interval):
    # The engine fires on game frames: each shot lands on the first frame
    # at or after its exact time.
    return [np.ceil(k * interval * FPS) / FPS for k in range(n)]


def dips(x, rate, start, end):
    lvl = envelope(x[int(start * rate) : int(end * rate)], rate, 8)
    return float(lvl.min() / np.median(lvl)) if len(lvl) else 0.0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gun", default="mp_weapon_r97")
    ap.add_argument("--rate", type=float, default=18.0, help="engine fire rate")
    ap.add_argument("--shots", type=int, default=17)
    ap.add_argument("--run", default="20261005-213715")
    a = ap.parse_args()
    d = os.path.join(SOUND, "guns", a.gun)
    clip = lambda n: read(os.path.join(d, n))[0]  # noqa: E731
    rate = read(os.path.join(d, "fire_1.wav"))[1]
    fire, auto_shot, loop, tail, decay = (clip(n) for n in ("fire_1.wav", "auto_shot.wav", "loop.wav", "tail.wav", "decay.wav"))
    interval = 1.0 / a.rate
    times = shot_times(a.shots, interval)
    end = times[-1] + interval
    length = int((end + 1.6) * rate)
    loop_period = len(auto_shot) / rate

    methods = {}
    # A: what the game plays now: first shot sample, one auto_shot per shot, decay.
    methods["per_shot"] = mix([(times[0], fire, 1.0)] + [(t, auto_shot, 1.0) for t in times[1:]] + [(end, decay, 1.0)], length, rate)
    # B: loop from the 2nd shot at its recorded speed until release, then tail.
    reps = int(np.ceil((end - times[1]) / (len(loop) / rate))) + 1
    looped = np.concatenate([loop] * reps)[: int((end - times[1]) * rate)]
    methods["loop_recorded_speed"] = mix([(times[0], fire, 1.0), (times[1], looped, 1.0), (end - interval, tail, 1.0)], length, rate)
    # C: what the game plays now: the loop resampled to the engine's rate
    # (speed = engine rate / recorded cadence), then the decay.
    loop_rate = json.load(open(os.path.join(d, "loop.json")))["shots_per_second"]
    speed = a.rate / loop_rate
    span = int((end - times[1]) * rate)
    src = np.concatenate([loop] * (reps + 2))
    idx = np.arange(0, span) * speed
    looped_c = np.stack([np.interp(idx, np.arange(len(src)), src[:, c]) for c in range(2)], axis=1).astype(np.float32)
    methods["loop_rate_matched"] = mix([(times[0], fire, 1.0), (times[1], looped_c, 1.0), (end + interval, decay, 1.0)], length, rate)
    # D: full single-shot sample per shot (overlapping tails).
    methods["full_sample_per_shot"] = mix([(t, fire * 0.6, 1.0) for t in times] , length, rate)

    fx, frate = read(os.path.join(SOUND, f"{a.run}.wav"))
    ev = {e["kind"]: e["t"] for e in json.load(open(os.path.join(SOUND, f"{a.run}.json")))["events"] if e["gun"] == a.gun}
    fs = fx[int(ev["auto"] * frate) : int((ev["auto_end"] + 1.0) * frate)]
    lvl = envelope(fs, frate, 8)
    on = np.argmax(lvl > lvl.max() * 0.2) / frate
    print(f"Flowstate burst: dip {dips(fs, frate, on + 0.15, on + 0.8):.2f}  (recorded period {loop_period * 1000:.1f} ms, engine interval {interval * 1000:.1f} ms)")
    for name, x in methods.items():
        print(f"{name:22s} dip {dips(x, rate, 0.15, end - 0.05):.2f}")
        write(os.path.join(SOUND, f"burst_{a.gun}_{name}.wav"), x / max(1e-6, np.abs(x).max()) * 0.9, rate)
    write(os.path.join(SOUND, f"burst_{a.gun}_flowstate.wav"), fs / max(1e-6, np.abs(fs).max()) * 0.9, frate)
    print("wrote burst_*.wav in", SOUND)


if __name__ == "__main__":
    main()
