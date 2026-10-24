//! The local log file: `kinora.log` in the app's log folder.
//!
//! The log is how an incident id shown in the UI becomes a diagnosis: the
//! shell logs every unexpected failure under its id, with the paths and OS
//! messages that never cross IPC. It stays on the machine (no telemetry);
//! a user can attach it to a bug report. When it passes [`MAX_LOG_BYTES`]
//! at start-up it becomes `kinora.old.log`, so two files at most are kept.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{LevelFilter, Log, Metadata, Record};

/// The log file's name.
pub const LOG_FILE: &str = "kinora.log";

/// The previous log, kept after rotation.
pub const OLD_LOG_FILE: &str = "kinora.old.log";

/// Size at which the log is rotated when the app starts.
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// A [`Log`] that appends one line per record to a file.
#[derive(Debug)]
pub struct FileLogger {
    file: Mutex<File>,
    level: LevelFilter,
}

impl FileLogger {
    /// Opens (or creates) the log in `dir`, rotating a large one first.
    ///
    /// # Errors
    /// When the folder or the file cannot be created.
    pub fn open(dir: &Path, level: LevelFilter) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(LOG_FILE);
        if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
            fs::rename(&path, dir.join(OLD_LOG_FILE))?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            file: Mutex::new(file),
            level,
        })
    }

    /// Makes this the process's logger for the `log` macros.
    ///
    /// # Errors
    /// When a logger is already installed.
    pub fn install(self) -> Result<(), log::SetLoggerError> {
        let level = self.level;
        log::set_boxed_logger(Box::new(self))?;
        log::set_max_level(level);
        Ok(())
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        let line = format!(
            "{millis} {:<5} {}: {}\n",
            record.level(),
            record.target(),
            record.args()
        );
        // A poisoned lock only means another thread panicked mid-write;
        // the file is still usable. Logging never fails the caller.
        let mut file = self.file.lock().unwrap_or_else(|poison| poison.into_inner());
        let _ = file.write_all(line.as_bytes());
    }

    fn flush(&self) {
        let mut file = self.file.lock().unwrap_or_else(|poison| poison.into_inner());
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use log::Level;

    use super::*;

    #[test]
    fn writes_records_at_or_above_the_level() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = FileLogger::open(dir.path(), LevelFilter::Info).expect("opens");
        logger.log(
            &Record::builder()
                .args(format_args!("kept"))
                .level(Level::Warn)
                .target("kinora_shell")
                .build(),
        );
        logger.log(
            &Record::builder()
                .args(format_args!("dropped"))
                .level(Level::Debug)
                .build(),
        );
        logger.flush();
        let text = fs::read_to_string(dir.path().join(LOG_FILE)).expect("reads");
        assert!(text.contains("WARN  kinora_shell: kept\n"), "{text}");
        assert!(!text.contains("dropped"));
    }

    #[test]
    fn a_large_log_is_rotated() {
        let dir = tempfile::tempdir().expect("temp dir");
        let big = vec![b'x'; usize::try_from(MAX_LOG_BYTES).expect("fits") + 1];
        fs::write(dir.path().join(LOG_FILE), big).expect("writes");
        FileLogger::open(dir.path(), LevelFilter::Info).expect("opens");
        assert!(dir.path().join(OLD_LOG_FILE).is_file());
        let fresh = fs::metadata(dir.path().join(LOG_FILE)).expect("exists");
        assert_eq!(fresh.len(), 0);
    }
}
