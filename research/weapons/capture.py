"""Weapon capture in offline R5Flowstate over the dev agent link.

Needs Flowstate started with -devsdk (agent link on 127.0.0.1: dedicated
server 37019, client 37020) and the ApexRuntime.WeaponLog mod installed
(research/weapons/flowstate_mod). Drives only console/script commands, no
OS input, so the game can run unfocused on another monitor.

For every gun in F:/ApexContent/guns.json (or --guns): give it, log its live
settings, fire at a dummy from the hip (hold for auto, taps for semi, hold +
release for charge), reload, then the same aimed down sights. The mod
writes [WLOG] lines to the client and server script.log; afterwards they
are copied to --out.

    python research/weapons/capture.py [--guns r97,wingman] [--dist 400]
"""

import argparse
import base64
import glob
import json
import os
import shutil
import socket
import time

ROOT = r"F:\R5Flowstate"


class Link:
    def __init__(self, port, token_file):
        token = open(os.path.join(ROOT, "platform", token_file)).read().strip()
        self.sock = socket.create_connection(("127.0.0.1", port), timeout=30)
        self.buf = b""
        self.next_id = 1
        self.send_line(f"AUTH {token}")
        reply = self.read_line()
        while reply.startswith("AGENTLINK"):  # greeting
            reply = self.read_line()
        if not reply.startswith("OK"):
            raise RuntimeError(f"auth on {port}: {reply}")

    def send_line(self, line):
        self.sock.sendall(line.encode() + b"\n")

    def read_line(self):
        while b"\n" not in self.buf:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise RuntimeError("link closed")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return line.decode(errors="replace")

    def request(self, op, rest):
        rid = self.next_id
        self.next_id += 1
        self.send_line(f"{op} {rid} {rest}")
        while True:
            line = self.read_line()
            if line.startswith(f"RESULT {rid} "):
                return line

    def eval(self, ctx, code):
        b64 = base64.b64encode(code.encode()).decode()
        line = self.request("EVAL64", f"{ctx} {b64}")
        # RESULT <id> ok|err {json}
        parts = line.split(" ", 3)
        body = parts[3] if len(parts) > 3 else "{}"
        try:
            data = json.loads(body)
        except json.JSONDecodeError:
            data = {"raw": body}
        reply = data.get("reply") if isinstance(data, dict) else None
        if isinstance(reply, str):
            try:
                return parts[2] == "ok", json.loads(reply), data
            except json.JSONDecodeError:
                return parts[2] == "ok", {"raw": reply}, data
        return parts[2] == "ok", None, data

    def cmd(self, line):
        return self.request("CMD", line)


def connect(port, token, wait):
    deadline = time.time() + wait
    while True:
        try:
            return Link(port, token)
        except (OSError, RuntimeError) as e:
            if time.time() > deadline:
                raise
            print(f"waiting for agent link {port}: {e}")
            time.sleep(5)


def newest_dir(role):
    dirs = glob.glob(os.path.join(ROOT, "platform", "logs", role, "*"))
    return max(dirs, key=os.path.getmtime) if dirs else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--guns", default="")
    ap.add_argument("--dist", type=float, default=400.0, help="dummy distance (units)")
    ap.add_argument("--out", default=r"F:\ApexContent\captures")
    ap.add_argument("--wait", type=float, default=900.0, help="seconds to wait for the game")
    args = ap.parse_args()

    if args.guns:
        guns = [g if g.startswith("mp_weapon_") else "mp_weapon_" + g for g in args.guns.split(",")]
    else:
        guns = [g["gun"] for g in json.load(open(r"F:\ApexContent\guns.json"))]

    server = connect(37019, "agentlink.token", args.wait)
    client = connect(37020, "agentlink_client.token", args.wait)
    print("links up")

    # Wait until the player exists on the server.
    deadline = time.time() + args.wait
    while True:
        ok, reply, raw = server.eval("server", "WLog_Setup( %.1f )" % args.dist)
        if ok and reply and reply.get("ok"):
            break
        if time.time() > deadline:
            raise SystemExit(f"setup failed: {raw}")
        print("waiting for the player / mod:", raw)
        time.sleep(5)
    server.eval("server", 'Agent_God( "me", true )')

    started = time.strftime("%Y%m%d-%H%M%S")
    plan = []
    for gun in guns:
        ok, info, raw = server.eval("server", f'WLog_Give( "{gun}" )')
        if not (ok and info and info.get("ok")):
            print(f"{gun}: give failed {raw}")
            plan.append({"gun": gun, "ok": False})
            continue
        print(f"{gun}: {info}")
        time.sleep(1.5)  # raise
        server.eval("server", "WLog_Dump()")
        semi = info.get("semi")
        charge = float(info.get("charge") or 0.0)

        for mode in ("hip", "ads"):
            server.eval("server", "WLog_Fill()")
            server.eval("server", "WLog_Aim()")
            client.eval("client", f'WLog_Start( "{gun}:{mode}" )')
            server.eval("server", f'WLog_Note( "{gun} {mode} start" )')
            if mode == "ads":
                client.cmd("+zoom")
                time.sleep(0.8)
            end = time.time() + (5.0 if mode == "hip" else 3.0)
            if charge > 0.0:
                while time.time() < end:
                    client.cmd("+attack")
                    time.sleep(charge + 0.4)
                    client.cmd("-attack")
                    time.sleep(0.6)
            elif semi:
                while time.time() < end:
                    client.cmd("+attack")
                    time.sleep(0.03)
                    client.cmd("-attack")
                    time.sleep(0.03)
            else:
                client.cmd("+attack")
                time.sleep(max(0.0, end - time.time()))
                client.cmd("-attack")
            if mode == "ads":
                client.cmd("-zoom")
            time.sleep(0.4)
            server.eval("server", f'WLog_Note( "{gun} {mode} reload" )')
            client.cmd("+reload")
            time.sleep(0.15)
            client.cmd("-reload")
            time.sleep(5.0)
            client.eval("client", "WLog_Stop()")
        plan.append({"gun": gun, "ok": True, **info})

    os.makedirs(args.out, exist_ok=True)
    for role in ("client", "server"):
        d = newest_dir(role)
        if d:
            for f in glob.glob(os.path.join(d, "*.log")):
                shutil.copy(f, os.path.join(args.out, f"{started}-{role}-{os.path.basename(f)}"))
    json.dump(plan, open(os.path.join(args.out, f"{started}-plan.json"), "w"), indent=1)
    print("done:", args.out)


if __name__ == "__main__":
    main()
