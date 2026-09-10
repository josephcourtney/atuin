Locally patched Atuin used ONLY for `atuin pty-proxy`.

Reason:
Atuin PTY proxy sets ws_xpixel/ws_ypixel to zero. This build patches
crates/atuin-pty-proxy/src/runtime.rs to propagate Crossterm window_size()
pixel width/height.

Source checkout:
~/src/atuin

Rebuild:
  cd ~/src/atuin
  git fetch origin
  # update/rebase patch onto matching release
  cargo build --release -p atuin
  cp target/release/atuin ~/.local/bin/atuin-pty-proxy-patched

After upgrading Homebrew Atuin, rebuild this binary from the same release.
