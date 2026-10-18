#!/usr/bin/env python3
"""Read-only diagnostics for this Atuin downstream fork (macOS)."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

HOME = Path.home()
PATCHED = HOME / ".local/bin/atuin-patched"
HOMEBREW = Path("/opt/homebrew/bin/atuin")

def run(*args, timeout=8):
    try:
        result = subprocess.run(args, text=True, capture_output=True, timeout=timeout, check=False)
        return {"ok": result.returncode == 0, "code": result.returncode,
                "stdout": result.stdout.strip(), "stderr": result.stderr.strip()}
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {"ok": False, "error": str(exc)}

def version(path):
    return run(str(path), "--version") if Path(path).exists() else {"ok": False, "error": "executable absent"}

def release(info):
    raw = info.get("stdout", "")
    return raw.removeprefix("atuin ").split(" ", 1)[0] if info.get("ok") else None

def diagnose():
    brew, patched = version(HOMEBREW), version(PATCHED)
    active = run(str(HOMEBREW), "__internal", "pty-proxy-active")
    daemon = run(str(HOMEBREW), "daemon", "status")
    config = run(str(HOMEBREW), "config", "print")
    installed = run("uv", "tool", "list") if shutil.which("uv") else {"ok": False, "error": "uv unavailable"}
    proxy_processes = run("pgrep", "-fl", "atuin.*pty-proxy")
    data = {
        "homebrew": {"path": str(HOMEBREW), **brew},
        "patched": {"path": str(PATCHED), "resolved": str(PATCHED.resolve()) if PATCHED.exists() else None, **patched},
        "same_release": release(brew) == release(patched) if release(brew) and release(patched) else None,
        "shell": {"ATUIN_SESSION": os.environ.get("ATUIN_SESSION"), "ATUIN_PTY_PROXY_ACTIVE": os.environ.get("ATUIN_PTY_PROXY_ACTIVE"),
                  "ATUIN_PROXY_BIN": os.environ.get("ATUIN_PROXY_BIN")},
        "active_proxy_rpc": active, "proxy_processes": proxy_processes,
        "daemon": daemon, "config": config,
        "uv_tools": installed,
    }
    proxy_lines = [line for line in proxy_processes.get("stdout", "").splitlines() if "pty-proxy" in line]
    patched_lines = [line for line in proxy_lines if "atuin-patched" in line]
    upstream_lines = [line for line in proxy_lines if "atuin-patched" not in line]
    data["proxy_identity"] = {
        "classification": ("mixed" if patched_lines and upstream_lines else
                           "patched" if patched_lines else
                           "upstream" if upstream_lines else "unknown"),
        "patched_count": len(patched_lines),
        "upstream_count": len(upstream_lines),
        "scope": "host-wide processes; not necessarily this terminal",
    }
    problems = []
    if not brew.get("ok"): problems.append("Homebrew Atuin cannot run")
    if not patched.get("ok"): problems.append("patched Atuin cannot run")
    if data["same_release"] is False: problems.append("Homebrew and patched releases differ")
    if not daemon.get("ok"): problems.append("daemon status unsuccessful")
    data["problems"] = problems
    data["capture_state"] = "not tested: run just test-capture"
    data["handshake_state"] = "not tested: run just test-capture with installed Copout"
    return data

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--json", action="store_true")
    p.add_argument("--strict", action="store_true", help="fail when known prerequisite errors are present")
    a = p.parse_args()
    d = diagnose()
    if a.json:
        print(json.dumps(d, indent=2))
    else:
        for key in ("homebrew", "patched"):
            print(f"{key}: {d[key].get('stdout') or d[key].get('error')} ({d[key]['path']})")
        print(f"release match: {d['same_release']}")
        print(f"daemon: {d['daemon'].get('stdout') or d['daemon'].get('stderr') or d['daemon'].get('error')}")
        print(f"proxy active check: {d['active_proxy_rpc'].get('stdout') or 'unavailable'}")
        print(f"proxy processes: {d['proxy_processes'].get('stdout') or 'none detected'}")
        print(f"proxy identity: {d['proxy_identity']['classification']} (host-wide)")
        print(f"capture: {d['capture_state']}")
        for issue in d["problems"]:
            print(f"FAIL: {issue}")
        if not d["problems"]: print("No known binary/daemon prerequisites failed; proxy identity and capture still need qualification.")
    return 1 if a.strict and d["problems"] else 0

if __name__ == "__main__":
    sys.exit(main())
