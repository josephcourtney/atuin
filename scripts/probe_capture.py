#!/usr/bin/env python3
"""Live shell-to-Copout captured-output qualification. Creates isolated shell session."""
import argparse
import os
import pty
import select
import shutil
import signal
import sys
import time
import uuid
import xml.etree.ElementTree as ET

def run_probe(timeout, shell):
    marker = "atuin-capture-qualification-" + uuid.uuid4().hex
    if not shutil.which("copout"):
        raise RuntimeError("copout not on PATH; install the updated pinned version first")
    pid, fd = pty.fork()
    if pid == 0:
        env = os.environ.copy()
        for key in ("ATUIN_SESSION", "ATUIN_SHLVL", "ATUIN_PTY_PROXY_ACTIVE", "ATUIN_PROXY_BIN"):
            env.pop(key, None)
        os.execve(shell, [shell, "-il"], env)
    buffer = b""
    deadline = time.monotonic() + timeout
    try:
        # Wait for shell startup rather than racing commands against zsh initialization.
        time.sleep(1)
        os.write(fd, ("printf '%s\\n' '" + marker + "'\n").encode())
        time.sleep(1)
        os.write(fd, b"copout 1 --print\n")
        time.sleep(1)
        os.write(fd, b"printf '__ATUIN_QUALIFICATION_END__\\n'\n")
        while time.monotonic() < deadline:
            ready, _, _ = select.select([fd], [], [], min(0.5, max(0.01, deadline - time.monotonic())))
            if ready:
                try:
                    part = os.read(fd, 65536)
                except OSError:
                    break
                if not part: break
                buffer += part
                if b"__ATUIN_QUALIFICATION_END__" in buffer and b"</copout>" in buffer:
                    break
        output = buffer.decode(errors="replace")
        start = output.find("<copout ")
        end = output.find("</copout>", start)
        if start < 0 or end < 0:
            raise RuntimeError("No Copout XML received from fresh login shell; inspect startup, selection, or gRPC. Tail:\n" + output[-1600:])
        xml = output[start:end+len("</copout>")]
        root = ET.fromstring(xml)
        runs = root.findall("run")
        if len(runs) != 1: raise RuntimeError(f"Expected one selected run; found {len(runs)}")
        command = runs[0].findtext("command") or ""
        captured = runs[0].findtext("output") or ""
        if marker not in command: raise RuntimeError("Copout selected a different command: " + command)
        if marker not in captured: raise RuntimeError("New command output was not persisted/retrieved: " + repr(captured))
        print("PASS: fresh interactive shell history, output persistence and Copout retrieval")
        print("history_id: " + str(runs[0].get("history_id")))
        print("NOTE: this uses the installed Copout and the active Atuin daemon; a test history record is created.")
        return 0
    finally:
        try: os.kill(pid, signal.SIGHUP)
        except ProcessLookupError: pass
        try: os.close(fd)
        except OSError: pass
        try: os.waitpid(pid, os.WNOHANG)
        except ChildProcessError: pass

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--timeout", type=float, default=30)
    p.add_argument("--shell", default=shutil.which("zsh") or "/bin/zsh")
    args = p.parse_args()
    try:
        return run_probe(args.timeout, args.shell)
    except Exception as exc:
        print("FAIL: " + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    sys.exit(main())
