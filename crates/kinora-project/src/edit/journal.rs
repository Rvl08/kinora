//! `journal.jsonl`: the edits made since the last save, for crash recovery
//! (ADR-0004 §2, amendment 2).
//!
//! # Format
//!
//! One JSON object per line, each ending with `\n`:
//!
//! ```text
//! {"kind":"header","format":1,"schemaVersion":0,"project":"…","base":"9f2c…","revision":7}
//! {"kind":"apply","revision":8,"command":{"op":"splitClip","params":{…}}}
//! {"kind":"undo","revision":9,"step":null}
//! {"kind":"redo","revision":10,"step":null}
//! ```
//!
//! - The **header** ties the journal to one `project.json`: `base` is the
//!   [`fingerprint`] of the file's bytes when the journal began, and
//!   `revision` is the editor's revision at that moment. A journal whose
//!   header does not match the file on disk (for example, the editor crashed
//!   between writing `project.json` and truncating the journal) is stale and
//!   is not replayed.
//! - **Entries** record what the user did, not what changed: the command
//!   itself, or an undo or redo. Replaying them through an [`Editor`] rebuilds
//!   the document and its undo history.
//! - An undo or redo of a step that is older than the journal (made before
//!   the last save) cannot be rebuilt by replay, so its entry carries the
//!   step (`step`), and replay applies it directly. [`Editor`] decides this.
//!
//! # Reading untrusted journals
//!
//! A crash can cut the last line short, and a damaged disk can garble any
//! line. [`read`] keeps every entry up to the first line it cannot use and
//! reports why it stopped; it never fails because of the entries themselves.
//! Sizes are bounded ([`MAX_JOURNAL_BYTES`], [`MAX_LINE_BYTES`]).
//!
//! The crate does no file I/O: the shell appends [`Header::to_line`] and
//! [`Entry::to_line`] to the file and syncs it to disk.
//!
//! [`Editor`]: super::Editor

use serde::{Deserialize, Serialize};

use crate::command::EditCommand;
use crate::id::ProjectId;
use crate::version::CURRENT_SCHEMA_VERSION;

use super::{Patch, Revision};

/// Version of the journal format, including the patches it may carry.
pub const FORMAT: u32 = 1;

/// Largest journal read, in bytes.
pub const MAX_JOURNAL_BYTES: usize = 256 * 1024 * 1024;

/// Largest single line read, in bytes. A batch of many thousands of commands
/// stays well below it.
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

/// A fingerprint of `project.json`'s bytes: FNV-1a 64-bit, as 16 lowercase
/// hexadecimal digits.
///
/// It identifies which saved file a journal continues. It is not
/// cryptographic: it guards against stale journals, not against tampering.
/// The journal is untrusted input either way, and replay validates
/// everything it applies.
pub fn fingerprint(bytes: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let hash = bytes.iter().fold(OFFSET, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    });
    format!("{hash:016x}")
}

/// The first line of a journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Header {
    /// [`FORMAT`] of the writer.
    pub format: u32,
    /// The document's schema version; commands are written in its terms.
    pub schema_version: u32,
    /// The project.
    pub project: ProjectId,
    /// [`fingerprint`] of the `project.json` the entries apply to.
    pub base: String,
    /// The editor's revision when the journal began; the first entry has the
    /// next one.
    pub revision: Revision,
}

/// An undo step written into the journal because replay cannot rebuild it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordedStep {
    /// The undo history label.
    pub label: String,
    /// The patch the undo or redo applied.
    pub patch: Patch,
}

/// One user action after the header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Entry {
    /// A command was applied.
    Apply {
        /// The revision it produced.
        revision: Revision,
        /// The command.
        command: Box<EditCommand>,
    },
    /// The most recent step was undone.
    Undo {
        /// The revision it produced.
        revision: Revision,
        /// The step, when it is older than the journal.
        step: Option<RecordedStep>,
    },
    /// The most recently undone step was redone.
    Redo {
        /// The revision it produced.
        revision: Revision,
        /// The step, when it is older than the journal.
        step: Option<RecordedStep>,
    },
}

impl Entry {
    /// The revision the entry produced.
    pub fn revision(&self) -> Revision {
        match self {
            Self::Apply { revision, .. }
            | Self::Undo { revision, .. }
            | Self::Redo { revision, .. } => *revision,
        }
    }

    /// The entry as one journal line, ending with `\n`.
    ///
    /// # Errors
    /// When the entry cannot be serialised (a non-finite float in a command;
    /// such a command is refused by the editor before it is journalled).
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }
}

/// The header line, with its `kind` tag.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum HeaderLine {
    Header(Header),
}

impl Header {
    /// The header as the first journal line, ending with `\n`.
    ///
    /// # Errors
    /// Never in practice (a header holds integers and strings); returned
    /// rather than assumed, so a broken header is never written.
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(&HeaderLine::Header(self.clone()))?;
        line.push('\n');
        Ok(line)
    }
}

/// Why a journal cannot be used at all. The shell then keeps the file aside
/// (it may still help a support request) and starts a new journal.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum JournalError {
    /// The file exceeds [`MAX_JOURNAL_BYTES`].
    #[error("the journal is {size} bytes; the limit is {MAX_JOURNAL_BYTES}")]
    TooLarge {
        /// Its size in bytes.
        size: usize,
    },
    /// The first line is missing or is not a header.
    #[error("the journal has no valid header")]
    MissingHeader,
    /// Written by another journal format.
    #[error("the journal uses format {found}; this version reads format {FORMAT}")]
    UnsupportedFormat {
        /// The journal's format.
        found: u32,
    },
    /// Written for another schema version of the document.
    #[error("the journal was written for document version {found}")]
    OtherSchemaVersion {
        /// The journal's schema version.
        found: u32,
    },
    /// Belongs to another project.
    #[error("the journal belongs to another project")]
    OtherProject,
    /// Continues a different `project.json` than the one on disk.
    #[error("the journal does not continue the saved project file")]
    Stale,
}

/// Where and why reading stopped before the end of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The last line is incomplete (a crash during a write). Expected after
    /// a crash; nothing that was acknowledged is lost.
    TornTail,
    /// A complete line is not a valid entry.
    BadLine {
        /// 1-based line number.
        line: usize,
    },
    /// A line exceeds [`MAX_LINE_BYTES`].
    LineTooLong {
        /// 1-based line number.
        line: usize,
    },
    /// An entry's revision does not follow the previous one.
    OutOfOrder {
        /// 1-based line number.
        line: usize,
    },
}

/// The usable part of a journal.
#[derive(Clone, Debug, PartialEq)]
pub struct Recovered {
    /// The header.
    pub header: Header,
    /// Entries in order, with consecutive revisions after the header's.
    pub entries: Vec<Entry>,
    /// Why reading stopped early, if it did.
    pub stop: Option<Stop>,
}

/// Reads a journal and checks that it continues the `project.json` with
/// fingerprint `base` of project `project`.
///
/// # Errors
/// See [`JournalError`]. Problems after a valid header never fail: reading
/// stops there and [`Recovered::stop`] says why.
pub fn read(text: &str, project: ProjectId, base: &str) -> Result<Recovered, JournalError> {
    if text.len() > MAX_JOURNAL_BYTES {
        return Err(JournalError::TooLarge { size: text.len() });
    }
    // Only complete lines count; anything after the last `\n` was cut short.
    let (complete, torn) = match text.rfind('\n') {
        Some(end) => (&text[..end], !text[end + 1..].is_empty()),
        None => ("", !text.is_empty()),
    };
    let mut lines = complete.split('\n').filter(|_| !complete.is_empty());

    let header = lines
        .next()
        .filter(|line| line.len() <= MAX_LINE_BYTES)
        .and_then(|line| serde_json::from_str::<HeaderLine>(line).ok())
        .map(|HeaderLine::Header(header)| header)
        .ok_or(JournalError::MissingHeader)?;
    if header.format != FORMAT {
        return Err(JournalError::UnsupportedFormat {
            found: header.format,
        });
    }
    if header.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(JournalError::OtherSchemaVersion {
            found: header.schema_version,
        });
    }
    if header.project != project {
        return Err(JournalError::OtherProject);
    }
    if header.base != base {
        return Err(JournalError::Stale);
    }

    let mut entries = Vec::new();
    let mut expected = header.revision;
    let mut stop = None;
    for (index, line) in lines.enumerate() {
        // Line 1 is the header.
        let number = index + 2;
        if line.len() > MAX_LINE_BYTES {
            stop = Some(Stop::LineTooLong { line: number });
            break;
        }
        let Ok(entry) = serde_json::from_str::<Entry>(line) else {
            stop = Some(Stop::BadLine { line: number });
            break;
        };
        if Some(entry.revision()) != expected.next() {
            stop = Some(Stop::OutOfOrder { line: number });
            break;
        }
        expected = entry.revision();
        entries.push(entry);
    }
    if stop.is_none() && torn {
        stop = Some(Stop::TornTail);
    }
    Ok(Recovered {
        header,
        entries,
        stop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::ClipId;

    fn header(project: ProjectId) -> Header {
        Header {
            format: FORMAT,
            schema_version: CURRENT_SCHEMA_VERSION,
            project,
            base: fingerprint(b"{}"),
            revision: Revision(4),
        }
    }

    fn rename(revision: u32) -> Entry {
        Entry::Apply {
            revision: Revision(revision),
            command: Box::new(EditCommand::RenameClip {
                clip: ClipId::generate(),
                name: Some("A".into()),
            }),
        }
    }

    fn journal(project: ProjectId, entries: &[Entry]) -> String {
        let mut text = header(project).to_line().expect("serialises");
        for entry in entries {
            text.push_str(&entry.to_line().expect("serialises"));
        }
        text
    }

    #[test]
    fn fingerprint_is_fnv1a_64() {
        // Published FNV-1a 64 test vectors.
        assert_eq!(fingerprint(b""), "cbf29ce484222325");
        assert_eq!(fingerprint(b"a"), "af63dc4c8601ec8c");
        assert_eq!(fingerprint(b"foobar"), "85944171f73967e8");
    }

    #[test]
    fn round_trips_every_entry_kind() {
        let project = ProjectId::generate();
        let entries = vec![
            rename(5),
            Entry::Undo {
                revision: Revision(6),
                step: None,
            },
            Entry::Redo {
                revision: Revision(7),
                step: Some(RecordedStep {
                    label: "Trim".into(),
                    patch: Patch::default(),
                }),
            },
        ];
        let text = journal(project, &entries);
        assert!(text.starts_with(r#"{"kind":"header","format":1,"#));
        let recovered = read(&text, project, &fingerprint(b"{}")).expect("reads");
        assert_eq!(recovered.header, header(project));
        assert_eq!(recovered.entries, entries);
        assert_eq!(recovered.stop, None);
    }

    #[test]
    fn rejects_journals_for_other_files() {
        let project = ProjectId::generate();
        let text = journal(project, &[rename(5)]);
        let base = fingerprint(b"{}");
        assert_eq!(
            read(&text, ProjectId::generate(), &base),
            Err(JournalError::OtherProject)
        );
        assert_eq!(
            read(&text, project, "0000000000000000"),
            Err(JournalError::Stale)
        );
        assert_eq!(read("", project, &base), Err(JournalError::MissingHeader));
        assert_eq!(
            read("garbage\n", project, &base),
            Err(JournalError::MissingHeader)
        );
        let newer = text.replacen(r#""format":1"#, r#""format":2"#, 1);
        assert_eq!(
            read(&newer, project, &base),
            Err(JournalError::UnsupportedFormat { found: 2 })
        );
    }

    #[test]
    fn keeps_what_precedes_a_damaged_line() {
        let project = ProjectId::generate();
        let base = fingerprint(b"{}");
        let good = journal(project, &[rename(5), rename(6)]);

        // A crash in the middle of the third entry.
        let torn = format!("{good}{}", &rename(7).to_line().expect("line")[..20]);
        let recovered = read(&torn, project, &base).expect("reads");
        assert_eq!(recovered.entries.len(), 2);
        assert_eq!(recovered.stop, Some(Stop::TornTail));

        let garbled = format!(
            "{good}{{\"kind\":\"apply\"}}\n{}",
            rename(7).to_line().expect("line")
        );
        let recovered = read(&garbled, project, &base).expect("reads");
        assert_eq!(recovered.entries.len(), 2);
        assert_eq!(recovered.stop, Some(Stop::BadLine { line: 4 }));

        let skipped = journal(project, &[rename(5), rename(7)]);
        let recovered = read(&skipped, project, &base).expect("reads");
        assert_eq!(recovered.entries.len(), 1);
        assert_eq!(recovered.stop, Some(Stop::OutOfOrder { line: 3 }));
    }

    #[test]
    fn a_header_alone_is_an_empty_journal() {
        let project = ProjectId::generate();
        let recovered = read(
            &header(project).to_line().expect("serialises"),
            project,
            &fingerprint(b"{}"),
        )
        .expect("reads");
        assert!(recovered.entries.is_empty());
        assert_eq!(recovered.stop, None);
    }
}
