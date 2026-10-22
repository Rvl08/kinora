//! The error every IPC command can return.

use std::fmt;
use std::io;

use kinora_project::LoadError;
use kinora_project::edit::{EditError, EditRule, EntityKind, HistoryError};
use kinora_project::version::MigrationError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::Revision;

/// Most problems listed in [`IpcError::ProjectInvalid`]; the rest are
/// counted, not sent.
pub const MAX_PROBLEMS: usize = 20;

/// Why a command failed.
///
/// Serialised as `{ "code": "<code>", …details }`. The UI picks a localised
/// message by `code` and fills it from the details; the English `Display`
/// text is for logs only.
///
/// Safety rules for every variant:
/// - no file-system paths (the UI already knows which file the user chose),
///   no OS error strings, no stack traces, no source code locations;
/// - strings are either identifiers the UI sent or messages produced by
///   Kinora's own validation;
/// - anything unexpected becomes [`IpcError::Internal`]: the shell logs the
///   details locally under the incident id and the UI shows only the id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum IpcError {
    /// The command needs an open project and none is open.
    #[error("no project is open")]
    NoProject,
    /// The open project has unsaved changes and the request did not allow
    /// discarding them.
    #[error("the project has unsaved changes")]
    UnsavedChanges,
    /// A referenced entity does not exist.
    #[error("{entity} {id} does not exist")]
    NotFound {
        /// What kind of entity.
        entity: Entity,
        /// The identifier the UI sent.
        id: String,
    },
    /// An argument is malformed or out of range.
    #[error("invalid argument `{argument}`: {reason}")]
    InvalidArgument {
        /// Field path within `args`, e.g. `command.params.at`.
        argument: String,
        /// What is wrong.
        reason: String,
    },
    /// The request was computed against an older state of the project; the
    /// UI should refresh and retry.
    #[error("request made against revision {expected}, project is at {current}")]
    RevisionConflict {
        /// The revision the request carried.
        expected: Revision,
        /// The project's revision.
        current: Revision,
    },
    /// The edit is well-formed but not allowed here (it would overlap a
    /// clip, cut outside a clip, …).
    #[error("the edit was rejected ({rule}): {reason}")]
    EditRejected {
        /// Which rule the edit breaks; the UI explains it in its own words.
        rule: EditRule,
        /// Details in English, for logs and developer tools.
        reason: String,
        /// JSON Pointer to the failing command inside the request's
        /// `command`, e.g. `/params/commands/2`; empty for the command
        /// itself.
        command: String,
    },
    /// Undo was requested with an empty undo history.
    #[error("nothing to undo")]
    NothingToUndo,
    /// Redo was requested with an empty redo history.
    #[error("nothing to redo")]
    NothingToRedo,
    /// The project was saved by a newer Kinora.
    #[error("the project uses format {found}; this version reads up to {supported}")]
    ProjectTooNew {
        /// The file's format version.
        found: u32,
        /// The newest format this build reads.
        supported: u32,
    },
    /// The file is damaged or is not a Kinora project.
    #[error("the project file is invalid ({} problem(s) listed, {omitted} omitted)", .problems.len())]
    ProjectInvalid {
        /// Up to [`MAX_PROBLEMS`] problems.
        problems: Vec<ProjectProblem>,
        /// How many further problems were found but not sent.
        omitted: u32,
    },
    /// A file-system operation failed.
    #[error("file system error: {kind:?}")]
    Io {
        /// What went wrong, without the path or the OS message.
        kind: IoErrorKind,
    },
    /// The operation was cancelled.
    #[error("cancelled")]
    Cancelled,
    /// An unexpected failure. Details are in the local log under `incident`.
    #[error("internal error, incident {incident}")]
    Internal {
        /// Correlates the UI report with the local log entry.
        incident: IncidentId,
    },
}

impl IpcError {
    /// A new [`IpcError::Internal`] with a fresh incident id. The caller logs
    /// the underlying error together with the id before returning this.
    pub fn internal() -> Self {
        Self::Internal {
            incident: IncidentId(Uuid::now_v7()),
        }
    }
}

/// Entity kinds, for [`IpcError::NotFound`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Entity {
    /// A media asset.
    Media,
    /// A timeline.
    Timeline,
    /// A track.
    Track,
    /// A clip.
    Clip,
    /// An effect on a clip.
    Effect,
    /// A transition.
    Transition,
    /// A marker.
    Marker,
    /// A background job.
    Job,
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// One problem in an invalid project file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectProblem {
    /// JSON Pointer inside `project.json` (empty for the whole document).
    pub pointer: String,
    /// What is wrong, from Kinora's parser or validator.
    pub message: String,
}

/// File-system failure kinds, mapped from [`io::ErrorKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IoErrorKind {
    /// The file or folder does not exist.
    NotFound,
    /// Access was denied, or the location is read-only.
    PermissionDenied,
    /// The target already exists.
    AlreadyExists,
    /// The disk is full or the quota is exceeded.
    StorageFull,
    /// The file is locked by another program.
    Busy,
    /// The name is not valid on this file system.
    InvalidName,
    /// A file was found where a folder was expected, or the reverse.
    WrongKind,
    /// Anything else.
    Other,
}

impl From<io::ErrorKind> for IoErrorKind {
    fn from(kind: io::ErrorKind) -> Self {
        use io::ErrorKind as K;
        match kind {
            K::NotFound => Self::NotFound,
            K::PermissionDenied | K::ReadOnlyFilesystem => Self::PermissionDenied,
            K::AlreadyExists | K::DirectoryNotEmpty => Self::AlreadyExists,
            K::StorageFull | K::QuotaExceeded | K::FileTooLarge => Self::StorageFull,
            K::ResourceBusy | K::ExecutableFileBusy => Self::Busy,
            K::InvalidFilename => Self::InvalidName,
            K::IsADirectory | K::NotADirectory => Self::WrongKind,
            _ => Self::Other,
        }
    }
}

/// Keeps only the kind: `io::Error`'s message can name paths.
impl From<io::Error> for IpcError {
    fn from(error: io::Error) -> Self {
        Self::Io {
            kind: error.kind().into(),
        }
    }
}

/// Maps a failed `project.json` read. Messages come from Kinora's parser
/// and validator and contain JSON locations, never file paths.
impl From<LoadError> for IpcError {
    fn from(error: LoadError) -> Self {
        let problems: Vec<ProjectProblem> = match error {
            LoadError::Version(MigrationError::TooNew { found, supported }) => {
                return Self::ProjectTooNew { found, supported };
            }
            LoadError::Invalid { issues } => issues
                .into_iter()
                .map(|issue| ProjectProblem {
                    pointer: issue.path,
                    message: issue.message,
                })
                .collect(),
            other => vec![ProjectProblem {
                pointer: String::new(),
                message: other.to_string(),
            }],
        };
        let omitted = problems.len().saturating_sub(MAX_PROBLEMS);
        Self::ProjectInvalid {
            problems: problems.into_iter().take(MAX_PROBLEMS).collect(),
            omitted: u32::try_from(omitted).unwrap_or(u32::MAX),
        }
    }
}

/// Maps a refused edit. [`EditError::Internal`] becomes
/// [`IpcError::Internal`]: the shell logs the original error under the new
/// incident id before returning it.
impl From<EditError> for IpcError {
    fn from(error: EditError) -> Self {
        let command = error.command_pointer();
        match error.root() {
            EditError::NotFound { entity, id } => Self::NotFound {
                entity: (*entity).into(),
                id: id.to_string(),
            },
            EditError::Rejected { rule, message } => Self::EditRejected {
                rule: *rule,
                reason: message.clone(),
                command,
            },
            EditError::Invalid { issues } => Self::EditRejected {
                rule: EditRule::InvalidResult,
                reason: issues.first().map(ToString::to_string).unwrap_or_default(),
                command,
            },
            EditError::InBatch { .. } | EditError::Internal(_) => Self::internal(),
        }
    }
}

/// Maps a failed apply, undo or redo; see `From<EditError>`.
impl From<HistoryError> for IpcError {
    fn from(error: HistoryError) -> Self {
        match error {
            HistoryError::RevisionConflict { expected, current } => {
                Self::RevisionConflict { expected, current }
            }
            HistoryError::NothingToUndo => Self::NothingToUndo,
            HistoryError::NothingToRedo => Self::NothingToRedo,
            HistoryError::Edit(error) => error.into(),
            HistoryError::RevisionsExhausted => Self::internal(),
        }
    }
}

impl From<EntityKind> for Entity {
    fn from(kind: EntityKind) -> Self {
        match kind {
            EntityKind::Media => Self::Media,
            EntityKind::Timeline => Self::Timeline,
            EntityKind::Track => Self::Track,
            EntityKind::Clip => Self::Clip,
            EntityKind::Effect => Self::Effect,
            EntityKind::Transition => Self::Transition,
            EntityKind::Marker => Self::Marker,
        }
    }
}

/// Identifies one internal failure in the local log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct IncidentId(Uuid);

impl fmt::Display for IncidentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), f)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn serialises_as_code_and_details() {
        let error = IpcError::RevisionConflict {
            expected: Revision(3),
            current: Revision(5),
        };
        assert_eq!(
            serde_json::to_value(&error).expect("serialise"),
            json!({ "code": "revisionConflict", "expected": 3, "current": 5 })
        );
        assert_eq!(
            serde_json::to_value(IpcError::NoProject).expect("serialise"),
            json!({ "code": "noProject" })
        );
    }

    #[test]
    fn io_errors_do_not_leak_paths() {
        let os = io::Error::new(
            io::ErrorKind::PermissionDenied,
            r"access denied: C:\Users\someone\secret\project.json",
        );
        let error = IpcError::from(os);
        let json = serde_json::to_string(&error).expect("serialise");
        assert_eq!(json, r#"{"code":"io","kind":"permissionDenied"}"#);
        assert!(!error.to_string().contains("someone"));
    }

    #[test]
    fn load_errors_map_to_project_errors() {
        let too_new = format!(
            r#"{{"schemaVersion": {}}}"#,
            kinora_project::CURRENT_SCHEMA_VERSION + 1
        );
        let error = IpcError::from(kinora_project::from_json(&too_new).expect_err("too new"));
        assert!(matches!(error, IpcError::ProjectTooNew { .. }));

        let error = IpcError::from(kinora_project::from_json("not json").expect_err("syntax"));
        let IpcError::ProjectInvalid { problems, omitted } = error else {
            panic!("expected ProjectInvalid");
        };
        assert_eq!(omitted, 0);
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].message.contains("line 1"),
            "{}",
            problems[0].message
        );
    }

    #[test]
    fn edit_errors_carry_a_rule_and_the_failing_command() {
        let error = EditError::InBatch {
            index: 2,
            error: Box::new(EditError::Rejected {
                rule: EditRule::Overlap,
                message: "the clip would overlap clip x".into(),
            }),
        };
        assert_eq!(
            serde_json::to_value(IpcError::from(error)).expect("serialise"),
            json!({
                "code": "editRejected",
                "rule": "overlap",
                "reason": "the clip would overlap clip x",
                "command": "/params/commands/2"
            })
        );
        let id = Uuid::now_v7();
        let missing = IpcError::from(HistoryError::Edit(EditError::NotFound {
            entity: EntityKind::Transition,
            id,
        }));
        assert_eq!(
            missing,
            IpcError::NotFound {
                entity: Entity::Transition,
                id: id.to_string()
            }
        );
        assert_eq!(
            IpcError::from(HistoryError::NothingToUndo),
            IpcError::NothingToUndo
        );
        assert!(matches!(
            IpcError::from(EditError::Internal("bug")),
            IpcError::Internal { .. }
        ));
    }

    #[test]
    fn internal_errors_carry_only_an_incident() {
        let value = serde_json::to_value(IpcError::internal()).expect("serialise");
        let object = value.as_object().expect("object");
        assert_eq!(object.len(), 2);
        assert_eq!(object["code"], "internal");
        assert!(object["incident"].is_string());
    }
}
