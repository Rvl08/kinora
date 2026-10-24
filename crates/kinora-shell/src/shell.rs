//! The command surface: one method per IPC command.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kinora_ipc::command::{
    ApplyCommandArgs, CancelJobArgs, CloseProjectArgs, CreateProjectArgs, ForgetRecentProjectArgs,
    HistoryArgs, OpenProjectArgs, ProjectOpened, ProjectSnapshot, ProjectState, RecentProject,
    SaveProjectArgs,
};
use kinora_ipc::event::{ChangeReason, JobProgress, ProjectChanged};
use kinora_ipc::{Entity, IoErrorKind, IpcError};

use crate::error::{invalid, io_error};
use crate::layout::PROJECT_EXTENSION;
use crate::name;
use crate::recent::{self, MAX_SAFE_INTEGER, RecentProjects};
use crate::session::{Failure, Session, canonical, display_path};

/// Receives the shell's notifications. The desktop app forwards them to the
/// window as the `project:changed` and `job:progress` events.
pub trait EventSink: Send + Sync {
    /// The open project changed: the payload of `project:changed`.
    fn project_changed(&self, event: &ProjectChanged);

    /// A background job changed state: the payload of `job:progress`. No
    /// job exists yet; exports and imports arrive in Phase 1.
    fn job_progress(&self, event: &JobProgress);
}

/// The desktop session: at most one open project, and the recent-projects
/// list.
///
/// Each public method is one IPC command and returns the contract's result.
/// Methods take `&mut self`: the app keeps the shell behind a mutex, so
/// commands run one at a time and in order.
pub struct Shell {
    session: Option<Session>,
    recents: RecentProjects,
    events: Box<dyn EventSink>,
    /// Milliseconds since the Unix epoch, for recent-project times.
    clock: fn() -> u64,
}

impl Shell {
    /// A shell that keeps its recent-projects list in `data_dir` (the app's
    /// data folder, created when first needed) and reports changes to
    /// `events`.
    pub fn new(data_dir: &Path, events: impl EventSink + 'static) -> Self {
        Self {
            session: None,
            recents: RecentProjects::new(data_dir),
            events: Box::new(events),
            clock: now_ms,
        }
    }

    /// Replaces the clock used for recent-project times, for tests.
    #[must_use]
    pub fn with_clock(mut self, clock: fn() -> u64) -> Self {
        self.clock = clock;
        self
    }

    /// The open project's state, if a project is open.
    pub fn state(&self) -> Option<ProjectState> {
        self.session.as_ref().map(Session::state)
    }

    /// `create_project`: creates `<parentDir>/<name>.kinora` and opens it. A
    /// project that is open without unsaved changes is closed first.
    ///
    /// # Errors
    /// `invalidArgument` for a bad name or path, `unsavedChanges`, `io`.
    pub fn create_project(&mut self, args: CreateProjectArgs) -> Result<ProjectOpened, IpcError> {
        name::validate(&args.name).map_err(|e| invalid("name", e.to_string()))?;
        let parent = existing_folder(&args.parent_dir, "parentDir")?;
        self.require_closable()?;
        let session = Session::create(&parent, &args.name, args.settings)?;
        Ok(self.switch_to(session, 0))
    }

    /// `open_project`: opens a project folder, recovering unsaved edits from
    /// its journal. Opening the project that is already open returns it as
    /// it is. A project that is open without unsaved changes is closed
    /// first.
    ///
    /// # Errors
    /// `invalidArgument`, `unsavedChanges`, `io`, `projectInvalid`,
    /// `projectTooNew`.
    pub fn open_project(&mut self, args: OpenProjectArgs) -> Result<ProjectOpened, IpcError> {
        let folder = existing_folder(&args.path, "path")?;
        let current = self.session.as_ref().filter(|session| session.folder() == folder);
        if let Some(session) = current {
            let opened = ProjectOpened {
                snapshot: session.snapshot(),
                migrated_from: session.migrated_from(),
                recovered_edits: 0,
            };
            let entry = session.recent_entry((self.clock)());
            self.record_recent(entry);
            return Ok(opened);
        }
        self.require_closable()?;
        let (session, recovered_edits) = Session::open(folder)?;
        Ok(self.switch_to(session, recovered_edits))
    }

    /// `save_project`: saves in place, or as a copy in a new folder that
    /// becomes the open project ("save as"). `saveAs` may name the folder
    /// with or without the `.kinora` extension.
    ///
    /// # Errors
    /// `noProject`, `invalidArgument`, `io`. An `io` error after the file
    /// was written (only the journal failed) still saved the project; the
    /// `project:changed` event says so.
    pub fn save_project(&mut self, args: SaveProjectArgs) -> Result<ProjectState, IpcError> {
        let session = self.session.as_mut().ok_or(IpcError::NoProject)?;
        let (result, moved) = match args.save_as {
            None => (session.save(), false),
            Some(path) => match save_as_target(&path, session.folder())? {
                Target::Current => (session.save(), false),
                Target::New(folder) => (session.save_as(&folder), true),
            },
        };
        let state = session.state();
        if took_effect(&result) {
            let entry = session.recent_entry((self.clock)());
            if moved {
                self.record_recent(entry);
            } else if let Err(error) = self.recents.refresh(entry) {
                log::warn!("could not update the recent-projects list: {error}");
            }
            self.notify(ChangeReason::Saved);
        }
        result.map(|()| state).map_err(|failure| failure.error)
    }

    /// `close_project`: closes the open project; does nothing when none is
    /// open.
    ///
    /// # Errors
    /// `unsavedChanges` unless `discardChanges` is set. Discarded changes
    /// stay in the journal and are recovered when the project is next
    /// opened.
    pub fn close_project(&mut self, args: CloseProjectArgs) -> Result<(), IpcError> {
        let Some(session) = self.session.take() else {
            return Ok(());
        };
        if session.is_dirty() && !args.discard_changes {
            self.session = Some(session);
            return Err(IpcError::UnsavedChanges);
        }
        session.close();
        self.notify(ChangeReason::Closed);
        Ok(())
    }

    /// `get_project`: the open project's document and state.
    ///
    /// # Errors
    /// `noProject`.
    pub fn get_project(&self) -> Result<ProjectSnapshot, IpcError> {
        self.session
            .as_ref()
            .map(Session::snapshot)
            .ok_or(IpcError::NoProject)
    }

    /// `apply_command`: applies one edit as one undo step.
    ///
    /// # Errors
    /// `noProject`, `revisionConflict`, `editRejected`, `notFound`,
    /// `internal`. An `io` error means the edit was applied but could not be
    /// written to the journal; the `project:changed` event carries the new
    /// state, and the journal catches up on the next successful write.
    pub fn apply_command(&mut self, args: ApplyCommandArgs) -> Result<ProjectState, IpcError> {
        self.edit(ChangeReason::Edited, |session| {
            session.apply(args.base_revision, &args.command)
        })
    }

    /// `undo`: reverts the most recent step.
    ///
    /// # Errors
    /// As [`Shell::apply_command`], and `nothingToUndo`.
    pub fn undo(&mut self, args: HistoryArgs) -> Result<ProjectState, IpcError> {
        self.edit(ChangeReason::Undone, |session| session.undo(args.base_revision))
    }

    /// `redo`: re-applies the most recently undone step.
    ///
    /// # Errors
    /// As [`Shell::apply_command`], and `nothingToRedo`.
    pub fn redo(&mut self, args: HistoryArgs) -> Result<ProjectState, IpcError> {
        self.edit(ChangeReason::Redone, |session| session.redo(args.base_revision))
    }

    /// `cancel_job`: there are no background jobs yet, so every job is
    /// unknown.
    ///
    /// # Errors
    /// `notFound` (entity `job`).
    pub fn cancel_job(&self, args: CancelJobArgs) -> Result<(), IpcError> {
        Err(IpcError::NotFound {
            entity: Entity::Job,
            id: args.job.to_string(),
        })
    }

    /// `list_recent_projects`: the recent projects, most recent first. A
    /// list that cannot be read is empty.
    ///
    /// # Errors
    /// None so far; the result type leaves room for them.
    pub fn list_recent_projects(&mut self) -> Result<Vec<RecentProject>, IpcError> {
        Ok(self.recents.list())
    }

    /// `forget_recent_project`: removes a path from the recent list;
    /// succeeds when it is not listed. A folder that still exists is matched
    /// by its resolved path, so any spelling of it works; a missing one by
    /// the exact string the list returned. The open project is never
    /// forgotten.
    ///
    /// # Errors
    /// `io` when the list cannot be written.
    pub fn forget_recent_project(&mut self, args: ForgetRecentProjectArgs) -> Result<(), IpcError> {
        let resolved = Some(Path::new(&args.path))
            .filter(|path| path.is_absolute())
            .and_then(|path| fs::canonicalize(path).ok())
            .map(|path| display_path(&path));
        let candidates: Vec<&str> = [Some(args.path.as_str()), resolved.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        let open = self.session.as_ref().map(Session::display);
        if candidates.iter().any(|&path| Some(path) == open) {
            return Ok(());
        }
        self.recents
            .forget(&candidates)
            .map_err(|e| io_error("updating the recent-projects list", self.recents.file(), e))
    }

    /// Runs an edit, undo or redo, and reports it when the revision moved
    /// (also when only writing the journal failed).
    fn edit(
        &mut self,
        reason: ChangeReason,
        operation: impl FnOnce(&mut Session) -> Result<(), Failure>,
    ) -> Result<ProjectState, IpcError> {
        let session = self.session.as_mut().ok_or(IpcError::NoProject)?;
        let before = session.state().revision;
        let result = operation(session);
        let state = session.state();
        if state.revision != before {
            self.notify(reason);
        }
        result.map(|()| state).map_err(|failure| failure.error)
    }

    /// Makes `session` the open project, closing the previous one (which
    /// has no unsaved changes), and records it as the most recent.
    fn switch_to(&mut self, session: Session, recovered_edits: u32) -> ProjectOpened {
        if let Some(previous) = self.session.take() {
            previous.close();
        }
        let opened = ProjectOpened {
            snapshot: session.snapshot(),
            migrated_from: session.migrated_from(),
            recovered_edits,
        };
        self.record_recent(session.recent_entry((self.clock)()));
        self.session = Some(session);
        self.notify(ChangeReason::Opened);
        opened
    }

    fn require_closable(&self) -> Result<(), IpcError> {
        match &self.session {
            Some(session) if session.is_dirty() => Err(IpcError::UnsavedChanges),
            _ => Ok(()),
        }
    }

    /// The recent list is a convenience: failing to update it is logged,
    /// not reported.
    fn record_recent(&mut self, entry: recent::Entry) {
        if let Err(error) = self.recents.record(entry) {
            log::warn!("could not update the recent-projects list: {error}");
        }
    }

    fn notify(&self, reason: ChangeReason) {
        let event = ProjectChanged {
            reason,
            state: self.state(),
        };
        self.events.project_changed(&event);
    }
}

/// Where "save as" writes.
enum Target {
    /// The open project's own folder: a plain save.
    Current,
    /// A new folder.
    New(PathBuf),
}

fn took_effect(result: &Result<(), Failure>) -> bool {
    match result {
        Ok(()) => true,
        Err(failure) => failure.took_effect,
    }
}

/// Checks a "save as" path: an absolute path whose last part is a valid
/// project name, with or without `.kinora`, in an existing folder.
fn save_as_target(path: &str, current: &Path) -> Result<Target, IpcError> {
    let path = absolute(path, "saveAs")?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("saveAs", "the path does not end with a folder name"))?;
    let stem = strip_extension(file_name);
    name::validate(stem).map_err(|e| invalid("saveAs", e.to_string()))?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid("saveAs", "the path has no parent folder"))?;
    let target = existing_dir(parent)?.join(format!("{stem}.{PROJECT_EXTENSION}"));
    match fs::canonicalize(&target) {
        Ok(existing) if existing == current => Ok(Target::Current),
        Ok(_) => Err(IpcError::Io {
            kind: IoErrorKind::AlreadyExists,
        }),
        Err(_) => Ok(Target::New(target)),
    }
}

/// `name` without a final `.kinora` (in any letter case).
fn strip_extension(name: &str) -> &str {
    let Some(split) = name.len().checked_sub(PROJECT_EXTENSION.len() + 1) else {
        return name;
    };
    match (name.get(..split), name.get(split..)) {
        (Some(stem), Some(extension))
            if !stem.is_empty()
                && extension
                    .strip_prefix('.')
                    .is_some_and(|e| e.eq_ignore_ascii_case(PROJECT_EXTENSION)) =>
        {
            stem
        }
        _ => name,
    }
}

/// An absolute path argument; the UI only passes paths from native dialogs,
/// drag and drop, or the recent list (docs/07 §10).
fn absolute(path: &str, argument: &str) -> Result<PathBuf, IpcError> {
    if path.is_empty() {
        return Err(invalid(argument, "the path is empty"));
    }
    if path.contains('\0') {
        return Err(invalid(argument, "the path contains a NUL character"));
    }
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(invalid(argument, "the path is not absolute"))
    }
}

/// An existing folder named by an argument, fully resolved.
fn existing_folder(path: &str, argument: &str) -> Result<PathBuf, IpcError> {
    existing_dir(&absolute(path, argument)?)
}

fn existing_dir(path: &Path) -> Result<PathBuf, IpcError> {
    let folder = canonical(path)?;
    if folder.is_dir() {
        Ok(folder)
    } else {
        Err(IpcError::Io {
            kind: IoErrorKind::WrongKind,
        })
    }
}

fn now_ms() -> u64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(since_epoch.as_millis())
        .unwrap_or(u64::MAX)
        .min(MAX_SAFE_INTEGER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_is_optional_in_save_as() {
        assert_eq!(strip_extension("Holiday.kinora"), "Holiday");
        assert_eq!(strip_extension("Holiday.KINORA"), "Holiday");
        assert_eq!(strip_extension("Holiday"), "Holiday");
        assert_eq!(strip_extension(".kinora"), ".kinora");
        assert_eq!(strip_extension("kinora"), "kinora");
        assert_eq!(strip_extension("Café.kinora"), "Café");
        assert_eq!(strip_extension("日本.kinora"), "日本");
    }

    #[test]
    fn paths_must_be_absolute() {
        for (path, reason) in [
            ("", "the path is empty"),
            ("relative/folder", "the path is not absolute"),
            ("a\0b", "the path contains a NUL character"),
        ] {
            assert_eq!(
                absolute(path, "path"),
                Err(invalid("path", reason)),
                "{path:?}"
            );
        }
    }
}
