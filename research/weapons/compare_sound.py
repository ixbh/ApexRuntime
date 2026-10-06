"""Compare a gun's sustained fire between Flowstate and our game:
shot onsets, cadence, the level between shots (stutter shows as dips),
and a picture of both waveforms.

    python research/weapons/compare_sound.py <ours.wav> [--gun mp_weapon_r97]
"""

import argparse
import glob
import json
import os
import subprocess

import numpy as np

from sound_slice import SOUND, envelope, read


def bursts(env, rate, thr, min_len=0.3):
    loud = env > thr
    out, start = [], None
    for i in range(0, len(loud), int(rate * 0.01)):
        if loud[i] and start is None:
            start = i
        elif not loud[i] and start is not None:
            if (i - start) / rate >= min_len:
                out.append((start, i))
            start = None
    return out


def describe(name, x, rate, a, b):
    seg = x[a:b]
    env = envelope(seg, rate, 1.5)
    # Shot onsets: peaks of the envelope's rise.
    d = np.diff(env)
    rise = np.convolve(np.maximum(d, 0), np.ones(int(rate * 0.003)), mode="same")
    thr = rise.max() * 0.35
    peaks = []
    for i in range(1, len(rise) - 1):
        if rise[i] > thr and rise[i] >= rise[i - 1] and rise[i] >= rise[i + 1]:
            if not peaks or i - peaks[-1] > rate * 0.03:
                peaks.append(i)
    gaps = np.diff(peaks) / rate if len(peaks) > 1 else np.array([])
    # Level between shots, relative to the burst's typical level.
    lvl = envelope(seg, rate, 10)
    body = lvl[int(0.1 * rate) : max(int(0.1 * rate) + 1, len(lvl) - int(0.1 * rate))]
    dips = (body.min() / np.median(body)) if len(body) else 0
    print(
        f"{name}: {len(peaks)} shots in {len(seg) / rate:.2f}s, interval "
        f"{np.median(gaps) * 1000 if len(gaps) else 0:.1f} ms (spread {gaps.std() * 1000 if len(gaps) else 0:.1f} ms), "
        f"deepest dip {dips:.2f} of median level"
    )
    return seg


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ours")
    ap.add_argument("--gun", default="mp_weapon_r97")
    ap.add_argument("--run", default="")
    a = ap.parse_args()
    run = a.run or os.path.basename(sorted(glob.glob(os.path.join(SOUND, "2*.json")))[-1])[:-5]
    fx, rate = read(os.path.join(SOUND, f"{run}.wav"))
    ev = json.load(open(os.path.join(SOUND, f"{run}.json")))["events"]
    t = {e["kind"]: e["t"] for e in ev if e["gun"] == a.gun}
    fs = fx[int(t["auto"] * rate) : int((t["auto_end"] + 0.3) * rate)]
    ox, orate = read(a.ours)
    env = envelope(ox, orate, 5)
    thr = max(np.percentile(env, 50) * 6, env.max() * 0.08)
    found = bursts(env, orate, thr)
    print(f"our recording: {len(found)} bursts")
    f_seg = describe("Flowstate", fs, rate, 0, len(fs))
    segs = [describe(f"ours #{i + 1}", ox, orate, s, e) for i, (s, e) in enumerate(found[:4])]
    # Picture: Flowstate on top, our first burst under it, same scale.
    out = os.path.join(SOUND, "compare.png")
    tmp = [os.path.join(SOUND, "_cmp_fs.wav"), os.path.join(SOUND, "_cmp_ours.wav")]
    from sound_slice import write

    n = int(1.2 * rate)
    write(tmp[0], f_seg[:n] / max(1e-6, np.abs(f_seg).max()) * 0.9, rate)
    if segs:
        write(tmp[1], segs[0][:n] / max(1e-6, np.abs(segs[0]).max()) * 0.9, orate)
        subprocess.run(
            ["ffmpeg", "-loglevel", "error", "-y", "-i", tmp[0], "-i", tmp[1], "-filter_complex",
             "[0]showwavespic=s=1600x200:colors=orange:split_channels=0[a];[1]showwavespic=s=1600x200:colors=cyan[b];[a][b]vstack=2",
             out]
        )
        print("picture:", out)


if __name__ == "__main__":
    main()
