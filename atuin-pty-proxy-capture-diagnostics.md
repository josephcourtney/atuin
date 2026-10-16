# Atuin 18.24 PTY capture diagnostics

This is an **opt-in, metadata-only** diagnostic extension for the Atuin 18.24
PTY proxy. It operates on Atuin's private `OSC 18188735;C/D;<history-id>`
markers, **not** the pre-18.24 OSC 133 capture protocol.

## Enable

Set `ATUIN_PTY_CAPTURE_DIAGNOSTICS` to an **absolute path** before starting a
new PTY proxy. For example, launch a *new* Kitty process with:

```sh
ATUIN_PTY_CAPTURE_DIAGNOSTICS="$HOME/atuin-pty-capture.log" kitty
```

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

## Applying to an already-patched 18.24 checkout

This branch is based directly on upstream `v18.24.0`; it deliberately omits
the independent PTY pixel-dimension patch. To apply **only** these diagnostics
to an existing clean 18.24 pixel-patched branch, cherry-pick this branch's
single diagnostics commit, rather than replacing that working branch.
