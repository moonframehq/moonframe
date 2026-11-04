"""Writes frames to a Moonframe ring in shared memory, for `ui.video`.

The layout is documented in crates/moonframe-frames/src/lib.rs.
"""

import struct
import time
from multiprocessing import shared_memory

MAGIC = b"MFRING1\0"
HEADER = 64
SLOT_HEADER = 64

RGBA8, RGBA16, RGBA16F, MONO16, BAYER16 = 1, 2, 3, 4, 5
RGGB, BGGR, GRBG, GBRG = 0, 1, 2, 3
BYTES_PER_PIXEL = {RGBA8: 4, RGBA16: 8, RGBA16F: 8, MONO16: 2, BAYER16: 2}


class Ring:
    """A ring of `slots` frames of at most `frame_bytes` bytes.

    The writer never waits for a reader: one that falls behind skips frames.
    Keep what you record out of this: it is a preview, and a slot is reused
    `slots` frames later.
    """

    def __init__(self, name, slots, frame_bytes):
        self.slots = slots
        self.slot_size = (SLOT_HEADER + frame_bytes + 7) // 8 * 8
        size = HEADER + slots * self.slot_size
        try:
            old = shared_memory.SharedMemory(name=name)
            old.close()
            old.unlink()
        except FileNotFoundError:
            pass
        self.memory = shared_memory.SharedMemory(name=name, create=True, size=size)
        self.view = self.memory.buf
        self.view[0:8] = MAGIC
        struct.pack_into("<I", self.view, 8, slots)
        struct.pack_into("<Q", self.view, 16, self.slot_size)
        struct.pack_into("<Q", self.view, 24, 0)
        self.number = 0

    def write(self, width, height, format, pixels, pattern=RGGB, captured=None):
        """Writes one frame; `pixels` is bytes, or anything that has them."""
        number = self.number + 1
        slot = HEADER + (number - 1) % self.slots * self.slot_size
        stride = width * BYTES_PER_PIXEL[format]
        struct.pack_into("<Q", self.view, slot, 0)
        struct.pack_into(
            "<IIIIQI",
            self.view,
            slot + 8,
            width,
            height,
            format,
            stride,
            captured if captured is not None else time.time_ns(),
            pattern,
        )
        start = slot + SLOT_HEADER
        self.view[start : start + len(pixels)] = pixels
        struct.pack_into("<Q", self.view, slot, number)
        struct.pack_into("<Q", self.view, 24, number)
        self.number = number

    def close(self):
        self.view = None
        self.memory.close()
        self.memory.unlink()
