//! Opt-in, metadata-only PTY capture diagnostics.
//!
//! Never log terminal bytes, command text, raw marker parameters, or history IDs.
//! Diagnostics are process-local and disabled unless an absolute path is supplied.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

struct Logger {
    file: Mutex<File>,
    start: Instant,
}

static LOGGER: OnceLock<Option<Logger>> = OnceLock::new();

fn diagnostic_path(value: OsString) -> Option<PathBuf> {
    if value.is_empty() || value == "0" {
        return None;
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return None;
    }
    Some(path)
}

fn logger() -> Option<&'static Logger> {
    LOGGER
        .get_or_init(|| {
            let path = diagnostic_path(std::env::var_os("ATUIN_PTY_CAPTURE_DIAGNOSTICS")?)?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(&path)
                .ok()?;
            if file.set_permissions(std::fs::Permissions::from_mode(0o600)).is_err() {
                return None;
            }
            Some(Logger {
                file: Mutex::new(file),
                start: Instant::now(),
            })
        })
        .as_ref()
}

pub(crate) fn enabled() -> bool {
    logger().is_some()
}

pub(crate) fn event(kind: &str, detail: impl std::fmt::Display) {
    if let Some(logger) = logger() {
        if let Ok(mut file) = logger.file.lock() {
            let _ = writeln!(
                file,
                "elapsed_ms={} pid={} event={} {}",
                logger.start.elapsed().as_millis(),
                std::process::id(),
                kind,
                detail
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::diagnostic_path;

    #[test]
    fn disabled_and_relative_paths_are_rejected() {
        for value in ["", "0", "relative.log", "./trace.log"] {
            assert!(diagnostic_path(OsString::from(value)).is_none());
        }
    }

    #[test]
    fn an_absolute_path_can_enable_logging() {
        assert_eq!(
            diagnostic_path(OsString::from("/tmp/atuin-capture-diagnostics.log"))
                .unwrap()
                .to_string_lossy(),
            "/tmp/atuin-capture-diagnostics.log"
        );
    }
}
