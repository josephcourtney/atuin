//! Read-only, bounded Git observation for a future command-lifecycle provenance field.
//!
//! This module does not write history or change shell integration. Callers must
//! associate any observation with the *actual* Atuin history ID and phase.
//! A later inspection must never be presented as an event-time observation.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

const DEFAULT_PROBE_BUDGET: Duration = Duration::from_millis(300);

/// Versioned command-lifecycle provenance envelope.
///
/// An event is explicitly bound to the Atuin history UUID that initiated the
/// probe. No consumer may substitute a later snapshot for this evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitProvenanceEvent {
    pub schema_version: u8,
    pub history_id: crate::history::HistoryId,
    pub phase: GitObservationPhase,
    pub observed_at: time::OffsetDateTime,
    pub outcome: GitObservationOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitObservationPhase {
    CommandStart,
    CommandEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum GitObservationOutcome {
    Observed(GitObservation),
    Unavailable(GitObservationUnavailable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitObservationUnavailable {
    GitUnavailable,
    NotRepository,
    Timeout,
    Failed,
}

impl From<GitProbeUnavailable> for GitObservationUnavailable {
    fn from(value: GitProbeUnavailable) -> Self {
        match value {
            GitProbeUnavailable::GitUnavailable => Self::GitUnavailable,
            GitProbeUnavailable::NotRepository => Self::NotRepository,
            GitProbeUnavailable::Timeout => Self::Timeout,
            GitProbeUnavailable::Failed => Self::Failed,
        }
    }
}

impl GitProvenanceEvent {
    pub const SCHEMA_VERSION: u8 = 1;

    /// Attach a result collected during this command's known lifecycle phase.
    ///
    /// The caller must supply the command's real history ID, not a guessed
    /// match from shell timestamps or a later repository inspection.
    #[must_use]
    pub fn new(
        history_id: crate::history::HistoryId,
        phase: GitObservationPhase,
        observed_at: time::OffsetDateTime,
        result: GitProbeResult,
    ) -> Self {
        let outcome = match result {
            GitProbeResult::Observed(value) => GitObservationOutcome::Observed(value),
            GitProbeResult::Unavailable(value) => {
                GitObservationOutcome::Unavailable(value.into())
            }
        };
        Self {
            schema_version: Self::SCHEMA_VERSION,
            history_id,
            phase,
            observed_at,
            outcome,
        }
    }
}

/// Fields observed from a working tree at one specific point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitObservation {
    pub root: String,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    /// None means the status probe did not complete; do not treat it as clean.
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitProbeUnavailable {
    GitUnavailable,
    NotRepository,
    Timeout,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitProbeResult {
    Observed(GitObservation),
    Unavailable(GitProbeUnavailable),
}

async fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let result = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8(result.stdout).ok())
        .flatten()
        .map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
}

/// Observe Git *now*, within a total latency budget.
///
/// This is an opt-in substrate for future Atuin history persistence, not a
/// reconstruction of Git state at an earlier command's execution time.
/// Tracked-file status deliberately omits untracked filenames and diffs.
pub async fn observe_git_with_budget(cwd: &Path, budget: Duration) -> GitProbeResult {
    if !cwd.is_dir() {
        return GitProbeResult::Unavailable(GitProbeUnavailable::NotRepository);
    }

    let probe = async {
        let root = match git(cwd, &["rev-parse", "--show-toplevel"]).await {
            Some(root) if !root.is_empty() => root,
            _ => return GitProbeResult::Unavailable(GitProbeUnavailable::NotRepository),
        };
        // An unborn branch has no commit yet; absence is not an error.
        let commit = git(cwd, &["rev-parse", "--verify", "HEAD"]).await;
        let branch = git(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await;
        let dirty = git(cwd, &["status", "--porcelain=v1", "--untracked-files=no"])
            .await
            .map(|s| !s.is_empty());
        GitProbeResult::Observed(GitObservation {
            root,
            commit,
            detached: branch.is_none(),
            branch,
            dirty,
        })
    };

    match timeout(budget, probe).await {
        Ok(result) => result,
        Err(_) => GitProbeResult::Unavailable(GitProbeUnavailable::Timeout),
    }
}

pub async fn observe_git(cwd: &Path) -> GitProbeResult {
    observe_git_with_budget(cwd, DEFAULT_PROBE_BUDGET).await
}

#[cfg(test)]
mod tests {
    use std::process::Command as StdCommand;

    use super::*;

    fn git_ok(cwd: &Path, args: &[&str]) -> String {
        let out = StdCommand::new("git").arg("-C").arg(cwd).args(args).output().unwrap();
        assert!(out.status.success(), "git command failed");
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn provenance_event_serializes_with_uuid_phase_and_unavailability() {
        let id: crate::history::HistoryId =
            "f54c2846d581412392d88856e1523822".parse().unwrap();
        let observed_at = time::OffsetDateTime::UNIX_EPOCH;
        let event = GitProvenanceEvent::new(
            id,
            GitObservationPhase::CommandStart,
            observed_at,
            GitProbeResult::Unavailable(GitProbeUnavailable::Timeout),
        );
        let encoded = serde_json::to_string(&event).unwrap();
        assert!(encoded.contains("command_start"));
        assert!(encoded.contains("timeout"));
        let decoded: GitProvenanceEvent = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, event);
        assert_eq!(decoded.history_id, id);
        assert_eq!(decoded.schema_version, GitProvenanceEvent::SCHEMA_VERSION);
    }

    #[test]
    fn same_history_id_can_hold_distinct_start_and_end_observations() {
        let id: crate::history::HistoryId =
            "f54c2846d581412392d88856e1523822".parse().unwrap();
        let t = time::OffsetDateTime::UNIX_EPOCH;
        let start = GitProvenanceEvent::new(
            id,
            GitObservationPhase::CommandStart,
            t,
            GitProbeResult::Observed(GitObservation {
                root: "/repo".into(),
                commit: Some("a".into()),
                branch: Some("main".into()),
                detached: false,
                dirty: Some(false),
            }),
        );
        let end = GitProvenanceEvent::new(
            id,
            GitObservationPhase::CommandEnd,
            t + time::Duration::seconds(1),
            GitProbeResult::Observed(GitObservation {
                root: "/repo".into(),
                commit: Some("b".into()),
                branch: Some("topic".into()),
                detached: false,
                dirty: Some(false),
            }),
        );
        assert_eq!(start.history_id, end.history_id);
        assert_ne!(start.phase, end.phase);
        assert_ne!(start.outcome, end.outcome);
    }

    #[tokio::test]
    async fn non_repository_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            observe_git(dir.path()).await,
            GitProbeResult::Unavailable(GitProbeUnavailable::NotRepository),
        );
    }

    #[tokio::test]
    async fn observes_current_branch_commit_and_tracked_changes() {
        let dir = tempfile::tempdir().unwrap();
        git_ok(dir.path(), &["init", "-q"]);
        git_ok(dir.path(), &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.org",
            "commit",
            "--allow-empty",
            "-q",
            "-m",
            "init",
        ]);
        let expected_commit = git_ok(dir.path(), &["rev-parse", "HEAD"]);
        let initial = observe_git(dir.path()).await;
        let GitProbeResult::Observed(initial) = initial else {
            panic!("expected Git observation");
        };
        assert_eq!(initial.commit.as_deref(), Some(expected_commit.as_str()));
        assert!(!initial.detached);
        assert!(initial.branch.is_some());
        assert_eq!(initial.dirty, Some(false));

        std::fs::write(dir.path().join("tracked.txt"), "first").unwrap();
        git_ok(dir.path(), &["add", "tracked.txt"]);
        assert_eq!(
            match observe_git(dir.path()).await {
                GitProbeResult::Observed(value) => value.dirty,
                GitProbeResult::Unavailable(_) => panic!("expected Git observation"),
            },
            Some(true)
        );
    }

    #[tokio::test]
    async fn timeout_is_explicit_not_a_clean_observation() {
        let dir = tempfile::tempdir().unwrap();
        git_ok(dir.path(), &["init", "-q"]);
        assert_eq!(
            observe_git_with_budget(dir.path(), Duration::ZERO).await,
            GitProbeResult::Unavailable(GitProbeUnavailable::Timeout)
        );
    }

    #[tokio::test]
    async fn detached_head_is_not_misreported_as_a_branch() {
        let dir = tempfile::tempdir().unwrap();
        git_ok(dir.path(), &["init", "-q"]);
        git_ok(dir.path(), &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.org",
            "commit",
            "--allow-empty",
            "-q",
            "-m",
            "init",
        ]);
        git_ok(dir.path(), &["checkout", "--detach", "-q", "HEAD"]);
        let GitProbeResult::Observed(observed) = observe_git(dir.path()).await else {
            panic!("expected Git observation");
        };
        assert!(observed.detached);
        assert!(observed.branch.is_none());
        assert!(observed.commit.is_some());
    }
}
