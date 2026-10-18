# Maintaining the patched Atuin proxy

This fork tracks upstream Atuin and carries a small downstream change in
`crates/atuin-pty-proxy/src/runtime.rs` for terminal pixel dimensions.
It does not replace the upstream documentation.

## Architecture and identities

- `/opt/homebrew/bin/atuin`: upstream executable, commonly used for shell integration and daemon.
- `~/.local/bin/atuin-patched`: stable symlink to a versioned local patched binary.
- The terminal's **running** proxy can differ from either path after upgrades.
- A manual `exec ... pty-proxy --shell ...` in zsh startup may activate the proxy
  even when `atuin config print` reports `[pty_proxy] enabled = false`.
- Atuin **release**, Git **build**, daemon **protocol**, proxy **process**, and
  Python dependency revisions are separate identities; never infer one from another.

Jerakeen depends on the daemon's gRPC contract, including strict
`x-atuin-build` metadata for Atuin 18.24.1. Copout pins Jerakeen by Git revision;
updating either repository does not upgrade an existing `uv tool` installation.

## Normal upgrade, without premature activation

The `prepare` recipe updates the patch and builds a candidate but does not
install or activate it. Run commands from a **clean** patch-maintenance worktree.

```sh
just status
just doctor
just prepare 18.24.1
just stage
```

The staged path is printed. Review the `patch-target` diff and test output.
Do not change the live proxy while unresolved conflicts or tests remain.

```sh
just install-candidate ~/.local/bin/atuin-patched-18.24.1
```

The exact versioned path must match the printed staging path. Activation
atomically updates the stable symlink and records the previous target for
`just rollback`. Existing running processes do **not** change automatically.

Start a **new** terminal whose zsh startup loads the selected patched executable.
If daemon configuration changed, restart the daemon intentionally and record
its PID/build before continuing. Existing shells can retain the old proxy.

```sh
just doctor
just test-capture
```

`test-capture` launches a fresh login shell and writes a unique record to
Atuin history, then retrieves its captured stdout using the **installed**
Copout. This tests a live environment, not an isolated sandbox. If the local
startup configuration prevents a fresh child shell, treat that as a failure
to investigate rather than proof that capture is working.

`just rollback` restores the previously linked candidate but does not restart
running shells/proxies/daemons. Reopen affected shells and requalify.

The original all-in-one `just upgrade VERSION` remains for compatibility
but installs before live qualification; prefer the staged procedure above.

## Test boundaries

| Recipe | What it proves | What it does not prove |
| --- | --- | --- |
| `just check` | Format, proxy crate compilation and tests | Running daemon capture |
| `just verify-candidate` | Built release matches Homebrew release | Exact build/protocol identity |
| `just test-proxy` | Host PTY ioctl preserves row/column/pixel geometry | Patched proxy forwards SIGWINCH |
| `just doctor` | Inspectable binary/process/config state | Fresh output persistence |
| `just test-capture` | New history entry's stdout is retrievable | Universal stderr completeness or geometry |
| `just qualify-candidate` | Static candidate checks | Running installation |
| `just qualify-live` | Prerequisite diagnostics and captured stdout | All downstream consumers |

Do not report a complete upgrade solely because release versions match or
`Status` succeeds. For build-specific interoperability, run downstream tests
using the actual deployed daemon and inspect their resolved dependencies.

## Downstream release checklist

1. Compare upstream changes to daemon messages, protobuf descriptors, build
   metadata, capture registration, and storage schema.
2. Rebase and check the patch in a disposable worktree.
3. Test the versioned candidate, then activate deliberately.
4. Regenerate/review Jerakeen bindings if necessary; run strict-build
   handshake and streaming-RPC tests against the running daemon.
5. Update Copout's pinned Jerakeen commit and lockfile; run all tests.
6. Install Copout from the qualified revision and inspect `uv tool list`,
   its environment and the actual pinned Git dependency.
7. Run `just test-capture` in a fresh terminal.
8. Qualify Catherd's direct history database reads independently from RPCs,
   and Raedboc's command-history linkage as applicable.
9. Merge reviewed PRs. Clean only known disposable worktrees and branches.
   Squash merging breaks simple ancestry-based cleanup checks.

## Common failures

| Symptom | Investigation |
| --- | --- |
| `FAILED_PRECONDITION: client sent no build` | Installed Jerakeen source revision and daemon build handshake |
| `GetCommandOutput` returns `None` | Running daemon effective output state; proxy, command-ID association and persistence |
| `pty_proxy.enabled = false`, proxy active | Explicit zsh startup invocation and running process ancestry |
| Same release, different build string | Expected with patched source; verify contract, not string equality |
| Empty Copout selector | Current `ATUIN_SESSION`, fresh history records and selector semantics |
| Correct Git source, old global command | `uv tool` installed revision and private Python environment |
| `completeness="unknown"` | Upstream has not certified completeness; not itself a missing-capture error |

Do not restart or reconfigure the production daemon merely because a probe
fails. Record the running build and state first, then make one controlled
change and re-run the same probe.

## Shared-maintenance scripts

The general patch machinery lives in
`~/.config/just/lib/downstream-patch.just` and
`~/.config/just/lib/versioned-install.just`. Preserve these imports.

Scripts under `scripts/` implement diagnostics and safe installation
mechanics. Python standard-library unit tests live in `tests/test_atuin_maintenance.py`.
Run them with `python3 -m unittest discover -s tests -p 'test_atuin_maintenance.py'`.

`just report` emits JSON to stdout, including environment paths and process
information. Inspect it before sharing. It does not include the text of shell
history, but may reveal sensitive local paths.
