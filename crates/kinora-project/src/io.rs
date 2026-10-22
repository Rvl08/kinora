//! Reading and writing `project.json`.

use serde::Deserialize;
use serde_json::Value;

use crate::model::Project;
use crate::validate::{Issue, validate};
use crate::version::{self, CURRENT_SCHEMA_VERSION, MigrationError};

/// Largest `project.json` accepted, in bytes. A project with thousands of
/// clips is a few megabytes; the limit stops a corrupt or hostile file from
/// exhausting memory.
pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;

/// Why a document could not be read.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The file exceeds [`MAX_DOCUMENT_BYTES`].
    #[error("the document is {size} bytes; the limit is {MAX_DOCUMENT_BYTES}")]
    TooLarge {
        /// The document's size in bytes.
        size: usize,
    },
    /// The file is not JSON.
    #[error("the document is not valid JSON: {0}")]
    Syntax(#[source] serde_json::Error),
    /// The version is missing, invalid, newer than supported, or could not
    /// be migrated.
    #[error(transparent)]
    Version(#[from] MigrationError),
    /// The JSON does not match the document structure (unknown or missing
    /// fields, wrong types).
    #[error("the document does not match the project format: {0}")]
    Structure(#[source] serde_json::Error),
    /// The structure is right but semantic rules are broken.
    #[error("the document breaks {} rule(s); first: {}", .issues.len(), .issues.first().map(ToString::to_string).unwrap_or_default())]
    Invalid {
        /// Every violation found.
        issues: Vec<Issue>,
    },
}

/// Why a document could not be written.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The document breaks semantic rules; writing it would produce a file
    /// that cannot be opened.
    #[error("the document breaks {} rule(s); first: {}", .issues.len(), .issues.first().map(ToString::to_string).unwrap_or_default())]
    Invalid {
        /// Every violation found.
        issues: Vec<Issue>,
    },
    /// Serialisation failed, or the output did not read back (for example a
    /// NaN, which JSON writes as `null`).
    #[error("the document does not round-trip through JSON: {0}")]
    RoundTrip(String),
}

/// Only the version, to choose between the direct path and migration.
#[derive(Deserialize)]
struct VersionProbe {
    #[serde(rename = "schemaVersion")]
    schema_version: Option<Value>,
}

/// Parses, migrates and validates a `project.json` document.
///
/// Documents at the current version are deserialised straight from the text
/// (one pass, with line and column in errors); older ones go through the
/// untyped migration first.
///
/// # Errors
/// See [`LoadError`]. Messages never contain file-system paths.
pub fn from_json(json: &str) -> Result<Project, LoadError> {
    if json.len() > MAX_DOCUMENT_BYTES {
        return Err(LoadError::TooLarge { size: json.len() });
    }
    let probe: VersionProbe = serde_json::from_str(json).map_err(|e| {
        if e.is_data() {
            // Valid JSON, but not an object.
            LoadError::Version(MigrationError::NotAnObject)
        } else {
            LoadError::Syntax(e)
        }
    })?;
    let is_current = probe
        .schema_version
        .as_ref()
        .and_then(Value::as_u64)
        .is_some_and(|v| v == u64::from(CURRENT_SCHEMA_VERSION));

    let project: Project = if is_current {
        serde_json::from_str(json).map_err(LoadError::Structure)?
    } else {
        let value: Value = serde_json::from_str(json).map_err(LoadError::Syntax)?;
        let migrated = version::migrate(value)?;
        serde_json::from_value(migrated.document).map_err(LoadError::Structure)?
    };

    let issues = validate(&project);
    if issues.is_empty() {
        Ok(project)
    } else {
        Err(LoadError::Invalid { issues })
    }
}

/// Validates and serialises a document as pretty-printed JSON with a final
/// newline: two-space indent, fields in declaration order, map keys sorted,
/// every field present. The same document always produces the same bytes.
///
/// The output is read back before it is returned, so a document that would
/// not open again is never written.
///
/// # Errors
/// See [`SaveError`].
pub fn to_json(project: &Project) -> Result<String, SaveError> {
    let issues = validate(project);
    if !issues.is_empty() {
        return Err(SaveError::Invalid { issues });
    }
    let mut json =
        serde_json::to_string_pretty(project).map_err(|e| SaveError::RoundTrip(e.to_string()))?;
    json.push('\n');
    match from_json(&json) {
        Ok(read_back) if read_back == *project => Ok(json),
        Ok(_) => Err(SaveError::RoundTrip(
            "the document reads back with different values".into(),
        )),
        Err(e) => Err(SaveError::RoundTrip(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProjectSettings;

    #[test]
    fn rejects_non_json_and_non_objects() {
        assert!(matches!(from_json("{"), Err(LoadError::Syntax(_))));
        assert!(matches!(
            from_json("[]"),
            Err(LoadError::Version(MigrationError::NotAnObject))
        ));
        assert!(matches!(
            from_json("{}"),
            Err(LoadError::Version(MigrationError::MissingVersion))
        ));
    }

    #[test]
    fn rejects_newer_documents() {
        let json = format!(r#"{{ "schemaVersion": {} }}"#, CURRENT_SCHEMA_VERSION + 1);
        assert!(matches!(
            from_json(&json),
            Err(LoadError::Version(MigrationError::TooNew { .. }))
        ));
    }

    #[test]
    fn rejects_oversized_documents() {
        let json = " ".repeat(MAX_DOCUMENT_BYTES + 1);
        assert!(matches!(from_json(&json), Err(LoadError::TooLarge { .. })));
    }

    #[test]
    fn new_project_round_trips() {
        let project = Project::new("Holiday", ProjectSettings::default());
        let json = to_json(&project).expect("valid");
        assert!(json.ends_with("}\n"));
        assert_eq!(from_json(&json).expect("reads back"), project);
        assert_eq!(to_json(&project).expect("valid"), json, "deterministic");
    }

    #[test]
    fn refuses_to_write_nan() {
        let mut project = Project::new("NaN", ProjectSettings::default());
        project.timelines[0].tracks.push(crate::model::Track {
            id: crate::id::TrackId::generate(),
            kind: crate::model::TrackKind::Video,
            name: "V1".into(),
            muted: false,
            hidden: false,
            locked: false,
            clips: vec![crate::model::Clip {
                id: crate::id::ClipId::generate(),
                name: None,
                start: kinora_core::Time::ZERO,
                duration: kinora_core::Time::from_secs(1).expect("in range"),
                enabled: true,
                content: crate::model::ClipContent::Adjustment,
                visual: Some(crate::model::Visual {
                    opacity: crate::value::Animated::constant(f64::NAN),
                    ..Default::default()
                }),
                audio: None,
            }],
            transitions: Vec::new(),
        });
        assert!(matches!(to_json(&project), Err(SaveError::Invalid { .. })));
    }
}
