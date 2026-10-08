# Local Atuin PTY proxy patch maintenance

import '~/.config/just/lib/downstream-patch.just'
import '~/.config/just/lib/versioned-install.just'

# Generic downstream-patch configuration.
patch_branch := "patched-pty-proxy"
patch_base_file := ".patch-base.env"
patch_upstream_remote := "upstream"
patch_release_prefix := "v"

# Atuin-specific configuration.
patched_bin := env_var("HOME") / ".local/bin/atuin-patched"
atuin_bin := "/opt/homebrew/bin/atuin"
toolchain := "1.98.0"

# Primary upstream file for PTY integration; the filter lives in kitty_osc133.rs.
patch_target := "crates/atuin-pty-proxy/src/runtime.rs"

default:
    @just help

help:
    #!/usr/bin/env bash
    cat <<'EOF_HELP'
    Atuin patched PTY proxy
    =======================

    This fork propagates PTY pixel dimensions and sanitizes Kitty OSC 133 markers.\n    Set ATUIN_PTY_OSC133_COMPAT=auto|kitty|off (default auto).
    Generic downstream-patch state is stored in .patch-base.env.

    Typical upgrade
    ---------------
      brew upgrade atuin
      just upgrade NEW          # e.g. just upgrade 18.23.0

    Then open a new terminal and run:
      just proxy-status
      just window-size          # resize, then run again

    Common operations
    -----------------
      just preflight            Validate clean worktree and recorded patch base
      just base                 Show recorded upstream ref and commit
      just patch                Show changes relative to the recorded base
      just update NEW           Rebase patch onto a new release, no rebuild
      just rebuild              Check, test, build, install, and verify
      just proxy-list           List all running patched proxy versions

    If rebase conflicts
    -------------------
      # preserve terminal pixel dimensions and Kitty OSC 133 compatibility
      git add <resolved-files>
      git rebase --continue
      just finish-update NEW
      just patch
      just rebuild

    To abandon: git rebase --abort
    Do not git pull on the patch branch; use git fetch / just update NEW.
    EOF_HELP

# Project-facing wrappers around the reusable downstream-patch machinery.
preflight: patch-preflight

base: patch-base

# Show the patch relative to its recorded upstream base.
patch:
    #!/usr/bin/env bash
    set -euo pipefail
    source "{{ patch_base_file }}"
    git diff --check "$PATCH_BASE_COMMIT..HEAD"
    git diff "$PATCH_BASE_COMMIT..HEAD"

# Rebase the patch and then show the Atuin source file that should carry the
# semantic downstream change.
update new_release:
    just patch-update "{{ new_release }}"
    just patch-target

# Complete metadata bookkeeping after manually resolving a rebase conflict.
finish-update new_release:
    just patch-finish "{{ new_release }}"
    just patch-target

# Show the primary patched runtime source; filter implementation lives in kitty_osc133.rs.
patch-target:
    #!/usr/bin/env bash
    set -euo pipefail
    source "{{ patch_base_file }}"
    git diff "$PATCH_BASE_COMMIT..HEAD" -- "{{ patch_target }}"

# Format and check the patched PTY proxy.
check:
    cargo +nightly fmt --all -- --check
    git diff --check
    cargo +{{ toolchain }} check -p atuin-pty-proxy
    cargo +{{ toolchain }} test -p atuin-pty-proxy

# Build the complete Atuin executable containing the patched proxy.
build: check
    cargo +{{ toolchain }} build --release -p atuin

# Install a versioned patched executable and update the stable symlink.
install: build
    #!/usr/bin/env bash
    set -euo pipefail

    version="$(target/release/atuin --version)"
    version="${version#atuin }"
    version="${version%% *}"

    just versioned-install target/release/atuin "{{ patched_bin }}" "$version"

# Compare the Homebrew and currently installed patched release versions.
verify:
    #!/usr/bin/env bash
    set -euo pipefail

    homebrew_version="$("{{ atuin_bin }}" --version)"
    patched_version="$("{{ patched_bin }}" --version)"

    printf 'Homebrew: %s\n' "$homebrew_version"
    printf 'Patched:  %s\n' "$patched_version"

    homebrew_release="${homebrew_version#atuin }"
    homebrew_release="${homebrew_release%% *}"
    patched_release="${patched_version#atuin }"
    patched_release="${patched_release%% *}"

    if [[ "$homebrew_release" != "$patched_release" ]]; then
        echo >&2 "error: Atuin release mismatch"
        exit 1
    fi

# Rebuild/reinstall the patch without changing its upstream base.
rebuild: install verify

# Update the branch to a new release, build it, install it, and verify it.
upgrade new_release:
    just update "{{ new_release }}"
    just rebuild

# Read the current PTY dimensions, including pixel dimensions.
window-size:
    #!/usr/bin/env python3
    import array
    import fcntl
    import sys
    import termios

    ws = array.array("H", [0, 0, 0, 0])
    fcntl.ioctl(sys.stdout.fileno(), termios.TIOCGWINSZ, ws, True)
    print(f"rows={ws[0]} cols={ws[1]} pixels={ws[2]}x{ws[3]}")

# List all patched Atuin PTY proxies and their child shells.
proxy-list:
    #!/usr/bin/env bash
    set -euo pipefail

    pids="$(pgrep -f '/atuin-patched(-[^ ]+)? pty-proxy' || true)"

    if [[ -z "$pids" ]]; then
        echo "No patched Atuin PTY proxies running."
        exit 0
    fi

    printf '%-7s %-9s %-12s %-10s %-7s %-9s %s\n' \
        PROXY TTY ELAPSED VERSION CHILD CHILD-TTY CHILD-COMMAND

    while read -r pid; do
        command="$(ps -o command= -p "$pid" | xargs)"
        proxy_tty="$(ps -o tty= -p "$pid" | xargs)"
        elapsed="$(ps -o etime= -p "$pid" | xargs)"
        exe="${command%% *}"
        name="${exe##*/}"

        if [[ "$name" == atuin-patched-* ]]; then
            version="${name#atuin-patched-}"
        else
            version="unknown"
        fi

        child="$(pgrep -P "$pid" | head -1 || true)"
        if [[ -n "$child" ]]; then
            child_tty="$(ps -o tty= -p "$child" | xargs)"
            child_cmd="$(ps -o command= -p "$child" | xargs)"
        else
            child="-"
            child_tty="-"
            child_cmd="<no child>"
        fi

        printf '%-7s %-9s %-12s %-10s %-7s %-9s %s\n' \
            "$pid" "$proxy_tty" "$elapsed" "$version" \
            "$child" "$child_tty" "$child_cmd"
    done <<< "$pids"

# Confirm that this terminal is actually running under a patched proxy.
proxy-status:
    #!/usr/bin/env bash
    set -euo pipefail

    pid=$PPID
    while [[ "$pid" -gt 1 ]]; do
        command="$(ps -o command= -p "$pid")"
        exe="${command%% *}"
        name="${exe##*/}"

        if [[ ("$name" == atuin-patched || "$name" == atuin-patched-*) \
              && "$command" == *" pty-proxy"* ]]; then
            if [[ "$name" == atuin-patched-* ]]; then
                version="${name#atuin-patched-}"
            else
                version="unknown"
            fi
            printf 'active:  yes\n'
            printf 'pid:     %s\n' "$pid"
            printf 'version: %s\n' "$version"
            printf 'command: %s\n' "$command"
            exit 0
        fi

        pid="$(ps -o ppid= -p "$pid" | tr -d ' ')"
    done

    echo "active: no"
    exit 1
