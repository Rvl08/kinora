//! File-system helpers: crash-safe replacement and bounded reads.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::layout::TEMP_SUFFIX;

/// The sibling `<name>.tmp` that [`write_atomic`] writes first.
pub(crate) fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(TEMP_SUFFIX);
    path.with_file_name(name)
}

/// Replaces the file at `path` with `bytes` so that, after a crash or power
/// loss at any moment, it holds either its old or its new contents in full:
/// the bytes go to a temporary file in the same folder, are synced to disk,
/// and the temporary file is then renamed over the target.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = temp_path(path);
    let temp = temp.as_path();
    let written = write_synced(temp, bytes).and_then(|()| fs::rename(temp, path));
    if let Err(error) = written {
        // Best effort: a leftover is also removed when the folder is opened.
        let _ = fs::remove_file(temp);
        return Err(error);
    }
    if let Some(dir) = path.parent() {
        sync_dir(dir);
    }
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Makes a rename inside `dir` durable: Unix needs the directory itself
/// synced. Best effort; a failure is logged.
#[cfg(unix)]
pub(crate) fn sync_dir(dir: &Path) {
    if let Err(error) = File::open(dir).and_then(|handle| handle.sync_all()) {
        log::debug!("syncing the folder {} failed: {error}", dir.display());
    }
}

/// Makes a rename inside `dir` durable. NTFS records the rename in its own
/// metadata log, and Windows cannot flush a directory handle from user
/// mode, so there is nothing to do.
#[cfg(not(unix))]
pub(crate) fn sync_dir(_dir: &Path) {}

/// Reads at most `limit` bytes of the file at `path`. A longer file comes
/// back cut at `limit`; callers pass one byte more than they accept, so they
/// can tell.
pub(crate) fn read_prefix(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let limit = u64::try_from(limit).unwrap_or(u64::MAX);
    let mut bytes = Vec::new();
    File::open(path)?.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Removes the file at `path`; a file that is already gone is not an
/// error.
pub(crate) fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_writes_replace_and_leave_no_temporary_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("project.json");
        write_atomic(&path, b"old").expect("first write");
        write_atomic(&path, b"new").expect("second write");
        assert_eq!(fs::read(&path).expect("reads"), b"new");
        assert!(!temp_path(&path).exists());
        assert_eq!(
            temp_path(&path).file_name().and_then(|n| n.to_str()),
            Some("project.json.tmp")
        );
    }

    #[test]
    fn failed_writes_keep_the_old_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        // The target is a folder, so the rename cannot replace it.
        let path = dir.path().join("taken");
        fs::create_dir(&path).expect("folder");
        fs::write(path.join("inside"), b"keep").expect("file");
        assert!(write_atomic(&path, b"new").is_err());
        assert_eq!(fs::read(path.join("inside")).expect("reads"), b"keep");
        assert!(!temp_path(&path).exists());
    }

    #[test]
    fn reads_are_bounded() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("big");
        fs::write(&path, [7u8; 100]).expect("writes");
        assert_eq!(read_prefix(&path, 10).expect("reads").len(), 10);
        assert_eq!(read_prefix(&path, 1000).expect("reads").len(), 100);
        remove_if_present(&path).expect("removes");
        remove_if_present(&path).expect("already gone");
    }
}
