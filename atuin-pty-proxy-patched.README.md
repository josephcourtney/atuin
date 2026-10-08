# Atuin patched PTY proxy

This downstream branch is based on the Atuin release recorded in `.patch-base.env`.
It retains two independent PTY enhancements:

- Propagate terminal pixel dimensions at startup and on resize.
- Sanitize Atuin-specific OSC 133 command-finished metadata before forwarding
  terminal output to Kitty. Original PTY bytes are still sent to Atuin's
  command capture parser, without modification.

The Kitty compatibility filter is automatically enabled when
`KITTY_WINDOW_ID` is present. Override with
`ATUIN_PTY_OSC133_COMPAT=auto|kitty|off` (default `auto`).
An invalid override falls back to `auto` with a warning.

Only recognized OSC 133 command-finished markers containing `history_id=`
are rewritten, to `OSC 133;D;<status>` with the original terminator.
Other complete markers pass through. Malformed, incomplete, or oversized
OSC 133 sequences are discarded to avoid forwarding incompatible metadata;
this can result in lost terminal control bytes. The filter limits an
in-progress sequence to 4096 bytes. Incomplete OSC-prefix candidates,
which are not recognized OSC 133 sequences, pass through on shutdown.

Normal Atuin commands continue to use Homebrew; only the PTY proxy uses
the patched executable. Run `just help` for upgrade/build/install instructions.

For future upstream release rebases, preserve both the pixel-size propagation
and OSC 133 compatibility logic. The `justfile` imports shared local recipes
from `~/.config/just/lib/`, so those recipes are required for `just` commands.

## Capture diagnostics

The patched proxy supports an opt-in, metadata-only trace. Set
`ATUIN_PTY_CAPTURE_DIAGNOSTICS` to an **absolute file path** before starting
a *new* Kitty window / PTY proxy; `0` or unset disables diagnostics.

Example:

```sh
ATUIN_PTY_CAPTURE_DIAGNOSTICS="$HOME/atuin-pty-capture.log" kitty
```

The file is opened with restrictive permissions (0600). The trace reports
proxy startup, whether a command-capture sink is configured, Kitty compatibility
mode, PTY read byte counts, parser queue failures, OSC 133 event kinds,
semantic zone changes, capture resets, missing/invalid history-ID markers,
completed capture byte counts, and sink invocation/return. It **never writes
captured terminal content, OSC parameter values, commands, or history IDs**.
The log does contain process identifiers, event timing, and command-output
lengths; review it before sharing.

Interpretation:

- `proxy_start capture_enabled=false`: the proxy was launched without capture
  configuration; check Atuin CLI setup before inspecting shell markers.
- No `osc133_marker` despite `pty_read`: shell integration is not emitting
  recognized markers, or the parser is not recognizing them.
- `capture_pending`: a finish marker lacked a parseable history ID.
- `capture_ready` and `sink_return`: the proxy submitted the capture to its
  configured sink. **This does not prove daemon acceptance or persistence.**
  If output is still unavailable, instrument the sink/daemon separately.

Do not enable diagnostics indefinitely: the logger appends a record for
each PTY read and can generate significant disk usage.
