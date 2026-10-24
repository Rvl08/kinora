//! The files of a project folder (ADR-0004 §2).

use std::path::Path;

use crate::fsx;

/// Extension of a project folder: `Holiday.kinora`.
pub const PROJECT_EXTENSION: &str = "kinora";

/// The document.
pub const PROJECT_FILE: &str = "project.json";

/// The edits since the last save, for crash recovery.
pub const JOURNAL_FILE: &str = "journal.jsonl";

/// A journal that could not be replayed (stale, damaged, or from another
/// format), kept for a support request until the next one replaces it.
pub const DISCARDED_JOURNAL_FILE: &str = "journal.discarded.jsonl";

/// Appended to a file's name while it is being written.
pub(crate) const TEMP_SUFFIX: &str = ".tmp";

/// The copy of a migrated `project.json` made before it is first rewritten
/// in the current format.
pub(crate) fn backup_file_name(version: u32) -> String {
    format!("project.v{version}.json")
}

/// Removes the temporary files a crash during a write can leave behind.
/// They never hold anything the folder still needs: the rename that would
/// have published them did not happen.
pub(crate) fn remove_leftovers(folder: &Path) {
    for name in [PROJECT_FILE, JOURNAL_FILE] {
        let temp = fsx::temp_path(&folder.join(name));
        if let Err(error) = fsx::remove_if_present(&temp) {
            log::warn!("could not remove {}: {error}", temp.display());
        }
    }
}
