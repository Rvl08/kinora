//! Document versions and forward migration.
//!
//! # Policy
//!
//! - `schemaVersion` is a single integer, bumped by **every** change to the
//!   document format, additive or not. There is no minor version: an older
//!   build cannot preserve fields it does not know, and re-saving would drop
//!   them, so it must refuse a newer file instead of opening it.
//! - Reading an older file runs the migrations from its version up to
//!   [`CURRENT_SCHEMA_VERSION`], one step at a time, on the untyped JSON
//!   (the old Rust types no longer exist). Each step is a pure function that
//!   receives version `N` and returns version `N + 1`; the engine sets
//!   `schemaVersion` itself.
//! - Reading a newer file fails with [`MigrationError::TooNew`].
//! - Version 0 is the Phase 0/1 draft. It may still change without a
//!   migration until the first release that writes projects for users
//!   freezes it as version 1; from then on every change adds a step here and
//!   a fixture under `tests/fixtures/` (ADR-0004).

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The version this build reads and writes.
pub const CURRENT_SCHEMA_VERSION: u32 = 0;

/// The JSON key holding the version.
pub const VERSION_KEY: &str = "schemaVersion";

/// The `schemaVersion` field of a typed [`Project`](crate::Project).
///
/// A typed document is always at [`CURRENT_SCHEMA_VERSION`]: older documents
/// are migrated before they are deserialised, and any other number is
/// rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(as = "u32"))]
pub struct SchemaVersion(u32);

impl SchemaVersion {
    /// [`CURRENT_SCHEMA_VERSION`].
    pub const CURRENT: Self = Self(CURRENT_SCHEMA_VERSION);

    /// The version number.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for SchemaVersion {
    type Error = MigrationError;

    fn try_from(version: u32) -> Result<Self, Self::Error> {
        if version == CURRENT_SCHEMA_VERSION {
            Ok(Self(version))
        } else {
            Err(MigrationError::NotCurrent {
                found: version,
                current: CURRENT_SCHEMA_VERSION,
            })
        }
    }
}

impl From<SchemaVersion> for u32 {
    fn from(version: SchemaVersion) -> Self {
        version.0
    }
}

impl JsonSchema for SchemaVersion {
    fn schema_name() -> Cow<'static, str> {
        "SchemaVersion".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Document format version. This schema describes exactly this version.",
            "type": "integer",
            "const": CURRENT_SCHEMA_VERSION
        })
    }
}

/// One migration step: turns a version `from` document into a version
/// `from + 1` document.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    /// The version this step reads.
    pub from: u32,
    /// What changed, for the changelog and error messages.
    pub summary: &'static str,
    /// Rewrites the document's root object in place. Must not touch
    /// `schemaVersion`.
    pub apply: fn(&mut Map<String, Value>) -> Result<(), String>,
}

/// The registered steps; `MIGRATIONS[n].from == n` and the last step reaches
/// [`CURRENT_SCHEMA_VERSION`]. Empty while version 0 is the draft.
pub const MIGRATIONS: &[Migration] = &[];

/// Why a document could not be brought to the current version.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MigrationError {
    /// The document is not a JSON object.
    #[error("the document is not a JSON object")]
    NotAnObject,
    /// `schemaVersion` is absent.
    #[error("the document has no `schemaVersion`")]
    MissingVersion,
    /// `schemaVersion` is not a non-negative integer that fits `u32`.
    #[error("`schemaVersion` is not a valid version number")]
    InvalidVersion,
    /// The document was written by a newer build.
    #[error("the document has version {found}, newer than the supported version {supported}")]
    TooNew {
        /// The document's version.
        found: u32,
        /// The newest version this build reads.
        supported: u32,
    },
    /// No step is registered for this version (a bug in the registry).
    #[error("no migration from version {from}")]
    NoPath {
        /// The version without a step.
        from: u32,
    },
    /// A step rejected the document.
    #[error("migration from version {from} failed: {reason}")]
    StepFailed {
        /// The version the failing step reads.
        from: u32,
        /// What the step reported.
        reason: String,
    },
    /// A typed document carried a version other than the current one.
    #[error("expected version {current}, found {found}; migrate first")]
    NotCurrent {
        /// The version found.
        found: u32,
        /// [`CURRENT_SCHEMA_VERSION`].
        current: u32,
    },
}

/// A document brought to the current version.
#[derive(Clone, Debug, PartialEq)]
pub struct Migrated {
    /// The migrated document.
    pub document: Value,
    /// The version the document had before migration.
    pub from_version: u32,
}

impl Migrated {
    /// `true` when at least one step ran, so saving rewrites the file in a
    /// newer format (the shell keeps a backup of the original first).
    pub fn was_migrated(&self) -> bool {
        self.from_version != CURRENT_SCHEMA_VERSION
    }
}

/// Reads `schemaVersion` from an untyped document.
///
/// # Errors
/// [`MigrationError::NotAnObject`], [`MigrationError::MissingVersion`] or
/// [`MigrationError::InvalidVersion`].
pub fn read_version(document: &Value) -> Result<u32, MigrationError> {
    let object = document.as_object().ok_or(MigrationError::NotAnObject)?;
    let version = object
        .get(VERSION_KEY)
        .ok_or(MigrationError::MissingVersion)?;
    version
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or(MigrationError::InvalidVersion)
}

/// Brings `document` to [`CURRENT_SCHEMA_VERSION`] with [`MIGRATIONS`].
///
/// # Errors
/// See [`MigrationError`].
pub fn migrate(document: Value) -> Result<Migrated, MigrationError> {
    migrate_with(document, MIGRATIONS, CURRENT_SCHEMA_VERSION)
}

/// [`migrate`] with an explicit registry and target, for tests.
fn migrate_with(
    mut document: Value,
    steps: &[Migration],
    target: u32,
) -> Result<Migrated, MigrationError> {
    let from_version = read_version(&document)?;
    if from_version > target {
        return Err(MigrationError::TooNew {
            found: from_version,
            supported: target,
        });
    }
    let object = document
        .as_object_mut()
        .ok_or(MigrationError::NotAnObject)?;
    for version in from_version..target {
        let step = usize::try_from(version)
            .ok()
            .and_then(|i| steps.get(i))
            .filter(|step| step.from == version)
            .ok_or(MigrationError::NoPath { from: version })?;
        (step.apply)(object).map_err(|reason| MigrationError::StepFailed {
            from: version,
            reason,
        })?;
        object.insert(VERSION_KEY.to_owned(), Value::from(version + 1));
    }
    Ok(Migrated {
        document,
        from_version,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// v0 → v1: renames `title` to `name`.
    fn rename_title(doc: &mut Map<String, Value>) -> Result<(), String> {
        let title = doc.remove("title").ok_or("`title` is missing")?;
        doc.insert("name".into(), title);
        Ok(())
    }

    /// v1 → v2: adds `tags` with a default.
    fn add_tags(doc: &mut Map<String, Value>) -> Result<(), String> {
        doc.entry("tags").or_insert_with(|| json!([]));
        Ok(())
    }

    const STEPS: &[Migration] = &[
        Migration {
            from: 0,
            summary: "rename title to name",
            apply: rename_title,
        },
        Migration {
            from: 1,
            summary: "add tags",
            apply: add_tags,
        },
    ];

    #[test]
    fn registry_is_contiguous_and_reaches_current() {
        for (index, step) in MIGRATIONS.iter().enumerate() {
            assert_eq!(
                usize::try_from(step.from).ok(),
                Some(index),
                "{}",
                step.summary
            );
        }
        assert_eq!(
            u32::try_from(MIGRATIONS.len()).ok(),
            Some(CURRENT_SCHEMA_VERSION)
        );
    }

    #[test]
    fn current_documents_pass_through_unchanged() {
        let doc = json!({ "schemaVersion": CURRENT_SCHEMA_VERSION, "name": "x" });
        let migrated = migrate(doc.clone()).expect("current version");
        assert_eq!(migrated.document, doc);
        assert!(!migrated.was_migrated());
    }

    #[test]
    fn steps_run_in_order_and_set_the_version() {
        let doc = json!({ "schemaVersion": 0, "title": "Holiday" });
        let migrated = migrate_with(doc, STEPS, 2).expect("migrates");
        assert_eq!(
            migrated.document,
            json!({ "schemaVersion": 2, "name": "Holiday", "tags": [] })
        );
        assert_eq!(migrated.from_version, 0);

        let doc = json!({ "schemaVersion": 1, "name": "Holiday" });
        let migrated = migrate_with(doc, STEPS, 2).expect("migrates");
        assert_eq!(migrated.document["tags"], json!([]));
        assert_eq!(migrated.from_version, 1);
    }

    #[test]
    fn newer_documents_are_refused() {
        let doc = json!({ "schemaVersion": 3 });
        assert_eq!(
            migrate_with(doc, STEPS, 2),
            Err(MigrationError::TooNew {
                found: 3,
                supported: 2
            })
        );
    }

    #[test]
    fn failing_steps_report_their_version() {
        let doc = json!({ "schemaVersion": 0 });
        assert_eq!(
            migrate_with(doc, STEPS, 2),
            Err(MigrationError::StepFailed {
                from: 0,
                reason: "`title` is missing".into()
            })
        );
    }

    #[test]
    fn gaps_in_the_registry_are_detected() {
        let doc = json!({ "schemaVersion": 0, "title": "x" });
        assert_eq!(
            migrate_with(doc.clone(), &STEPS[..1], 2),
            Err(MigrationError::NoPath { from: 1 })
        );
        assert_eq!(
            migrate_with(doc, &STEPS[1..], 2),
            Err(MigrationError::NoPath { from: 0 })
        );
    }

    #[test]
    fn malformed_versions_are_rejected() {
        assert_eq!(migrate(json!([])), Err(MigrationError::NotAnObject));
        assert_eq!(migrate(json!({})), Err(MigrationError::MissingVersion));
        for bad in [
            json!("0"),
            json!(-1),
            json!(0.5),
            json!(null),
            json!(4_294_967_296_u64),
        ] {
            assert_eq!(
                migrate(json!({ "schemaVersion": bad })),
                Err(MigrationError::InvalidVersion),
                "{bad}"
            );
        }
    }

    #[test]
    fn typed_version_accepts_only_current() {
        let current: SchemaVersion =
            serde_json::from_value(json!(CURRENT_SCHEMA_VERSION)).expect("current");
        assert_eq!(current, SchemaVersion::CURRENT);
        assert!(
            serde_json::from_value::<SchemaVersion>(json!(CURRENT_SCHEMA_VERSION + 1)).is_err()
        );
        assert_eq!(
            serde_json::to_value(SchemaVersion::CURRENT).expect("serialise"),
            json!(CURRENT_SCHEMA_VERSION)
        );
    }
}
