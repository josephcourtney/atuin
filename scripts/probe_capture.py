#!/usr/bin/env python3
"""Qualify new command capture through real zsh, Atuin, and installed Copout."""
import argparse
import errno
import json
import os
from pathlib import Path
import pty
import re
import select
import shlex
import shutil
import signal
import sys
import tempfile
import time
import uuid

SESSION_PATTERN = re.compile(rb"__ATUIN_SESSION_BEGIN__([0-9a-f]+)__ATUIN_SESSION_END__")

def send(fd, command):
    os.write(fd, command.encode() + b"\n")

def read_until(fd, marker, timeout):
    deadline = time.monotonic() + timeout
    buf = bytearray()
    while marker not in buf:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError(f"Timeout waiting for {marker!r}; terminal tail: {bytes(buf[-1200:])!r}")
        if not select.select([fd], [], [], remaining)[0]:
            continue
        try:
            chunk = os.read(fd, 4096)
        except OSError as exc:
            if exc.errno == errno.EIO:
                break
            raise
        if not chunk:
            break
        buf.extend(chunk)
    if marker not in buf:
        raise RuntimeError(f"Shell exited before {marker!r}; terminal tail: {bytes(buf[-1200:])!r}")
    return bytes(buf)

def split_marker(marker):
    midpoint = len(marker) // 2
    return f"'{marker[:midpoint]}''{marker[midpoint:]}'"

def probe(timeout, shell, copout):
    marker = "ATUIN_CAPTURE_PROBE_" + uuid.uuid4().hex
    command = f"printf '%s\\n' {split_marker(marker)}"
    parent_session = os.environ.get("ATUIN_SESSION")
    with tempfile.TemporaryDirectory(prefix="atuin-qualification-") as tmp:
        target = Path(tmp) / "copout.json"
        pid, fd = pty.fork()
        if pid == 0:
            for key in ("ATUIN_SESSION", "ATUIN_SHLVL", "ATUIN_HISTORY_ID",
                        "ATUIN_PTY_PROXY_ACTIVE", "ATUIN_PTY_PROXY_SOCKET"):
                os.environ.pop(key, None)
            os.execv(shell, [shell, "-il"])
        try:
            send(fd, "printf '%s\\n' '__ATUIN_''READY__'")
            read_until(fd, b"__ATUIN_READY__", timeout)
            send(fd, "print -r -- '__ATUIN_SESSION_''BEGIN__'\"$ATUIN_SESSION\"'__ATUIN_SESSION_''END__'")
            identity = read_until(fd, b"__ATUIN_SESSION_END__", timeout)
            match = SESSION_PATTERN.search(identity)
            if not match:
                raise RuntimeError("Child Atuin session could not be established")
            session = match.group(1).decode("ascii")
            if session == parent_session:
                raise RuntimeError("Child shell reused parent Atuin session")
            send(fd, command)
            observed = read_until(fd, marker.encode(), timeout)
            if marker.encode() not in observed:
                raise RuntimeError("stdout marker was not observed in the child PTY")
            # A new command cannot run until the shell's previous post-command
            # hook has had an opportunity to finalize the capture.
            send(fd, "printf '%s\\n' '__ATUIN_PROBE_''DONE__'")
            read_until(fd, b"__ATUIN_PROBE_DONE__", timeout)
            query = (
                f"{shlex.quote(copout)} --print --json --last 50 "
                f"> {shlex.quote(str(target))}; "
                "printf '%s\\n' '__ATUIN_QUERY_''DONE__'"
            )
            send(fd, query)
            read_until(fd, b"__ATUIN_QUERY_DONE__", timeout)
            if not target.exists():
                raise RuntimeError("Copout query did not write JSON")
            payload = json.loads(target.read_text())
            observed_session = payload.get("environment", {}).get("session_id")
            if observed_session != session:
                raise RuntimeError(f"Copout used session {observed_session!r}, expected {session!r}")
            matching = [x for x in payload.get("runs", []) if x.get("command", "").strip() == command]
            if not matching:
                raise RuntimeError("Probe command absent from Atuin history; inspect session and hooks")
            result = matching[-1]
            if result.get("result", {}).get("status") != 0:
                raise RuntimeError(f"Probe exit status incorrect: {result.get('result')!r}")
            captured = result.get("output", {})
            if captured.get("state") != "captured":
                raise RuntimeError("Captured output unavailable: " + str(captured.get("error") or captured.get("state")))
            body = captured.get("text", "")
            if marker not in body:
                raise RuntimeError("Marker reached child PTY but was absent from stored output: " + repr(body[:200]))
            print("PASS: fresh interactive zsh command recorded, captured, and retrieved")
            print("session_id:", session)
            print("history_id:", result.get("history_id"))
            print("NOTE: tested installed Copout against running daemon; a unique history record was created.")
            return 0
        finally:
            try:
                send(fd, "exit")
            except OSError:
                pass
            try:
                os.kill(pid, signal.SIGHUP)
            except ProcessLookupError:
                pass
            os.close(fd)
            try:
                os.waitpid(pid, os.WNOHANG)
            except ChildProcessError:
                pass

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--shell", default=shutil.which("zsh") or "/bin/zsh")
    parser.add_argument("--copout", default=shutil.which("copout"))
    args = parser.parse_args()
    if not args.copout:
        print("FAIL: copout executable not on PATH", file=sys.stderr)
        return 1
    try:
        return probe(args.timeout, args.shell, args.copout)
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    sys.exit(main())
