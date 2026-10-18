#!/usr/bin/env python3
"""Atomically activate a versioned candidate and restore the prior symlink."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

DEST = Path.home() / ".local/bin/atuin-patched"
RECORD = Path.home() / ".local/state/atuin-patch-install.json"

def activate(candidate):
    candidate = Path(candidate).expanduser().resolve(strict=True)
    if not candidate.is_file() or not os.access(candidate, os.X_OK):
        raise ValueError(f"not an executable: {candidate}")
    subprocess.run([str(candidate), "--version"], check=True, timeout=10)
    if DEST.exists() and not DEST.is_symlink():
        raise ValueError(f"refusing to replace non-symlink {DEST}")
    previous = str(DEST.resolve()) if DEST.is_symlink() else None
    DEST.parent.mkdir(parents=True, exist_ok=True)
    RECORD.parent.mkdir(parents=True, exist_ok=True)
    # Record previous installation before changing the active link.
    REC0 = {"previous": previous, "candidate": str(candidate)}
    with tempfile.NamedTemporaryFile(mode="w", dir=RECORD.parent, delete=False) as f:
        json.dump(REC0, f); tmp_record = Path(f.name)
    os.replace(tmp_record, RECORD)
    temp_link = DEST.parent / (".atuin-patched-" + str(os.getpid()))
    try:
        temp_link.symlink_to(candidate)
        os.replace(temp_link, DEST)
    finally:
        temp_link.unlink(missing_ok=True)
    print(f"activated {candidate}")
    print("Existing proxy and daemon processes are NOT restarted; run just doctor and just test-capture.")

def rollback():
    if not RECORD.exists(): raise ValueError("no previous installation recorded")
    previous = json.loads(RECORD.read_text()).get("previous")
    if not previous: raise ValueError("previous symlink target unavailable")
    if not Path(previous).is_file(): raise ValueError("previous installation no longer exists")
    activate(previous)
    print("Previous binary restored; running processes still require explicit restart.")

def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="action", required=True)
    a = sub.add_parser("activate"); a.add_argument("candidate")
    sub.add_parser("rollback")
    args = parser.parse_args()
    try:
        if args.action == "activate": activate(args.candidate)
        else: rollback()
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"error: {exc}", file=sys.stderr); return 1

if __name__ == "__main__":
    sys.exit(main())
