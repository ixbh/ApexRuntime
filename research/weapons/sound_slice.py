"""Cut a sound_capture.py recording into per-gun sound files for IW4L.

For every gun in the run's timeline, writes to
F:/ApexContent/captures/sound/guns/<gun>/ (personal, never committed):

    fire_1..3.wav  single shots with their full tail (and any pump/lever)
    loop.wav       full-auto guns: a seamless loop of sustained fire
                   (a whole number of shots long)
    tail.wav       full-auto guns: the last shot of a burst and its tail
    reload.wav     the reload, from the key press
    raise.wav      drawing the gun

One gain for the whole run keeps the guns' loudness relative to each other.

    python research/weapons/sound_slice.py [run, default newest]
"""

import glob
import json
import os
import sys
import wave

import numpy as np

SOUND = r"F:\ApexContent\captures\sound"
WEAPONS = r"F:\ApexContent\captures\weapons.json"
PEAK = 0.89  # -1 dBFS for the loudest fire sample of the run


def read(path):
    with wave.open(path, "rb") as w:
        rate, ch = w.getframerate(), w.getnchannels()
        x = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float32) / 32768.0
    x = x.reshape(-1, ch)
    if ch > 2:
        x = x[:, :2]
    elif ch == 1:
        x = np.repeat(x, 2, axis=1)
    return x, rate


def write(path, x, rate):
    y = np.clip(x, -1.0, 1.0)
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes((y * 32767.0).astype(np.int16).tobytes())


def envelope(x, rate, ms=2.0):
    a = np.abs(x).max(axis=1)
    n = max(1, int(rate * ms / 1000))
    return np.convolve(a, np.ones(n) / n, mode="same")


def fade(x, rate, fin=0.002, fout=0.05):
    x = x.copy()
    a, b = int(rate * fin), int(rate * fout)
    if a and len(x) > a:
        x[:a] *= np.linspace(0, 1, a)[:, None]
    if b and len(x) > b:
        x[-b:] *= np.linspace(1, 0, b)[:, None]
    return x


def onsets(env, thr, start, end, gap):
    """Sample indices where the envelope rises through `thr` in [start, end),
    at least `gap` samples apart."""
    seg = env[start:end] >= thr
    rises = np.flatnonzero(seg[1:] & ~seg[:-1]) + start + 1
    out = []
    for r in rises:
        if not out or r - out[-1] >= gap:
            out.append(int(r))
    return out


def trim_silence(x, rate, thr):
    env = envelope(x, rate, 10)
    loud = np.flatnonzero(env >= thr)
    if len(loud) == 0:
        return x[: int(0.2 * rate)]
    return x[: min(len(x), loud[-1] + int(0.15 * rate))]


def main():
    if len(sys.argv) > 1:
        run = sys.argv[1]
    else:
        run = os.path.basename(sorted(glob.glob(os.path.join(SOUND, "*.json")))[-1])[:-5]
    x, rate = read(os.path.join(SOUND, f"{run}.wav"))
    events = json.load(open(os.path.join(SOUND, f"{run}.json")))["events"]
    settings, measured = {}, {}
    if os.path.exists(WEAPONS):
        data = json.load(open(WEAPONS))
        settings = {g: r.get("settings", {}) for g, r in data.items()}
        measured = {g: (r.get("hip") or {}).get("rate_measured") for g, r in data.items()}
    env = envelope(x, rate)
    noise = float(np.percentile(env, 20))
    S = lambda t: max(0, min(len(x), int(t * rate)))  # noqa: E731

    by_gun = {}
    for e in events:
        by_gun.setdefault(e["gun"], []).append(e)

    # One gain for the whole run, from the loudest single shot.
    loudest = 1e-6
    for evs in by_gun.values():
        for e in evs:
            if e["kind"] == "single":
                loudest = max(loudest, float(np.abs(x[S(e["t"]) : S(e["t"] + 1.0)]).max()))
    gain = PEAK / loudest
    print(f"run {run}: {len(by_gun)} guns, noise {noise:.4f}, gain {gain:.2f}")

    for gun, evs in by_gun.items():
        out = os.path.join(SOUND, "guns", gun)
        os.makedirs(out, exist_ok=True)
        t = {}
        for e in evs:
            t.setdefault(e["kind"], []).append(e["t"])
        s = settings.get(gun, {})
        num = lambda k, d: float(s[k]) if k in s and s[k] not in ("?", "<missing>") else d  # noqa: E731
        auto = num("is_semi_auto", 0) == 0 and num("burst_fire_count", 0) == 0 and num("charge_time", 0) == 0
        thr = max(noise * 8, 0.02 / gain)

        # Single shots: from the first onset after the key press.
        n = 0
        for ts in t.get("single", []):
            ons = onsets(env, thr, S(ts - 0.02), S(ts + 1.2), int(0.05 * rate))
            if not ons:
                continue
            a = ons[0] - int(0.004 * rate)
            clip = trim_silence(x[a : min(a + int(1.7 * rate), S(ts + 1.78))] * gain, rate, thr * gain)
            n += 1
            write(os.path.join(out, f"fire_{n}.wav"), fade(clip, rate), rate)

        # Sustained fire (full-auto): a loop a whole number of shots long,
        # and the last shot with its tail.
        if auto and t.get("auto") and t.get("auto_end"):
            a0, a1 = S(t["auto"][0]), S(t["auto_end"][0] + 0.15)
            ons = onsets(env, thr, a0, a1, int(0.03 * rate))
            # Shots overlap in sustained fire: step by the measured rate.
            shots_per_s = measured.get(gun) or num("fire_rate", 10.0)
            period = max(1, int(round(rate / shots_per_s)))
            if ons and a1 - ons[0] > 6 * period:
                start = ons[0] + 2 * period - int(0.004 * rate)
                count = max(1, int(0.6 * rate / period))
                loop = x[start : start + count * period] * gain
                # Seamless wrap: crossfade the end into the start.
                xf = min(int(0.008 * rate), len(loop) // 4)
                w = np.linspace(0, 1, xf)[:, None]
                loop[:xf] = loop[:xf] * w + loop[-xf:] * (1 - w)
                loop = loop[:-xf]
                write(os.path.join(out, "loop.wav"), loop, rate)
                # One shot of sustained fire (exactly one period, so each
                # shot can play its own copy in sync with the gun).
                one = x[start : start + period] * gain
                write(os.path.join(out, "auto_shot.wav"), fade(one, rate, 0.001, 0.004), rate)
                # The last shot is within a period of the trigger release.
                n_shots = int((S(t["auto_end"][0]) - ons[0]) / period)
                last = ons[0] + max(0, n_shots - 1) * period - int(0.004 * rate)
                tail = trim_silence(x[last : last + int(1.6 * rate)] * gain, rate, thr * gain)
                # The decay alone (after the last shot's own period).
                decay = trim_silence(x[last + period : last + int(1.6 * rate)] * gain, rate, thr * gain)
                write(os.path.join(out, "decay.wav"), fade(decay, rate, 0.004, 0.05), rate)
                write(os.path.join(out, "tail.wav"), fade(tail, rate), rate)
        if t.get("reload"):
            r = S(t["reload"][0])
            end = S(t["reload_end"][0]) if t.get("reload_end") else r + int(4.6 * rate)
            write(os.path.join(out, "reload.wav"), fade(trim_silence(x[r:end] * gain, rate, thr * gain), rate), rate)
        if t.get("raise"):
            r = S(t["raise"][0])
            write(os.path.join(out, "raise.wav"), fade(trim_silence(x[r : r + int(1.4 * rate)] * gain, rate, thr * gain), rate), rate)
        print(f"{gun:28s} fire x{n} {'auto' if auto else 'single'} files: {sorted(os.listdir(out))}")


if __name__ == "__main__":
    main()
