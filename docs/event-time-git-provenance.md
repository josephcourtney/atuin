# Event-time Git provenance — implementation stages

## Boundary and semantics

Atuin currently creates a `History` entry at `history start`, including timestamp, cwd and command, and later resolves the same history ID at `history end`. The existing `History` struct, local database, synced record encoding and daemon start API do not have Git provenance fields.

A command-time Git observation therefore needs to be carried *atomically with the specific history UUID*, without changing what the existing fields mean. The current Copout `git.observed_at_capture=true` remains a separate observation and must not be reinterpreted as historical.

## Stage 1: bounded Git observer (this branch)

`atuin_client::git_provenance::observe_git` produces a read-only observation of the working tree **at invocation time**, not of a command from the past. It:

- Uses Git's public CLI, never reads its internal database or modifies the working tree.
- Bounds total observation latency to 300 ms; a timeout is an explicit unavailable result.
- Suppresses optional Git locks and filesystem-monitor hooks, and disables interactive prompts.
- Collects repository root, HEAD commit if present, symbolic branch/detached status, and tracked-file dirty state when observable.
- Does not collect remotes, diffs, filenames, environment variables, or command content.
- Is not yet called by shell lifecycle hooks and **does not write history or expose any claim of event-time provenance**.

Regression tests cover non-repositories, detached HEAD, basic tracked modifications, and explicit timeout. This stage can be reviewed and merged without a history schema or compatibility change.

## Stage 2: authoritative event binding

Choose whether to observe at command start, command end, or both. The two phases must have distinct labels: `git switch`, `git checkout`, `git commit`, and shell changes can mutate repository state during execution.

In the `history start` path, call the observer using the same cwd and bind the result to the newly allocated `HistoryId` **before** persisting it or sending it to the daemon. Do not allow asynchronous observations to be misattributed to another command. A failed Git probe must not stop history capture.

Determine storage design explicitly (history schema, local database migration, synced record serialization version, and daemon protobuf compatibility). Preserve backwards compatibility with older records and missing providers. Do **not** overload `intent`, `author`, `cmd_origin`, or output snapshot fields to store provenance.

## Stage 3: public retrieval and Copout consumer

Expose optional Git provenance through an authoritative Atuin history CLI format key or another supported client API. The provider must distinguish unavailable from clean, carry phase/time/source, and preserve linkage to the actual history UUID.

Only after retrieval exists may Jerakeen and Copout adopt an additional versioned `git_at_execution` field. Keep existing `git` capture-time context and `observed_at_capture=true` unchanged.

## Acceptance

Use two commands from the same cwd but different branches; verify distinct phase-appropriate historical snapshots after switching again before query. Test command-start and command-end transitions, linked worktrees, detached HEAD, non-repo cwd, missing Git, timeout, rapid commands and daemon fallback. Run workspace format/lint/tests; qualify the actual patched Atuin shell in Kitty. Never infer historical state from later refs or reflogs.

Source contract: [Copout Git provenance](https://github.com/josephcourtney/copout/blob/main/docs/GIT_PROVENANCE.md).
