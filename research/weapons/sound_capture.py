"""Record each gun's real mixed sound from offline Flowstate.

Plays every roster gun over the dev agent link (see capture.py) while
recording the PC's audio output (WASAPI loopback), and writes the
recording plus a timeline of what was done when. Nothing else should be
playing sound during the run. Aims at the sky so no hit sounds mix in.

    python research/weapons/sound_capture.py [--guns r97,wingman]

Output (personal, never committed): F:/ApexContent/captures/sound/<run>.wav
and <run>.json (events: gun, kind, seconds from the recording start).
"""

import argparse
import json
import os
import sys
import threading
import time
import wave

import pyaudiowpatch as pa

sys.path.insert(0, os.path.dirname(__file__))
from capture import connect  # noqa: E402

OUT = r"F:\ApexContent\captures\sound"


class Recorder:
    """Loopback recording of the default output, kept running with a silent
    stream so quiet gaps are recorded too (loopback skips silence)."""

    def __init__(self):
        self.p = pa.PyAudio()
        wasapi = self.p.get_host_api_info_by_type(pa.paWASAPI)
        out = self.p.get_device_info_by_index(wasapi["defaultOutputDevice"])
        lb = next(d for d in self.p.get_loopback_device_info_generator() if out["name"] in d["name"])
        self.rate = int(lb["defaultSampleRate"])
        self.channels = lb["maxInputChannels"]
        self.frames = []
        self.count = 0
        self.lock = threading.Lock()
        silence = b"\0" * (4 * 2 * 1024)
        self.keepalive = self.p.open(
            format=pa.paInt16, channels=2, rate=self.rate, output=True,
            output_device_index=out["index"], frames_per_buffer=1024,
            stream_callback=lambda *a: (silence[: a[1] * 4], pa.paContinue),
        )
        self.stream = self.p.open(
            format=pa.paInt16, channels=self.channels, rate=self.rate, input=True,
            input_device_index=lb["index"], frames_per_buffer=1024,
            stream_callback=self._on_audio,
        )
        self.t0 = time.time()
        print(f"recording {lb['name']} {self.rate} Hz x{self.channels}")

    def _on_audio(self, data, n, _info, _status):
        with self.lock:
            self.frames.append(data)
            self.count += n
        return (None, pa.paContinue)

    def now(self):
        """Seconds of audio recorded so far (the timeline clock)."""
        with self.lock:
            return self.count / self.rate

    def save(self, path):
        self.stream.stop_stream()
        self.keepalive.stop_stream()
        with wave.open(path, "wb") as w:
            w.setnchannels(self.channels)
            w.setsampwidth(2)
            w.setframerate(self.rate)
            w.writeframes(b"".join(self.frames))
        self.p.terminate()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--guns", default="")
    ap.add_argument("--wait", type=float, default=1200.0)
    args = ap.parse_args()
    if args.guns:
        guns = [g if g.startswith("mp_weapon_") else "mp_weapon_" + g for g in args.guns.split(",")]
    else:
        guns = [g["gun"] for g in json.load(open(r"F:\ApexContent\guns.json"))]

    server = connect(37019, "agentlink.token", args.wait)
    client = connect(37020, "agentlink_client.token", args.wait)
    # Keep sound when the game is not focused; no music.
    for line in ("sound_without_focus 1", "snd_mute_losefocus 0", "miles_music_volume 0"):
        client.cmd(line)
    # Wait until the player has spawned.
    deadline = time.time() + args.wait
    while True:
        ok, info, _ = server.eval("server", f'WLog_Give( "{guns[0]}" )')
        if ok and info and info.get("ok"):
            break
        if time.time() > deadline:
            raise SystemExit("no player")
        time.sleep(3)
    server.eval("server", 'Agent_God( "me", true )')

    os.makedirs(OUT, exist_ok=True)
    run = time.strftime("%Y%m%d-%H%M%S")
    rec = Recorder()
    events = []

    def mark(gun, kind):
        events.append({"gun": gun, "kind": kind, "t": round(rec.now(), 4)})

    try:
        time.sleep(2.0)
        for gun in guns:
            ok, info, raw = server.eval("server", f'WLog_Give( "{gun}" )')
            if not (ok and info and info.get("ok")):
                print(f"{gun}: give failed {raw}")
                continue
            mark(gun, "raise")
            print(gun, info)
            server.eval("server", 'Agent_Look( "me", <-70, 0, 0> )')
            time.sleep(2.0)
            semi = info.get("semi")
            charge = float(info.get("charge") or 0.0)

            def pull(hold):
                client.cmd("+attack")
                time.sleep(hold)
                client.cmd("-attack")

            # Single shots, well apart (clean sample with its full tail).
            for _ in range(3):
                server.eval("server", "WLog_Fill()")
                mark(gun, "single")
                pull(charge + 0.35 if charge > 0 else 0.03)
                time.sleep(1.8)
            # Sustained fire: hold (auto) or tap at the gun's rate (semi).
            server.eval("server", "WLog_Fill()")
            mark(gun, "auto")
            # Stop one round short of empty, so the gun does not reload on
            # its own and the reload below is a normal one.
            rate = max(float(info.get("rate") or 4.0), 0.5)
            clip = int(info.get("clip") or 0)
            shots = max(1, clip - 1) if clip > 0 else 16
            hold = min(1.6, shots / rate)
            if charge > 0:
                pull(charge + 0.35)
            elif semi:
                for _ in range(min(shots, int(1.6 * rate) + 1)):
                    pull(0.03)
                    time.sleep(max(0.03, 1.0 / rate - 0.03))
            else:
                pull(max(0.05, hold - 0.5 / rate))
            mark(gun, "auto_end")
            time.sleep(1.8)
            # Reload (partly empty after the burst).
            # The range refills magazines: leave one round so a reload runs.
            server.eval(
                "server",
                "try { GetPlayerArray()[0].GetActiveWeapon( eActiveInventorySlot.mainHand )"
                ".SetWeaponPrimaryClipCount( 1 ) } catch ( e ) {}",
            )
            time.sleep(0.1)
            mark(gun, "reload")
            client.cmd("+reload")
            time.sleep(0.15)
            client.cmd("-reload")
            time.sleep(4.8)
            mark(gun, "reload_end")

    except (OSError, RuntimeError) as e:
        # The game went away: keep what was recorded.
        print("stopped early:", e)
    time.sleep(1.0)
    wav = os.path.join(OUT, f"{run}.wav")
    rec.save(wav)
    json.dump({"rate": rec.rate, "events": events}, open(os.path.join(OUT, f"{run}.json"), "w"), indent=1)
    print("wrote", wav)


if __name__ == "__main__":
    main()
