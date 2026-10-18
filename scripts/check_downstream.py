#!/usr/bin/env python3
"""Inspect pinned and installed downstream consumers without modifying them."""
import argparse
import importlib.metadata
import json
from pathlib import Path
import shutil
import subprocess
import sys

HOME = Path.home()

def command(*args):
    try:
        p = subprocess.run(args, capture_output=True, text=True, timeout=15, check=False)
        return {"ok": p.returncode == 0, "stdout": p.stdout.strip(), "stderr": p.stderr.strip()}
    except (OSError, subprocess.TimeoutExpired) as e:
        return {"ok": False, "error": str(e)}

def dist_info(python, package):
    script = ("import importlib.metadata as m, json; "
              "d=m.distribution(" + repr(package) + "); "
              "f=d.read_text('direct_url.json'); "
              "print(json.dumps({'version':d.version,'source':json.loads(f) if f else None}))")
    result = command(str(python), "-c", script)
    if result.get("ok"):
        try:
            return json.loads(result["stdout"])
        except json.JSONDecodeError:
            pass
    return {"error": result.get("stderr") or result.get("error") or "unknown"}

def inspect():
    python = HOME / ".local/share/uv/tools/copout/bin/python"
    result = {"copout_python": str(python), "installed": python.exists()}
    if python.exists():
        result["copout"] = dist_info(python, "copout")
        result["jerakeen"] = dist_info(python, "jerakeen")
    for name in ("copout", "catherd", "raedboc-core"):
        binary = shutil.which(name)
        if binary: result[name + "_executable"] = binary
    result["note"] = "Compare revisions to qualified Git pins; presence alone is not protocol qualification."
    return result

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    a = parser.parse_args()
    state = inspect()
    if a.json: print(json.dumps(state, indent=2))
    else:
        print("Copout installed environment:", state["copout_python"])
        for name in ("copout", "jerakeen"):
            print(name + ":", json.dumps(state.get(name, {"state": "not installed"})))
        print(state["note"])
    return 0

if __name__ == "__main__":
    sys.exit(main())
