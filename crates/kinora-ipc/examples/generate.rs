//! Rewrites the generated contract files:
//! - `apps/desktop/ui/src/lib/ipc/bindings.ts` (TypeScript bindings);
//! - `crates/kinora-project/schema/project.v<N>.json` (JSON Schema).
//!
//! Run `cargo run -p kinora-ipc --example generate` after changing a type in
//! `kinora-core`, `kinora-project` or `kinora-ipc`, then review the diff.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Writes `contents` unless the file already holds the same thing: the same
/// text or, for JSON, the same value (Prettier may reformat checked-in JSON
/// without making it stale).
fn write(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let same = |old: String| {
        let old = old.replace("\r\n", "\n");
        old == contents
            || is_json(path) && json_value(&old).is_some_and(|v| Some(v) == json_value(contents))
    };
    if fs::read_to_string(path).is_ok_and(same) {
        println!("unchanged {}", path.display());
    } else {
        fs::write(path, contents)?;
        println!("wrote     {}", path.display());
    }
    Ok(())
}

fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "json")
}

fn json_value(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str(text).ok()
}

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bindings = match kinora_ipc::bindings::typescript() {
        Ok(bindings) => bindings,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let schema_path = root
        .join("crates/kinora-project")
        .join(kinora_project::schema::schema_file_name());
    let results = [
        write(&root.join(kinora_ipc::bindings::BINDINGS_PATH), &bindings),
        write(&schema_path, &kinora_project::schema::project_schema_json()),
    ];
    for result in results {
        if let Err(error) = result {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
