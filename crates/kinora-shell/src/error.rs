//! Turning failures into IPC errors: details go to the local log, the UI
//! gets a code.

use std::fmt::Display;
use std::io;
use std::path::Path;

use kinora_ipc::{IpcError, ProjectProblem};
use kinora_project::{HistoryError, Issue, LoadError};

/// Logs an unexpected failure under a new incident id and returns the
/// [`IpcError::Internal`] that carries only that id.
pub fn internal_error(context: &str, error: &dyn Display) -> IpcError {
    let ipc = IpcError::internal();
    if let IpcError::Internal { incident } = &ipc {
        log::error!("incident {incident}: {context}: {error}");
    }
    ipc
}

/// A failed file operation: the path and the OS message go to the log, the
/// UI gets the kind.
pub(crate) fn io_error(context: &str, path: &Path, error: io::Error) -> IpcError {
    log::warn!("{context} failed ({}): {error}", path.display());
    IpcError::from(error)
}

/// An argument the shell cannot use.
pub(crate) fn invalid(argument: &str, reason: impl Into<String>) -> IpcError {
    IpcError::InvalidArgument {
        argument: argument.to_owned(),
        reason: reason.into(),
    }
}

/// A folder or file that is not a usable Kinora project.
pub(crate) fn not_a_project(message: &str) -> IpcError {
    IpcError::ProjectInvalid {
        problems: vec![ProjectProblem {
            pointer: String::new(),
            message: message.to_owned(),
        }],
        omitted: 0,
    }
}

/// A document that breaks the format's rules.
pub(crate) fn invalid_project(issues: Vec<Issue>) -> IpcError {
    IpcError::from(LoadError::Invalid { issues })
}

/// A refused edit, undo or redo; an unexpected one is logged first.
pub(crate) fn history_error(error: HistoryError) -> IpcError {
    let ipc = IpcError::from(error.clone());
    if let IpcError::Internal { incident } = &ipc {
        log::error!("incident {incident}: the editor failed: {error}");
    }
    ipc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_keep_only_the_kind() {
        let error = io_error(
            "reading",
            Path::new("/home/someone/secret.kinora/project.json"),
            io::Error::new(io::ErrorKind::NotFound, "no such file: /home/someone"),
        );
        let json = serde_json::to_string(&error).expect("serialises");
        assert_eq!(json, r#"{"code":"io","kind":"notFound"}"#);
    }

    #[test]
    fn internal_errors_carry_only_an_incident() {
        let error = internal_error("testing", &"C:\\Users\\someone");
        let json = serde_json::to_string(&error).expect("serialises");
        assert!(
            json.starts_with(r#"{"code":"internal","incident":""#),
            "{json}"
        );
        assert!(!json.contains("someone"));
    }
}
