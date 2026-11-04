#!/usr/bin/env python3
"""A pretend film scanner: the app's engine, with no interface of its own.

It pulls a strip of synthetic film past a window, writes what the window shows
to a ring in shared memory for `ui.video`, and prints JSON lines of what it
measured from that same picture. Messages arrive as JSON lines on stdin.

It keeps its place in a file, so a crash loses nothing: the app starts it
again and the scan carries on. Closing stdin is its cue to save and leave.
"""

import array
import json
import math
import os
import queue
import random
import sys
import tempfile
import threading
import time

# This folder is part of an app package, which ships source only, and a file
# written here would count as a change to the provider.
sys.dont_write_bytecode = True

from ring import RGBA16, Ring  # noqa: E402

RING = "lightbox"
SLOTS = 4
WIDTH, HEIGHT = 640, 360
# The strip wraps around, so a scan can be any length.
STRIP = 1280
RATE = 24
TOTAL = 720
# Pixels the film moves each frame.
PULL = 3
# Frames between readings, and between saving the scan's place.
READING = 3
SAVING = RATE
# A sample every this many pixels feeds the instruments.
COLUMN_STEP = 5
ROW_STEP = 6
BINS = 32
POINTS_PER_COLUMN = 3
FRAME_LINE = 320
PLACE = os.path.join(
    tempfile.gettempdir(),
    f"moonframe-lightbox-{os.environ.get('MOONFRAME_PROFILE', 'default')}.json",
)


def hill(u, depth):
    """The height of a range of hills at `u`, which repeats each turn."""
    turn = 2 * math.pi * u
    return (
        0.62
        + depth * 0.1
        + 0.07 * math.sin(turn * 3 + depth * 5)
        + 0.035 * math.sin(turn * 7 + depth * 2)
        + 0.012 * math.sin(turn * 19)
    )


def pixel(x, v, far, near):
    """Red, green and blue from 0 to 1 at column `x`, `v` down the frame."""
    if x % FRAME_LINE < 6:
        return 0.02, 0.02, 0.025
    u = x / STRIP
    if v > near:
        shade = 0.9 - (v - near) * 1.1
        return 0.07 * shade, 0.1 * shade, 0.09 * shade
    if v > far:
        return 0.3, 0.19, 0.34
    glow = 1 - v / far
    red, green, blue = 0.98 - 0.5 * glow, 0.58 - 0.12 * glow, 0.3 + 0.42 * glow
    across = min(abs(u - 0.3), abs(u - 0.8)) * STRIP / HEIGHT
    gap = math.hypot(across, v - 0.46)
    if gap < 0.09:
        return 1.0, 0.93, 0.72
    lit = max(0.0, 1 - gap / 0.42) ** 2 * 0.55
    return min(red + lit, 1.0), min(green + lit * 0.8, 1.0), min(blue + lit * 0.3, 1.0)


class Film:
    """The strip as rows of 16-bit pixels, and what the instruments read."""

    def __init__(self):
        far = [hill(x / STRIP, 0.0) for x in range(STRIP)]
        near = [hill(x / STRIP + 0.37, 1.1) for x in range(STRIP)]
        self.rows = []
        self.columns = []
        samples = [[] for _ in range(STRIP // COLUMN_STEP)]
        for y in range(HEIGHT):
            v = y / HEIGHT
            row = array.array("H")
            for x in range(STRIP):
                red, green, blue = pixel(x, v, far[x], near[x])
                row.extend((int(red * 65535), int(green * 65535), int(blue * 65535), 65535))
                if x % COLUMN_STEP == 0 and y % ROW_STEP == 0:
                    samples[x // COLUMN_STEP].append((red, green, blue))
            data = row.tobytes()
            # Twice around, so a window over the seam is one slice.
            self.rows.append(data + data[: WIDTH * 8])
        for column in samples:
            self.columns.append(measure(column))

    def frame(self, at):
        start = at % STRIP * 8
        end = start + WIDTH * 8
        return b"".join(row[start:end] for row in self.rows)


def measure(samples):
    """One column's lows, highs, tones and chroma."""
    low = [min(sample[channel] for sample in samples) for channel in range(3)]
    high = [max(sample[channel] for sample in samples) for channel in range(3)]
    bins = [[0] * BINS for _ in range(3)]
    for sample in samples:
        for channel in range(3):
            bins[channel][min(int(sample[channel] * BINS), BINS - 1)] += 1
    chroma = []
    for red, green, blue in samples[:: max(len(samples) // POINTS_PER_COLUMN, 1)]:
        luma = 0.2126 * red + 0.7152 * green + 0.0722 * blue
        chroma.append(((blue - luma) / 0.9278, (red - luma) / 0.7874))
    return low, high, bins, chroma


def reading(film, at, exposure):
    """What the window shows at `at`, measured, with `exposure` applied."""
    count = WIDTH // COLUMN_STEP
    first = at % STRIP // COLUMN_STEP
    total = len(film.columns)
    columns = [film.columns[(first + index) % total] for index in range(count)]

    def level(value):
        return round(min(value * exposure, 1.0), 3)

    waveform = [
        {
            "low": [level(column[0][channel]) for column in columns],
            "high": [level(column[1][channel]) for column in columns],
        }
        for channel in range(3)
    ]
    tones = [[0] * BINS for _ in range(3)]
    for column in columns:
        for channel in range(3):
            for bin, amount in enumerate(column[2][channel]):
                tones[channel][min(int(bin * exposure), BINS - 1)] += amount
    points = []
    for column in columns[::2]:
        for cb, cr in column[3]:
            points.append(round(max(min(cb * exposure, 1), -1), 3))
            points.append(round(max(min(cr * exposure, 1), -1), 3))
    return {"waveform": waveform, "tones": tones, "scope": points}


class Sound:
    """Two channels of a soundtrack that isn't there, with held peaks."""

    def __init__(self):
        self.peaks = [-60.0, -60.0]

    def levels(self, frame, running):
        channels = []
        for index, name in enumerate(("L", "R")):
            level = -60.0
            if running:
                level = -17 + 8 * math.sin(frame / (11 + index * 4)) + random.uniform(-3, 3)
            self.peaks[index] = max(level, self.peaks[index] - 0.8)
            channels.append({"name": name, "level": round(level, 1), "peak": round(self.peaks[index], 1)})
        return channels


class Scan:
    """Where the scan is, kept in a file so that a crash loses nothing."""

    def __init__(self):
        self.frame, self.running, self.done, self.crashes = 0, False, False, 0
        try:
            with open(PLACE) as file:
                saved = json.load(file)
        except (OSError, ValueError):
            return
        self.frame, self.running = saved["frame"], saved["running"]
        self.crashes = saved["crashes"]
        if self.running and not saved["clean"]:
            self.crashes += 1

    def save(self, clean):
        place = {
            "frame": self.frame,
            "running": self.running,
            "crashes": self.crashes,
            "clean": clean,
        }
        with open(PLACE, "w") as file:
            json.dump(place, file)

    def obey(self, message):
        if message == "start" and not self.running:
            if self.done or self.frame >= TOTAL:
                self.frame = 0
            self.running, self.done, self.crashes = True, False, 0
        elif message == "stop":
            self.running = False
        elif message == "crash":
            # As a bug would: no chance to say goodbye.
            self.save(False)
            print("crashing, as asked", file=sys.stderr, flush=True)
            os._exit(3)
        self.save(True)


def listen(messages):
    """Hands each line of stdin over; `None` once it closes."""
    for line in sys.stdin:
        try:
            messages.put(json.loads(line)["message"])
        except (ValueError, KeyError):
            print(f"not a message: {line!r}", file=sys.stderr, flush=True)
    messages.put(None)


def main():
    film = Film()
    ring = Ring(RING, SLOTS, WIDTH * HEIGHT * 8)
    scan, sound = Scan(), Sound()
    exposure = 1.0
    messages = queue.Queue()
    threading.Thread(target=listen, args=(messages,), daemon=True).start()
    tick = 0
    next_at = time.monotonic()
    while True:
        while not messages.empty():
            message = messages.get()
            if message is None:
                scan.save(True)
                ring.close()
                return
            if isinstance(message, dict) and "exposure" in message:
                exposure = float(message["exposure"])
            else:
                scan.obey(message)
        if scan.running:
            scan.frame += 1
            if scan.frame % SAVING == 0:
                scan.save(False)
            if scan.frame >= TOTAL:
                scan.running, scan.done = False, True
                scan.save(True)
        at = scan.frame * PULL
        ring.write(WIDTH, HEIGHT, RGBA16, film.frame(at))
        if tick % READING == 0:
            out = reading(film, at, exposure)
            out.update(
                running=scan.running,
                done=scan.done,
                frame=scan.frame,
                total=TOTAL,
                rate=RATE,
                exposure=exposure,
                crashes=scan.crashes,
                sound=sound.levels(tick, scan.running),
            )
            print(json.dumps(out, separators=(",", ":")), flush=True)
        tick += 1
        next_at += 1 / RATE
        time.sleep(max(next_at - time.monotonic(), 0))


if __name__ == "__main__":
    main()
