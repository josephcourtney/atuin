# Atuin fork troubleshooting

Begin with `just doctor`, then `just test-capture` in a newly launched terminal.

1. **Build mismatch:** compare upstream and patched release and full build
   labels, the running proxy executable, and Jerakeen's actual handshake. A
   patched source build can legitimately have a different build ID from
   Homebrew's `NO_GIT` package.
2. **No history:** check `ATUIN_SESSION` and `atuin history list --session`.
   In Atuin 18.24.1, `history list --limit` is not supported.
3. **History but missing output:** establish proxy ownership, command-ID
   association, daemon lifecycle and output settings. The daemon may still
   be using settings from before a config change; restart only after recording
   its state and when doing so is appropriate.
4. **Client sent no build:** upgrade Jerakeen **inside Copout's installed
   environment**, not only in the Jerakeen checkout.
5. **Old proxy in an existing window:** the stable symlink changes only
   future process launches. Start a fresh terminal and inspect actual process
   ancestry rather than assuming a running proxy changed in place.
6. **PTY dimension mismatch:** `just test-proxy` is only a host ioctl smoke
   test. Run a separate controlled proxy resize test before asserting that
   terminal pixel-dimension forwarding is working.
7. **Deployment rollback:** `just rollback` restores the stable symlink. It
   cannot replace binaries already mapped into running processes.
