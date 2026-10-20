//! Last-resort diagnostics for failures that happen before a window
//! exists: instance lock, database, boot workers, renderer/window setup.
//!
//! The desktop entry discards stderr, so on a GUI launch every failure
//! in that window-less phase used to vanish without a trace ("opened
//! it, nothing appeared"). These helpers make them findable: each fatal
//! record lands in `logs/fatal.log` inside the data directory, and,
//! best-effort, in a `zenity` / `notify-send` dialog so the user sees
//! *something* instead of a silent exit.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Record a fatal startup failure, then (best-effort) tell the user.
///
/// Order: fatal log first (must never take the process down), stderr
/// second (terminal launches keep their output), dialog last (silently
/// dropped when no notifier is installed — CI containers, X-less
/// sessions). Call at the last point where the failure is understood,
/// right before `process::exit`.
pub fn fatal(data_dir: &Path, context: &str, message: impl std::fmt::Display) {
    let message = message.to_string();
    record(data_dir, context, &message);
    eprintln!("SupportOS-Plus: {context}: {message}");
    desktop_alert(&format!(
        "SupportOS+ could not start.\n\n{context}: {message}"
    ));
}

/// Route panics into the fatal log in addition to the default hook.
///
/// Renderer and context-setup panics (the failure class behind the
/// 1.1.0 "opened it, nothing appeared" reports) then leave a trace in
/// `logs/fatal.log` even when the process was started from the desktop
/// entry. Console behavior is unchanged — the default hook still runs.
pub fn install_panic_hook(data_dir: PathBuf) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        record(&data_dir, "panic", &info.to_string());
    }));
}

/// Append one record to `<data_dir>/logs/fatal.log`.
///
/// Best-effort only: diagnostics must never crash the crash path.
fn record(data_dir: &Path, context: &str, message: &str) {
    let dir = data_dir.join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let line = format!(
        "[{}] [{}] {} (version {} {}, session {}, SPP_RENDERER={})\n",
        now_stamp(),
        context,
        message.replace('\n', " | "),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unset".into()),
        std::env::var("SPP_RENDERER").unwrap_or_else(|_| "unset".into()),
    );
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("fatal.log"))
    else {
        return;
    };
    let _ = file.write_all(line.as_bytes());
}

/// `SystemTime` as a plain epoch stamp: dependency-free and unambiguous
/// (the file is a diagnostic, not user-facing output).
fn now_stamp() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => format!("{}.{:03}s", d.as_secs(), d.subsec_millis()),
        Err(e) => format!("clock error: {e}"),
    }
}

/// Show the failure through whatever desktop notifier exists.
///
/// `zenity` gets priority (a real modal dialog); `notify-send` is the
/// fallback notification. Missing tools are ignored — `status()` fails
/// and the attempt is dropped silently.
fn desktop_alert(message: &str) {
    const TITLE: &str = "SupportOS+ failed to start";
    let dialog = std::process::Command::new("zenity")
        .args(["--error", "--title", TITLE, "--text", message])
        .status();
    if dialog.is_err() {
        let _ = std::process::Command::new("notify-send")
            .args(["-u", "critical", TITLE, message])
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fatal_record_writes_log_and_never_panics() {
        let dir = std::env::temp_dir().join(format!(
            "spp-fatal-{}-{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("t")
                .replace('-', "_")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        record(&dir, "test-context", "boom: line one\nline two");
        // Second record must append, not truncate (first line survives).
        record(&dir, "test-context", "second failure");

        let log =
            std::fs::read_to_string(dir.join("logs").join("fatal.log")).expect("fatal.log written");
        assert!(
            log.contains("[test-context] boom: line one | line two"),
            "first record"
        );
        assert!(
            log.contains("[test-context] second failure"),
            "appended record"
        );
        assert!(
            log.contains("SPP_RENDERER="),
            "renderer context must be recorded"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn record_survives_an_unwritable_data_dir() {
        // A *file* where the logs directory should be: create_dir_all
        // fails and `record` must swallow the error without panicking.
        let base = std::env::temp_dir().join(format!("spp-fatal-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::write(&base, b"not a directory").expect("create blocker file");
        record(&base, "test-context", "must not panic");
        let _ = std::fs::remove_file(&base);
    }
}
