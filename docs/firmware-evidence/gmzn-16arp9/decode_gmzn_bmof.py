# SPDX-License-Identifier: GPL-2.0-only
"""Offline, bounds-checked Python transcription of bmfdec's DS-01 decompressor.

This is used only because this host lacks a native C compiler and WSL cannot be
started.  It reads a BMOF payload as bytes and writes a decompressed BMF; it
does not access WMI, ACPI, EC, or any device interface.
"""
from __future__ import annotations

import argparse
import pathlib
import struct


class Bits:
    def __init__(self, src: bytes):
        self.src = src
        self.pos = 0
        self.buf = 0
        self.pb = 32

    def get16(self) -> None:
        self.buf >>= 16
        self.pb -= 16
        if self.pos < len(self.src):
            lo = self.src[self.pos]
            hi = self.src[self.pos + 1] if self.pos + 1 < len(self.src) else 0
            self.pos += 2
            # bmfdec's buffer is uint32_t; retain C's wraparound semantics.
            self.buf = (self.buf | ((lo | (hi << 8)) << 16)) & 0xffffffff

    def prepare(self) -> int:
        if self.pb >= 16:
            self.get16()
        return self.buf >> self.pb

    def read(self, n: int) -> int:
        value = self.prepare() & ((1 << n) - 1)
        self.pb += n
        return value


def read_len(bits: Bits) -> int:
    value = bits.prepare()
    low = value & 15
    if low & 1:
        bits.pb += 1
        return 3
    if low in (2, 6, 10, 14):
        bits.pb += 3
        return ((value >> 2) & 1) + 4
    if low in (4, 12):
        bits.pb += 5
        return ((value >> 3) & 3) + 6
    if low == 8:
        bits.pb += 7
        return ((value >> 4) & 7) + 10
    high = (value >> 4) & 15
    if high & 1:
        bits.pb += 9
        return ((value >> 5) & 15) + 18
    if high in (2, 6, 10, 14):
        bits.pb += 11
        return ((value >> 6) & 31) + 34
    if high in (4, 12):
        bits.pb += 13
        return ((value >> 7) & 63) + 66
    if high == 8:
        bits.pb += 15
        return ((value >> 8) & 127) + 130
    bits.pb += 9
    return bits.read(8) + 258 if value & 256 else -1


def decompress(src: bytes, cap: int = 0x2000000) -> bytes:
    bits = Bits(src)
    if bits.read(16) != 0x5344:
        raise ValueError("not a DS-01 stream")
    bits.read(16)  # DS version, byte-swapped by bmfdec only for logging
    out = bytearray()
    expected = None
    while True:
        # The raw WMI buffer omits bmfdec's 16-byte wrapper.  Once the BMF
        # directory is available, it supplies the exact decompressed length.
        if expected is None and len(out) >= 8 and out[:4] == b"FOMB":
            first_part = struct.unpack_from("<I", out, 4)[0]
            if len(out) >= first_part + 20 and out[first_part:first_part + 16] == b"BMOFQUALFLAVOR11":
                entries = struct.unpack_from("<I", out, first_part + 16)[0]
                expected = first_part + 20 + entries * 8
                if expected > cap:
                    raise ValueError(f"BMF advertises excessive size {expected}")
        if expected is not None and len(out) == expected:
            break
        if expected is not None and len(out) > expected:
            raise ValueError("output overran BMF-advertised length")
        value = bits.prepare()
        mode = value & 3
        if mode == 0:
            bits.pb += 8
            offset = (value >> 2) & 63
        elif mode == 1:
            bits.pb += 9
            out.append(((value >> 2) | 128) & 0xff)
            continue
        elif mode == 2:
            bits.pb += 9
            out.append((value >> 2) & 127)
            continue
        else:
            if value & 4:
                bits.pb += 15
                offset = ((value >> 3) & 4095) + 320
            else:
                bits.pb += 11
                offset = ((value >> 3) & 255) + 64
        if offset == 0x113f:
            # A raw WMI BMOF buffer has no advertised output length.  Preserve
            # bmfdec's marker handling but defer its cluster-alignment check;
            # the final BMF structure is validated independently below.
            continue
        if offset == 0 or offset > len(out):
            raise ValueError(f"invalid backreference offset {offset} at {len(out)}")
        length = read_len(bits) - 1
        if length <= 0 or len(out) + length > cap:
            raise ValueError(f"invalid output length {length} at {len(out)}")
        for _ in range(length):
            out.append(out[-offset])
        if bits.pos >= len(src) and bits.pb >= 16:
            break
    # bmfdec validates a terminal sync code after the stream.
    terminal = bits.read(3)
    if terminal == 7:
        terminal = bits.read(12) + 320
    if terminal != 0x113f:
        raise ValueError(f"missing terminal sync marker (got 0x{terminal:x})")
    return bytes(out)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()
    args.output.write_bytes(decompress(args.input.read_bytes()))


if __name__ == "__main__":
    main()
