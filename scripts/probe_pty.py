#!/usr/bin/env python3
"""Test PTY row/column/pixel propagation without changing the user's terminal."""
import array
import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time

def winsize(fd):
    return struct.unpack("HHHH", fcntl.ioctl(fd, termios.TIOCGWINSZ, bytes(8)))

def set_size(fd, size):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", *size))

def main():
    master, slave = pty.openpty()
    try:
        initial = (31, 97, 720, 1280)
        changed = (42, 110, 900, 1550)
        set_size(slave, initial)
        observed = winsize(slave)
        if observed != initial:
            raise RuntimeError(f"PTY dimensions mismatch: {observed} != {initial}")
        set_size(slave, changed)
        observed = winsize(slave)
        if observed != changed:
            raise RuntimeError(f"PTY resize mismatch: {observed} != {changed}")
        print("PASS: local PTY supports rows, columns and pixel dimensions")
        print("NOTE: this tests the PTY ioctl, NOT propagation through the patched proxy")
        return 0
    finally:
        os.close(master)
        os.close(slave)

if __name__ == "__main__":
    sys.exit(main())
