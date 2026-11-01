use std::io::Write;
use std::path::{Path, PathBuf};

pub fn fatal(data_dir: &Path, context: &str, message: impl std::fmt::Display) {
    let message = message.to_string();
    record(data_dir, context, &message);
    eprintln!("SupportOS-Plus: {context}: {message}");
    desktop_alert(&format!(
        "SupportOS+ could not start.\n\n{context}: {message}"
    ));
}

pub fn install_panic_hook(data_dir: PathBuf) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        record(&data_dir, "panic", &info.to_string());
    }));
}

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

fn now_stamp() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => format!("{}.{:03}s", d.as_secs(), d.subsec_millis()),
        Err(e) => format!("clock error: {e}"),
    }
}

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
        let base = std::env::temp_dir().join(format!("spp-fatal-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::write(&base, b"not a directory").expect("create blocker file");
        record(&base, "test-context", "must not panic");
        let _ = std::fs::remove_file(&base);
    }
}
