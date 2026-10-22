//! IPC commands: requests from the UI to the shell.

use kinora_project::edit::EditorState;
use kinora_project::id::ProjectId;
use kinora_project::{EditCommand, Project, ProjectSettings};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::Revision;

/// A command the UI can invoke.
///
/// The shell implements each one as a Tauri command named [`Self::NAME`]
/// that takes `args: Self::Args` (or nothing when `Args` is `()`) and returns
/// `Result<Self::Output, IpcError>`:
///
/// ```ignore
/// use kinora_ipc::{IpcCommand, IpcError, command::ApplyCommand};
///
/// #[tauri::command]
/// async fn apply_command(
///     shell: tauri::State<'_, Shell>,
///     args: <ApplyCommand as IpcCommand>::Args,
/// ) -> Result<<ApplyCommand as IpcCommand>::Output, IpcError> {
///     shell.apply(args).await
/// }
/// ```
///
/// A shell test compares its `generate_handler!` list with [`COMMAND_NAMES`].
pub trait IpcCommand {
    /// The Tauri command name (snake_case, a Rust function name).
    const NAME: &'static str;
    /// What the command does, for the generated bindings.
    const DOC: &'static str;
    /// The single argument, sent as `{ args }`.
    type Args: Serialize + DeserializeOwned + TS + 'static;
    /// The result on success.
    type Output: Serialize + DeserializeOwned + TS + 'static;
}

/// Visits every command type; implemented by the bindings generator.
pub trait CommandVisitor {
    /// Called once per command, in declaration order.
    fn visit<C: IpcCommand>(&mut self);
}

macro_rules! commands {
    ($( $(#[doc = $doc:literal])* $marker:ident = $name:literal ($args:ty) -> $output:ty; )*) => {
        $(
            $(#[doc = $doc])*
            #[derive(Debug)]
            pub enum $marker {}

            impl IpcCommand for $marker {
                const NAME: &'static str = $name;
                const DOC: &'static str = concat!($($doc, "\n",)*);
                type Args = $args;
                type Output = $output;
            }
        )*

        /// Every command name, in declaration order.
        pub const COMMAND_NAMES: &[&str] = &[$($name),*];

        /// Calls `visitor` for every command, in declaration order.
        pub fn visit_commands(visitor: &mut impl CommandVisitor) {
            $( visitor.visit::<$marker>(); )*
        }
    };
}

commands! {
    /// Creates a project folder `<parentDir>/<name>.kinora` and opens it.
    CreateProject = "create_project" (CreateProjectArgs) -> ProjectOpened;
    /// Opens a project folder, migrating an older format and replaying the
    /// journal of an unclean shutdown.
    OpenProject = "open_project" (OpenProjectArgs) -> ProjectOpened;
    /// Writes the open project to disk (`project.json`, then truncates the
    /// journal).
    SaveProject = "save_project" (SaveProjectArgs) -> ProjectState;
    /// Closes the open project.
    CloseProject = "close_project" (CloseProjectArgs) -> ();
    /// The open project's full document and state. The UI calls it after
    /// opening and whenever `project:changed` reports a newer revision.
    GetProject = "get_project" (()) -> ProjectSnapshot;
    /// Applies one edit (which may be a batch) as one undo step.
    ApplyCommand = "apply_command" (ApplyCommandArgs) -> ProjectState;
    /// Reverts the most recent undo step.
    Undo = "undo" (HistoryArgs) -> ProjectState;
    /// Re-applies the most recently undone step.
    Redo = "redo" (HistoryArgs) -> ProjectState;
    /// Requests cancellation of a background job; its final state arrives as
    /// a `job:progress` event.
    CancelJob = "cancel_job" (CancelJobArgs) -> ();
}

/// Arguments of [`CreateProject`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProjectArgs {
    /// Folder to create the project in, as chosen in the native dialog.
    pub parent_dir: String,
    /// Project name; also the folder name, so it must be a valid file name.
    pub name: String,
    /// Initial settings.
    pub settings: ProjectSettings,
}

/// Arguments of [`OpenProject`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenProjectArgs {
    /// The `.kinora` folder, as chosen in the native dialog, dropped on the
    /// window or taken from the recent-projects list. The shell accepts only
    /// a folder containing `project.json`.
    pub path: String,
}

/// Arguments of [`SaveProject`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveProjectArgs {
    /// `null` saves in place; a folder path saves a copy there and switches
    /// to it ("save as").
    pub save_as: Option<String>,
}

/// Arguments of [`CloseProject`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloseProjectArgs {
    /// Closes even with unsaved changes (they stay recoverable from the
    /// journal until the next open); otherwise fails with `unsavedChanges`.
    pub discard_changes: bool,
}

/// Arguments of [`ApplyCommand`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyCommandArgs {
    /// The revision the UI computed the edit against.
    pub base_revision: Revision,
    /// The edit.
    pub command: EditCommand,
}

/// Arguments of [`Undo`] and [`Redo`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryArgs {
    /// The revision the UI showed when the user asked; a stale request (a
    /// double click) is rejected rather than undoing twice.
    pub base_revision: Revision,
}

/// Arguments of [`CancelJob`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelJobArgs {
    /// The job.
    pub job: JobId,
}

/// The session state of the open project, returned by every edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectState {
    /// Which project.
    pub project: ProjectId,
    /// Current revision.
    pub revision: Revision,
    /// `true` when there are changes since the last save.
    pub dirty: bool,
    /// Label of the step undo would revert; `null` when there is none.
    pub undo_label: Option<String>,
    /// Label of the step redo would re-apply; `null` when there is none.
    pub redo_label: Option<String>,
}

impl ProjectState {
    /// The state of an editing session on `project`.
    pub fn new(project: ProjectId, state: EditorState) -> Self {
        Self {
            project,
            revision: state.revision,
            dirty: state.dirty,
            undo_label: state.undo_label,
            redo_label: state.redo_label,
        }
    }
}

/// The document together with its session state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSnapshot {
    /// Session state.
    pub state: ProjectState,
    /// The document at `state.revision`.
    pub document: Project,
}

/// Result of [`CreateProject`] and [`OpenProject`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectOpened {
    /// The project.
    pub snapshot: ProjectSnapshot,
    /// The format version the file had when it was older than the current
    /// one; the file is rewritten on the next save, after a backup.
    pub migrated_from: Option<u32>,
    /// How many unsaved edits were recovered from the journal.
    pub recovered_edits: u32,
}

/// Identifies a background job (export, proxy, transcription, …).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct JobId(Uuid);

impl JobId {
    /// A new, unique job id.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }
}
