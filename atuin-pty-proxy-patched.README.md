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
