//! An open project: its folder, its editing session and its journal.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kinora_ipc::IpcError;
use kinora_ipc::command::{ProjectSnapshot, ProjectState};
use kinora_project::edit::journal::{self, Entry, Header, MAX_JOURNAL_BYTES};
use kinora_project::{
    CURRENT_SCHEMA_VERSION, Clip, EditCommand, Editor, LoadError, MAX_DOCUMENT_BYTES, Project,
    ProjectSettings, Revision, Time, Timeline, from_json, to_json,
};
use serde::Deserialize;

use crate::error::{
    history_error, internal_error, invalid, invalid_project, io_error, not_a_project,
};
use crate::fsx;
use crate::journal_file::JournalFile;
use crate::layout::{self, DISCARDED_JOURNAL_FILE, JOURNAL_FILE, PROJECT_EXTENSION, PROJECT_FILE};
use crate::recent;

/// A failed session operation.
#[derive(Debug)]
pub(crate) struct Failure {
    /// What the UI is told.
    pub(crate) error: IpcError,
    /// `true` when the change was made anyway (an edit applied, a file
    /// saved) and only writing the journal afterwards failed: the session
    /// moved on, and crash recovery catches up on the next successful write.
    pub(crate) took_effect: bool,
}

impl From<IpcError> for Failure {
    fn from(error: IpcError) -> Self {
        Self {
            error,
            took_effect: false,
        }
    }
}

/// The change stands, but the journal does not record it yet.
fn unjournalled(error: IpcError) -> Failure {
    Failure {
        error,
        took_effect: true,
    }
}

/// An open project.
#[derive(Debug)]
pub(crate) struct Session {
    /// The project folder, fully resolved.
    folder: PathBuf,
    /// `folder` as the UI and the recent-projects list show it.
    display: String,
    editor: Editor,
    /// Fingerprint of the `project.json` the journal continues.
    base: String,
    journal: JournalFile,
    /// The format version the file had when opened, until it is saved in
    /// the current one.
    migrated_from: Option<u32>,
}

impl Session {
    /// Creates `<parent>/<name>.kinora` (the name already checked, `parent`
    /// fully resolved) with a new project, and opens it.
    pub(crate) fn create(
        parent: &Path,
        name: &str,
        settings: ProjectSettings,
    ) -> Result<Self, IpcError> {
        let editor = Editor::new(Project::new(name, settings)).map_err(|issues| {
            let reason = issues.first().map(ToString::to_string).unwrap_or_default();
            invalid("settings", reason)
        })?;
        let json = serialise(editor.project())?;
        let folder = parent.join(format!("{name}.{PROJECT_EXTENSION}"));
        fs::create_dir(&folder).map_err(|e| io_error("creating the project folder", &folder, e))?;
        let started = canonical(&folder).and_then(|resolved| Self::start(resolved, editor, &json));
        if started.is_err() {
            // The folder was created just now and holds only Kinora's files.
            remove_new_folder(&folder);
        }
        started
    }

    /// Writes `json` as `project.json` in `folder` and starts its journal.
    fn start(folder: PathBuf, mut editor: Editor, json: &str) -> Result<Self, IpcError> {
        let file = folder.join(PROJECT_FILE);
        fsx::write_atomic(&file, json.as_bytes())
            .map_err(|e| io_error("writing project.json", &file, e))?;
        let base = journal::fingerprint(json.as_bytes());
        let header = header_line(&editor.begin_journal(base.clone()))?;
        let journal = start_journal(&folder, header)?;
        Ok(Self {
            display: display_path(&folder),
            folder,
            editor,
            base,
            journal,
            migrated_from: None,
        })
    }

    /// Opens the project in `folder` (fully resolved) and recovers the
    /// edits its journal holds. Returns the session and how many edits were
    /// recovered.
    pub(crate) fn open(folder: PathBuf) -> Result<(Self, u32), IpcError> {
        layout::remove_leftovers(&folder);
        let text = read_project(&folder)?;
        let project = from_json(&text).map_err(|error| {
            log::warn!("{} is not a usable project: {error}", folder.display());
            IpcError::from(error)
        })?;
        let migrated_from = original_version(&text);
        let base = journal::fingerprint(text.as_bytes());

        let journal_path = folder.join(JOURNAL_FILE);
        let recovered = match read_journal(&journal_path)? {
            Some(text) => match journal::read(&text, project.id, &base) {
                Ok(recovered) => Some(recovered),
                Err(error) => {
                    log::warn!(
                        "the journal of {} cannot be replayed ({error}); it is kept as {DISCARDED_JOURNAL_FILE}",
                        folder.display()
                    );
                    set_aside(&journal_path);
                    None
                }
            },
            None => None,
        };

        let (editor, recovered_edits, contents) = match recovered {
            Some(recovered) => {
                if let Some(stop) = recovered.stop {
                    log::info!("the journal of {} ends early: {stop:?}", folder.display());
                }
                let (editor, replay) =
                    Editor::recover(project, recovered).map_err(invalid_project)?;
                if let Some((index, error)) = &replay.failure {
                    log::warn!(
                        "entry {index} of the journal of {} could not be replayed: {error}",
                        folder.display()
                    );
                }
                // The replayed part replaces the file before anything is
                // appended, so a torn or failed tail cannot hide new entries.
                (editor, replay.replayed, replay.journal)
            }
            None => {
                let mut editor = Editor::new(project).map_err(invalid_project)?;
                let header = header_line(&editor.begin_journal(base.clone()))?;
                (editor, 0, header)
            }
        };
        let journal = start_journal(&folder, contents)?;
        let recovered_edits = u32::try_from(recovered_edits).unwrap_or(u32::MAX);
        if recovered_edits > 0 {
            log::info!(
                "recovered {recovered_edits} unsaved edit(s) of {}",
                folder.display()
            );
        }
        let session = Self {
            display: display_path(&folder),
            folder,
            editor,
            base,
            journal,
            migrated_from,
        };
        Ok((session, recovered_edits))
    }

    /// The project folder, fully resolved.
    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    /// The project folder as the UI and the recent-projects list show it.
    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    /// The format version the file had when it was opened, if it was older.
    pub(crate) fn migrated_from(&self) -> Option<u32> {
        self.migrated_from
    }

    /// `true` when there are changes since the last save.
    pub(crate) fn is_dirty(&self) -> bool {
        self.editor.state().dirty
    }

    /// The session state the UI shows.
    pub(crate) fn state(&self) -> ProjectState {
        ProjectState::new(self.editor.project().id, self.editor.state())
    }

    /// The document and its state.
    pub(crate) fn snapshot(&self) -> ProjectSnapshot {
        ProjectSnapshot {
            state: self.state(),
            document: self.editor.project().clone(),
        }
    }

    /// The recent-projects entry for this project, opened at `opened_at`.
    pub(crate) fn recent_entry(&self, opened_at: u64) -> recent::Entry {
        let project = self.editor.project();
        let timeline = project.timelines.first();
        recent::Entry {
            path: self.display.clone(),
            name: project.name.clone(),
            opened_at,
            format: timeline.map(|timeline| timeline.format),
            duration: timeline.map(timeline_end),
        }
    }

    /// Applies an edit.
    pub(crate) fn apply(&mut self, base: Revision, command: &EditCommand) -> Result<(), Failure> {
        let outcome = self.editor.apply(base, command).map_err(history_error)?;
        self.record(outcome.journal)
    }

    /// Reverts the most recent step.
    pub(crate) fn undo(&mut self, base: Revision) -> Result<(), Failure> {
        let outcome = self.editor.undo(base).map_err(history_error)?;
        self.record(outcome.journal)
    }

    /// Re-applies the most recently undone step.
    pub(crate) fn redo(&mut self, base: Revision) -> Result<(), Failure> {
        let outcome = self.editor.redo(base).map_err(history_error)?;
        self.record(outcome.journal)
    }

    /// Appends an entry the editor returned and syncs it to disk.
    fn record(&mut self, entry: Option<Entry>) -> Result<(), Failure> {
        let Some(entry) = entry else {
            return Ok(());
        };
        let line = entry
            .to_line()
            .map_err(|e| unjournalled(internal_error("writing a journal entry", &e)))?;
        self.journal.append(&line).map_err(|e| {
            unjournalled(io_error("appending to the journal", self.journal.path(), e))
        })
    }

    /// Saves in place: `project.json` is replaced atomically, then the
    /// journal starts again from the saved file. A migrated file is backed
    /// up first.
    pub(crate) fn save(&mut self) -> Result<(), Failure> {
        let json = serialise(self.editor.project())?;
        let base = journal::fingerprint(json.as_bytes());
        let header = header_line(&self.editor.begin_journal(base.clone()))?;
        if let Some(version) = self.migrated_from {
            self.back_up_original(version)?;
        }
        let file = self.folder.join(PROJECT_FILE);
        fsx::write_atomic(&file, json.as_bytes())
            .map_err(|e| io_error("saving project.json", &file, e))?;
        self.migrated_from = None;
        self.editor.mark_saved();
        self.base = base;
        self.journal
            .replace(header)
            .map_err(|e| unjournalled(io_error("restarting the journal", self.journal.path(), e)))
    }

    /// Saves a copy in `target`, a folder that does not exist yet (its name
    /// already checked), and continues there. The old folder keeps its last
    /// saved state: its journal, which holds the edits now saved in the
    /// copy, is removed.
    pub(crate) fn save_as(&mut self, target: &Path) -> Result<(), Failure> {
        let json = serialise(self.editor.project())?;
        let base = journal::fingerprint(json.as_bytes());
        let header = header_line(&self.editor.begin_journal(base.clone()))?;
        fs::create_dir(target).map_err(|e| io_error("creating the project folder", target, e))?;
        let written = canonical(target).and_then(|folder| {
            let file = folder.join(PROJECT_FILE);
            fsx::write_atomic(&file, json.as_bytes())
                .map_err(|e| io_error("writing project.json", &file, e))?;
            Ok(folder)
        });
        let folder = match written {
            Ok(folder) => folder,
            Err(error) => {
                remove_new_folder(target);
                return Err(error.into());
            }
        };

        let old_journal = self.folder.join(JOURNAL_FILE);
        self.display = display_path(&folder);
        self.folder = folder;
        self.migrated_from = None;
        self.editor.mark_saved();
        self.base = base;
        let started = self.journal.relocate(self.folder.join(JOURNAL_FILE), header);
        if let Err(error) = fsx::remove_if_present(&old_journal) {
            log::warn!("could not remove {}: {error}", old_journal.display());
        }
        started.map_err(|e| unjournalled(io_error("starting the journal", self.journal.path(), e)))
    }

    /// Ends the session. Without unsaved changes the journal starts again
    /// from the saved file, so the next open has nothing to recover; with
    /// them it is kept, and the next open recovers them.
    pub(crate) fn close(mut self) {
        let result = if self.is_dirty() {
            // Writes any lines a failed append left behind.
            self.journal.write()
        } else {
            match self.editor.begin_journal(self.base.clone()).to_line() {
                Ok(header) => self.journal.replace(header),
                Err(error) => Err(io::Error::other(error)),
            }
        };
        if let Err(error) = result {
            log::warn!(
                "closing {}: the journal could not be updated: {error}",
                self.folder.display()
            );
        }
    }

    /// Copies a migrated `project.json` aside before it is first rewritten
    /// in the current format. An existing backup (the oldest original) is
    /// kept.
    fn back_up_original(&self, version: u32) -> Result<(), IpcError> {
        let backup = self.folder.join(layout::backup_file_name(version));
        if backup.exists() {
            return Ok(());
        }
        fs::copy(self.folder.join(PROJECT_FILE), &backup)
            .map_err(|e| io_error("backing up project.json", &backup, e))?;
        Ok(())
    }
}

/// The fully resolved form of an existing path (links and `..` removed).
pub(crate) fn canonical(path: &Path) -> Result<PathBuf, IpcError> {
    fs::canonicalize(path).map_err(|e| io_error("resolving a path", path, e))
}

/// A resolved path as the UI shows it: on Windows without the `\\?\`
/// prefix whenever the plain form means the same.
pub(crate) fn display_path(path: &Path) -> String {
    dunce::simplified(path).to_string_lossy().into_owned()
}

fn serialise(project: &Project) -> Result<String, IpcError> {
    to_json(project).map_err(|e| internal_error("serialising the project", &e))
}

fn header_line(header: &Header) -> Result<String, IpcError> {
    header
        .to_line()
        .map_err(|e| internal_error("writing a journal header", &e))
}

fn start_journal(folder: &Path, contents: String) -> Result<JournalFile, IpcError> {
    let path = folder.join(JOURNAL_FILE);
    JournalFile::create(path.clone(), contents)
        .map_err(|e| io_error("writing the journal", &path, e))
}

/// Removes a folder this session created a moment ago, after a later step
/// failed.
fn remove_new_folder(folder: &Path) {
    if let Err(error) = fs::remove_dir_all(folder) {
        log::warn!("could not remove {}: {error}", folder.display());
    }
}

/// Reads `project.json` as text, bounded by [`MAX_DOCUMENT_BYTES`].
fn read_project(folder: &Path) -> Result<String, IpcError> {
    let file = folder.join(PROJECT_FILE);
    match fs::metadata(&file) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(not_a_project("the folder's project.json is not a file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(not_a_project("the folder has no project.json"));
        }
        Err(error) => return Err(io_error("reading project.json", &file, error)),
    }
    let bytes = fsx::read_prefix(&file, MAX_DOCUMENT_BYTES + 1)
        .map_err(|e| io_error("reading project.json", &file, e))?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(LoadError::TooLarge { size: bytes.len() }.into());
    }
    String::from_utf8(bytes).map_err(|_| not_a_project("project.json is not UTF-8 text"))
}

/// Only the version of a document.
#[derive(Deserialize)]
struct VersionProbe {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
}

/// The version of a document that [`from_json`] accepted, when it was older
/// than the current one (so it was migrated).
fn original_version(text: &str) -> Option<u32> {
    let probe: VersionProbe = serde_json::from_str(text).ok()?;
    (probe.schema_version != CURRENT_SCHEMA_VERSION).then_some(probe.schema_version)
}

/// Reads the journal, if there is one, bounded by [`MAX_JOURNAL_BYTES`]
/// (a longer one is refused by `journal::read`).
fn read_journal(path: &Path) -> Result<Option<String>, IpcError> {
    match fsx::read_prefix(path, MAX_JOURNAL_BYTES + 1) {
        Ok(bytes) => Ok(Some(utf8_prefix(bytes))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error("reading the journal", path, error)),
    }
}

/// The longest valid UTF-8 prefix. Damage cuts the journal at the first bad
/// byte; `journal::read` then treats what follows the last complete line as
/// torn, rather than replaying a line with altered text.
fn utf8_prefix(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            let valid = error.utf8_error().valid_up_to();
            let mut bytes = error.into_bytes();
            bytes.truncate(valid);
            String::from_utf8(bytes).unwrap_or_default()
        }
    }
}

/// Keeps a journal that cannot be replayed as [`DISCARDED_JOURNAL_FILE`],
/// replacing an older one.
fn set_aside(journal: &Path) {
    if let Err(error) = fs::rename(journal, journal.with_file_name(DISCARDED_JOURNAL_FILE)) {
        log::warn!("could not set {} aside: {error}", journal.display());
    }
}

/// The end of a timeline's last clip.
fn timeline_end(timeline: &Timeline) -> Time {
    timeline
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .filter_map(Clip::end)
        .max()
        .unwrap_or(Time::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_utf8_cuts_the_text() {
        assert_eq!(utf8_prefix(b"line\n".to_vec()), "line\n");
        assert_eq!(utf8_prefix(b"one\ntw\xffo\n".to_vec()), "one\ntw");
    }

    #[test]
    fn current_documents_were_not_migrated() {
        let json = to_json(&Project::new("A", ProjectSettings::default())).expect("valid");
        assert_eq!(original_version(&json), None);
    }
}
