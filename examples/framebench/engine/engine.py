#!/usr/bin/env python3
"""A stand-in for a capture engine, with no interface of its own.

It replays synthetic frames into a ring and answers on a Unix socket: JSON
lines each way. It runs and can be tested without Moonframe, and it keeps
running when the app quits, so the app finds its real state again.

    python3 examples/framebench/engine/engine.py [--size 2048x1536]

Each frame is 16-bit Bayer in 12 bits, like a camera's: a moving gradient,
made in advance so that writing costs what a memory copy costs.
"""

import argparse
import json
import math
import os
import selectors
import signal
import socket
import struct
import sys
import time

# This folder is part of an app package, which ships source only.
sys.dont_write_bytecode = True

from ring import BAYER16, Ring  # noqa: E402

SOCKET = "/tmp/moonframe-framebench.sock"
RING = "framebench"
SLOTS = 6
# Different frames in the loop: enough to see movement.
LOOP = 24


def frames(width, height):
    """`LOOP` frames of a gradient that moves across a slowly lit field."""
    made = []
    for index in range(LOOP):
        shift = index / LOOP
        row = bytearray()
        for x in range(width):
            level = 0.5 + 0.5 * math.sin((x / width + shift) * 2 * math.pi)
            row += struct.pack("<H", int(level * 4095))
        block = bytes(row)
        rows = []
        for y in range(height):
            cut = (y * 2 + index * 8) % len(block) & ~1
            rows.append(block[cut:] + block[:cut])
        made.append(b"".join(rows))
    return made


class Engine:
    def __init__(self, width, height):
        self.width, self.height = width, height
        print(f"making {LOOP} frames of {width}x{height}...", flush=True)
        self.frames = frames(width, height)
        self.ring = Ring(RING, SLOTS, width * height * 2)
        self.rate = 25
        self.running = False
        self.gain = 1.0
        self.written = 0
        self.late = 0
        self.clients = {}
        self.selector = selectors.DefaultSelector()
        self.listener = socket.socket(socket.AF_UNIX)
        if os.path.exists(SOCKET):
            os.unlink(SOCKET)
        self.listener.bind(SOCKET)
        os.chmod(SOCKET, 0o600)
        self.listener.listen()
        self.listener.setblocking(False)
        self.selector.register(self.listener, selectors.EVENT_READ)

    def state(self):
        return {
            "running": self.running,
            "rate": self.rate,
            "gain": self.gain,
            "written": self.written,
            "late": self.late,
            "width": self.width,
            "height": self.height,
        }

    def tell(self):
        line = (json.dumps(self.state()) + "\n").encode()
        for client in list(self.clients):
            try:
                client.sendall(line)
            except OSError:
                self.drop(client)

    def drop(self, client):
        self.selector.unregister(client)
        self.clients.pop(client, None)
        client.close()

    def obey(self, line):
        message = json.loads(line).get("message")
        if isinstance(message, dict):
            if "rate" in message:
                self.rate = float(message["rate"])
            if "gain" in message:
                self.gain = float(message["gain"])
            if "connected" in message:
                pass
        elif message == "start":
            self.running, self.written, self.late = True, 0, 0
        elif message == "stop":
            self.running = False
        # Whatever it was, the app learns the state from this and from
        # nothing else: sending a command says nothing about its effect.
        self.tell()

    def poll(self, timeout):
        for key, _ in self.selector.select(timeout):
            if key.fileobj is self.listener:
                client, _ = self.listener.accept()
                client.setblocking(False)
                self.clients[client] = b""
                self.selector.register(client, selectors.EVENT_READ)
                continue
            client = key.fileobj
            try:
                data = client.recv(65536)
            except OSError:
                data = b""
            if not data:
                self.drop(client)
                continue
            self.clients[client] += data
            while b"\n" in self.clients[client]:
                line, self.clients[client] = self.clients[client].split(b"\n", 1)
                self.obey(line)

    def run(self):
        due = time.monotonic()
        told = due
        while True:
            now = time.monotonic()
            if self.running and now >= due:
                self.ring.write(
                    self.width,
                    self.height,
                    BAYER16,
                    self.frames[self.written % LOOP],
                )
                self.written += 1
                due += 1 / self.rate
                if now - due > 1 / self.rate:
                    self.late += 1
                    due = now
            if now - told >= 0.25:
                told = now
                self.tell()
            wait = max(due - time.monotonic(), 0) if self.running else 0.05
            self.poll(min(wait, 0.05))
            if not self.running:
                due = time.monotonic()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--size", default="2048x1536")
    args = parser.parse_args()
    width, height = (int(side) for side in args.size.split("x"))
    engine = Engine(width, height)
    signal.signal(signal.SIGTERM, lambda *_: raise_exit())
    print(f"listening on {SOCKET}", flush=True)
    try:
        engine.run()
    except (KeyboardInterrupt, SystemExit):
        pass
    finally:
        engine.ring.close()
        os.unlink(SOCKET)


def raise_exit():
    raise SystemExit


if __name__ == "__main__":
    main()
