//! The public JSON Schema of `project.json` (ADR-0004 §5).
//!
//! Generated from the Rust types, so it cannot drift from what the code reads:
//! a test compares [`project_schema`] with the checked-in
//! `schema/project.v<N>.json`. The schema describes structure; the semantic
//! rules (references resolve, clips do not overlap) are enforced by
//! [`validate`](crate::validate) and listed in its documentation.

use serde_json::Value;

use crate::model::Project;
use crate::version::CURRENT_SCHEMA_VERSION;

/// Path of the checked-in schema, relative to this crate's directory.
pub fn schema_file_name() -> String {
    format!("schema/project.v{CURRENT_SCHEMA_VERSION}.json")
}

/// The JSON Schema (draft 2020-12) of the current document version.
pub fn project_schema() -> Value {
    let mut schema = schemars::schema_for!(Project);
    schema.insert(
        "$comment".into(),
        "Generated from crates/kinora-project; do not edit. Regenerate with `cargo run -p kinora-ipc --example generate`.".into(),
    );
    schema.to_value()
}

/// [`project_schema`] as pretty-printed JSON with a final newline.
pub fn project_schema_json() -> String {
    // Serialising a `Value` cannot fail.
    let mut json = serde_json::to_string_pretty(&project_schema()).unwrap_or_default();
    json.push('\n');
    json
}
