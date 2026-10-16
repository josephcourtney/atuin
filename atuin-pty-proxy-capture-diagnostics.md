# Atuin 18.24 PTY capture diagnostics

This is an **opt-in, metadata-only** diagnostic extension for the Atuin 18.24
PTY proxy. It operates on Atuin's private `OSC 18188735;C/D;<history-id>`
markers, **not** the pre-18.24 OSC 133 capture protocol.

## Enable

Set `ATUIN_PTY_CAPTURE_DIAGNOSTICS` to an **absolute path** in the environment
of the PTY **proxy process**, before that process starts. Setting it in a shell
that already runs under a proxy does not reconfigure that proxy. For example,
from an unattached interactive terminal:

```sh
export ATUIN_PTY_CAPTURE_DIAGNOSTICS="$HOME/atuin-pty-capture.log"
atuin-patched pty-proxy --shell /opt/homebrew/bin/zsh
```

Run `atuin __internal pty-proxy-active` inside the new shell: it should return
`1`. The shell's `ATUIN_PTY_PROXY_ACTIVE=2` alone does **not** prove attachment;
terminal windows launched from a proxied shell may inherit stale variables.

Unset or `0` disables logging. The logger rejects relative paths and opens its
file with mode 0600. It never prints events to the terminal. Do not leave it
enabled indefinitely: each PTY read produces a trace event and the file is
append-only.

## Events

- `proxy_start`: protocol number and whether the proxy has a capture sink
- `parser_start` / `parser_stop`: screen parser lifecycle
- `pty_read`: PTY read byte count; `parser_queue_error`: parser receiver disconnected
- `parser_resize`: terminal row/column count
- `marker`: accepted `output_start` or `output_end` event
- `marker_rejected`: malformed Atuin-private marker, unknown kind, or invalid
  history ID (neither the invalid value nor marker bytes are logged)
- `capture_start`, `capture_abandoned`, `capture_ignored`,
  `capture_discarded`: capture lifecycle and invalid attribution
- `capture_ready`: total observed and retained output byte counts
- `sink_invoke` / `sink_return`: handoff to the configured capture sink

These events contain **no terminal output bytes, commands, history IDs, or raw
OSC parameter values**. They *do* expose timing, PIDs, sizes and event counts,
which should be reviewed before sharing. `sink_return` shows only that the
configured sink callback returned, **not** that the Atuin daemon persisted the
capture; persistence must be verified with `GetCommandOutput`/Copout.

## Kitty OSC 133 compatibility

Atuin 18.24 moved its capture boundaries from OSC 133 to private OSC 18188735
markers. The former downstream Kitty filter stripped Atuin-specific
`history_id=` and `session_id=` parameters from `OSC 133;D` markers; the
new Atuin shell integration does not generate those legacy markers while a
version-2 proxy is active. The old filter is intentionally **not** ported:
re-filtering ordinary OSC 133 sequences could corrupt unrelated shell or
terminal integration. This establishes architectural non-necessity for the
old **Atuin** metadata workaround, not an empirical guarantee that every Kitty
configuration works. Qualify using a fresh Kitty window and the version-2
proxy (`ATUIN_PTY_PROXY_ACTIVE=2`), then check Atuin history and Copout output.

## Branch provenance and upgrades

The `repair/atuin-18.24-clean` integration branch contains **both** the PTY
pixel-dimension patch and these diagnostics, on top of upstream `v18.24.0`
(commit `4ca8b77c8a526b8c294eb864a2b74c95c685c983`). Its downstream
history has three commits: pixel-dimension maintenance, diagnostics, and Rust
formatting. The separate `feature/atuin-1824-capture-diagnostics` branch was
based directly on upstream 18.24.0 and intentionally omits the pixel patch.

For future Atuin upgrades, preserve both pixel-dimension propagation and the
opt-in diagnostics while updating the recorded upstream base in
`.patch-base.env`. The local `just update` workflow manages that rebase.
