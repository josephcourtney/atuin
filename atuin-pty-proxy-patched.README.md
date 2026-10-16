Atuin patched PTY proxy
=======================

This fork patches Atuin's PTY proxy to propagate terminal pixel dimensions
and adds opt-in, metadata-only capture diagnostics (documented in
`atuin-pty-proxy-capture-diagnostics.md`).
Normal Atuin commands use Homebrew; only the PTY proxy uses the patched binary.

Run `just help` for instructions on upgrading `atuin`.
