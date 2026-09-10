# Local Atuin PTY proxy patch maintenance
#
# Examples:
#   just update 18.21.0 18.22.0
#   just build
#   just install
#   just verify
#   just rebuild
#
# `update` intentionally stops if git rebase encounters a conflict. Resolve it,
# `git add` the files, and run `git rebase --continue`.

branch := "fix-pty-pixel-size"
patched_bin := env_var("HOME") / ".local/bin/atuin-patched"
atuin_bin := "/opt/homebrew/bin/atuin"
toolchain := "1.98.0"
base_file := ".patch-base.env"

default:
    @just help

help:
    #!/usr/bin/env bash
    cat <<'EOF'
    Atuin patched PTY proxy
    =======================

    This fork patches Atuin's PTY proxy to propagate terminal pixel dimensions.
    Normal Atuin commands use Homebrew; only the PTY proxy uses the patched binary.

    Typical upgrade
    ---------------

      brew upgrade atuin
      just upgrade NEW

    Example:

      just upgrade 18.22.0

    Then open a new terminal and verify:

      just proxy-status
      just window-size

    Resize the terminal and run `just window-size` again. Pixel dimensions
    should be nonzero and should change with the window.


    Other common operations
    -----------------------

      just rebuild
          Rebuild, reinstall, and verify without changing the upstream version.

      just update NEW
          Rebase the patch from its recorded base onto a new Atuin release.

      just patch
          Show exactly what this fork changes relative to its recorded upstream base.

      just base
          Show the recorded upstream release and commit.

      just build
          Check and build without installing.

      just install
          Build and install ~/.local/bin/atuin-patched.

      just verify
          Confirm patched and Homebrew Atuin have matching release versions.


    If an update conflicts
    ----------------------

    Resolve the conflict while preserving the pixel-size patch:

      * use terminal::window_size()
      * propagate rows/columns
      * propagate width/height into pixel_width/pixel_height
      * preserve this for both initial PTY creation and SIGWINCH resize

    Then:

      git add <files>
      git rebase --continue
      just patch NEW
      just rebuild

    To abandon the update:

      git rebase --abort


    If verification fails
    ---------------------

      /opt/homebrew/bin/atuin --version
      ~/.local/bin/atuin-patched --version

    Their release numbers must match. If Homebrew is newer, run `just upgrade`.
    If the branch is already current, run `just rebuild`.
    EOF




# Rebase the patch from its recorded base onto a new Atuin release.
update new_release: preflight
    #!/usr/bin/env bash
    set -euo pipefail

    source "{{ base_file }}"
    new_ref="v{{ new_release }}"

    git fetch origin --tags
    git rev-parse --verify "$new_ref^{commit}" >/dev/null

    new_commit="$(git rev-parse "$new_ref^{commit}")"

    if [[ "$new_commit" == "$PATCH_BASE_COMMIT" ]]; then
        echo >&2 "error: patch is already based on $new_ref"
        exit 1
    fi

    if ! git merge-base --is-ancestor "$PATCH_BASE_COMMIT" "$new_commit"; then
        echo >&2 "error: $new_ref is not a descendant of $PATCH_BASE_REF"
        exit 1
    fi

    common_base="$(git merge-base "{{ branch }}" "$new_ref")"
    if [[ "$common_base" != "$PATCH_BASE_COMMIT" ]]; then
        echo >&2 "error: patch branch contains upstream history beyond its recorded base"
        echo >&2 "  recorded:   $PATCH_BASE_COMMIT"
        echo >&2 "  merge-base: $common_base"
        echo >&2 "Do not git pull on {{ branch }}; use git fetch and just update."
        exit 1
    fi

    echo "Rebasing $PATCH_BASE_REF -> $new_ref"
    git rebase --onto "$new_ref" "$PATCH_BASE_COMMIT" "{{ branch }}"

    printf 'PATCH_BASE_REF=%s\nPATCH_BASE_COMMIT=%s\n' \
        "$new_ref" "$new_commit" > "{{ base_file }}"

    git add "{{ base_file }}"
    git commit -m "update patch base to $new_ref"

    git diff --check "$new_ref..HEAD"
    git diff "$new_ref..HEAD" -- crates/atuin-pty-proxy/src/runtime.rs


# Validate that the checkout is safe and internally consistent for an update.
preflight:
    #!/usr/bin/env bash
    set -euo pipefail

    # Updating must never implicitly include unrelated local changes.
    if [[ -n "$(git status --porcelain)" ]]; then
        echo >&2 "error: working tree is not clean"
        git status --short >&2
        exit 1
    fi

    if [[ ! -f "{{ base_file }}" ]]; then
        echo >&2 "error: {{ base_file }} does not exist"
        exit 1
    fi

    source "{{ base_file }}"

    if [[ -z "${PATCH_BASE_REF:-}" || -z "${PATCH_BASE_COMMIT:-}" ]]; then
        echo >&2 "error: {{ base_file }} is incomplete"
        exit 1
    fi

    # Both recorded objects must exist.
    git rev-parse --verify "$PATCH_BASE_REF^{commit}" >/dev/null
    git rev-parse --verify "$PATCH_BASE_COMMIT^{commit}" >/dev/null

    # The human-readable ref must still identify the recorded immutable commit.
    actual="$(git rev-parse "$PATCH_BASE_REF^{commit}")"
    if [[ "$actual" != "$PATCH_BASE_COMMIT" ]]; then
        echo >&2 "error: recorded patch base is inconsistent"
        echo >&2 "  ref:      $PATCH_BASE_REF"
        echo >&2 "  recorded: $PATCH_BASE_COMMIT"
        echo >&2 "  resolved: $actual"
        exit 1
    fi

    # The recorded base should actually be an ancestor of the patch branch.
    if ! git merge-base --is-ancestor "$PATCH_BASE_COMMIT" "{{ branch }}"; then
        echo >&2 "error: recorded base is not an ancestor of {{ branch }}"
        exit 1
    fi

    printf 'patch base: %s (%s)\n' \
        "$PATCH_BASE_REF" "${PATCH_BASE_COMMIT:0:12}"

# Format and check the patched PTY proxy.
check:
    cargo +nightly fmt --all -- --check
    git diff --check
    cargo +{{ toolchain }} check -p atuin-pty-proxy

# Build the complete Atuin binary containing the patched proxy.
build: check
    cargo +{{ toolchain }} build --release -p atuin

# Install the freshly built binary.
install: build
    mkdir -p "$(dirname '{{ patched_bin }}')"
    install -m 755 target/release/atuin "{{ patched_bin }}"

# Compare the release versions of Homebrew and patched Atuin.
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

    [[ "$homebrew_release" == "$patched_release" ]] || {
        echo >&2 "error: Atuin release mismatch"
        exit 1
    }

# Rebuild/reinstall the patch without changing its upstream base.
rebuild: install verify

# Update the branch to a new release, build it, install it, and verify it.
upgrade new_release:
    just update "{{ new_release }}"
    just rebuild

# Show the patch relative to its recorded upstream base.
patch:
    #!/usr/bin/env bash
    set -euo pipefail
    source "{{ base_file }}"
    git diff --check "$PATCH_BASE_COMMIT..HEAD"
    git diff "$PATCH_BASE_COMMIT..HEAD"

# Show the currently recorded upstream base.
base:
    #!/usr/bin/env bash
    set -euo pipefail
    source "{{ base_file }}"
    printf 'ref:    %s\ncommit: %s\n' "$PATCH_BASE_REF" "$PATCH_BASE_COMMIT"

# Test the current terminal's TIOCGWINSZ, including pixel dimensions.
window-size:
    #!/usr/bin/env python3
    import array
    import fcntl
    import sys
    import termios

    ws = array.array("H", [0, 0, 0, 0])
    fcntl.ioctl(sys.stdout.fileno(), termios.TIOCGWINSZ, ws, True)
    print(f"rows={ws[0]} cols={ws[1]} pixels={ws[2]}x{ws[3]}")

# Confirm that an Atuin PTY proxy is running.
proxy-status:
    @ps -ax -o pid,command | grep '[a]tuin.*pty-proxy' || \
        { echo >&2 "No Atuin PTY proxy found"; exit 1; }
