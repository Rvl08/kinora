//! The Kinora desktop session, without a window.
//!
//! [`Shell`] implements every command of the IPC contract (`kinora-ipc`) on
//! the local file system. The Tauri app (`apps/desktop/src-tauri`) is a thin
//! layer that runs these methods off the main thread and forwards
//! [`EventSink`] notifications as window events; the same type can back the
//! local API later. Nothing here depends on Tauri, so all of it is tested on
//! Linux and Windows.
//!
//! # A project folder
//!
//! ```text
//! Holiday.kinora/
//! ├─ project.json             the document, replaced atomically on save
//! ├─ journal.jsonl            edits since the last save (crash recovery)
//! ├─ journal.discarded.jsonl  a journal that could not be replayed, kept for support
//! └─ project.v<N>.json        the original of a migrated file, kept before the first save
//! ```
//!
//! Writes go to `<file>.tmp` first, are synced, and are then renamed over the
//! target, so a crash leaves either the old or the new file, never a mix. A
//! leftover `.tmp` is deleted when the folder is opened.
//!
//! # The journal
//!
//! It follows `docs/spikes/edit-core-report.md` §3.3 and ADR-0004
//! amendment 2:
//!
//! - **open:** read `project.json`; if a journal continues it, replay it with
//!   [`Editor::recover`](kinora_project::Editor::recover) and replace the
//!   file with the replayed part before anything is appended; otherwise
//!   start a journal with a fresh header;
//! - **edit, undo, redo:** append the entry the editor returns and sync it;
//! - **save:** write `project.json` atomically, mark the editor saved, and
//!   replace the journal with a new header.
//!
//! # Errors
//!
//! Every failure becomes an [`IpcError`](kinora_ipc::IpcError) without file
//! paths or OS messages. Details go to the local log (see [`logger`]);
//! unexpected failures are logged under the incident id the UI shows.

mod error;
mod fsx;
mod journal_file;
mod layout;
pub mod logger;
pub mod name;
mod recent;
mod session;
mod shell;

pub use error::internal_error;
pub use layout::{DISCARDED_JOURNAL_FILE, JOURNAL_FILE, PROJECT_EXTENSION, PROJECT_FILE};
pub use recent::{CORRUPT_RECENT_FILE, MAX_RECENT_PROJECTS, RECENT_FILE};
pub use shell::{EventSink, Shell};
