//! The open `journal.jsonl` of a session.
//!
//! The session keeps the text the file must hold (the header and every
//! entry since) and writes it in one of two ways:
//!
//! - **append:** the new line is written at the end of the open file and
//!   synced before the edit is acknowledged;
//! - **rewrite:** the whole text replaces the file atomically. This starts
//!   every journal (after open, recovery or save), and it repairs the file
//!   after a failed append, which may have left part of a line behind.
//!
//! A failed write is retried by the next one, so a passing problem (a full
//! disk that was cleaned up, a file briefly locked by a scanner) heals by
//! itself, and the file never holds a torn line followed by good ones.

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::fsx;

/// The journal file and the text it must hold.
#[derive(Debug)]
pub(crate) struct JournalFile {
    path: PathBuf,
    /// Everything the file must contain.
    contents: String,
    /// The file, open for writing, and how many bytes of `contents` it holds;
    /// `None` when the next write must rewrite the file in full.
    open: Option<(File, usize)>,
}

impl JournalFile {
    /// Writes `contents` as the journal at `path`, replacing any file there.
    pub(crate) fn create(path: PathBuf, contents: String) -> io::Result<Self> {
        let mut journal = Self {
            path,
            contents,
            open: None,
        };
        journal.write()?;
        Ok(journal)
    }

    /// Where the journal lives.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one line (ending with `\n`) and syncs it to disk. On failure
    /// the line is kept and written by the next call.
    pub(crate) fn append(&mut self, line: &str) -> io::Result<()> {
        self.contents.push_str(line);
        self.write()
    }

    /// Replaces the whole journal with `contents`.
    pub(crate) fn replace(&mut self, contents: String) -> io::Result<()> {
        // Closes the file first: Windows may refuse to rename over a file
        // that is still open.
        self.open = None;
        self.contents = contents;
        self.write()
    }

    /// Continues as a new journal at `path` holding `contents` ("save as");
    /// the old file is closed and left to the caller.
    pub(crate) fn relocate(&mut self, path: PathBuf, contents: String) -> io::Result<()> {
        self.path = path;
        self.replace(contents)
    }

    /// Writes whatever the file is missing; a failure leaves the file to be
    /// rewritten in full next time.
    pub(crate) fn write(&mut self) -> io::Result<()> {
        let result = self.write_missing();
        if result.is_err() {
            self.open = None;
        }
        result
    }

    fn write_missing(&mut self) -> io::Result<()> {
        if let Some((file, written)) = &mut self.open {
            let missing = self.contents.as_bytes().get(*written..).unwrap_or_default();
            if !missing.is_empty() {
                file.write_all(missing)?;
                file.sync_data()?;
                *written = self.contents.len();
            }
            return Ok(());
        }
        fsx::write_atomic(&self.path, self.contents.as_bytes())?;
        let mut file = OpenOptions::new().write(true).open(&self.path)?;
        file.seek(SeekFrom::End(0))?;
        self.open = Some((file, self.contents.len()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn appends_after_the_initial_contents() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("journal.jsonl");
        let mut journal = JournalFile::create(path.clone(), "header\n".into()).expect("creates");
        journal.append("one\n").expect("appends");
        journal.append("two\n").expect("appends");
        assert_eq!(
            fs::read_to_string(&path).expect("reads"),
            "header\none\ntwo\n"
        );
        journal.replace("fresh\n".into()).expect("replaces");
        journal.append("three\n").expect("appends");
        assert_eq!(fs::read_to_string(&path).expect("reads"), "fresh\nthree\n");
    }

    #[test]
    fn a_failed_append_is_repaired_by_the_next_write() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("journal.jsonl");
        let mut journal = JournalFile::create(path.clone(), "header\n".into()).expect("creates");
        // Simulate a failure that left a torn line: the file gains garbage
        // and the handle is lost.
        fs::write(&path, "header\non").expect("tears");
        journal.open = None;
        journal.append("one\n").expect("rewrites");
        assert_eq!(fs::read_to_string(&path).expect("reads"), "header\none\n");
    }
}
