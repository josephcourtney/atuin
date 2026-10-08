//! Opt-in, metadata-only PTY capture diagnostics.
//! No terminal bytes, command strings, OSC parameter values, or history IDs are logged.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

struct Logger {
    file: Mutex<File>,
    start: Instant,
}
static LOGGER: OnceLock<Option<Logger>> = OnceLock::new();

fn logger() -> Option<&'static Logger> {
    LOGGER.get_or_init(|| {
        let path = std::env::var_os("ATUIN_PTY_CAPTURE_DIAGNOSTICS")?;
        if path.is_empty() || path == "0" {
            return None;
        }
        // An explicit path avoids accidentally writing sensitive diagnostics to the
        // terminal or an unexpected working directory.
        let path = std::path::PathBuf::from(path);
        let file = OpenOptions::new().create(true).append(true).mode(0o600).open(&path).ok()?;
        if file.set_permissions(std::fs::Permissions::from_mode(0o600)).is_err() {
            return None;
        }
        Some(Logger { file: Mutex::new(file), start: Instant::now() })
    }).as_ref()
}

pub(crate) fn enabled() -> bool {
    logger().is_some()
}

pub(crate) fn event(kind: &str, detail: impl std::fmt::Display) {
    if let Some(logger) = logger() {
        if let Ok(mut file) = logger.file.lock() {
            let _ = writeln!(file, "elapsed_ms={} pid={} event={} {}", logger.start.elapsed().as_millis(), std::process::id(), kind, detail);
        }
    }
}
