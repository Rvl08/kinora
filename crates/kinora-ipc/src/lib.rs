//! The typed contract between the Kinora shell (Rust, Tauri 2) and its UI
//! (Svelte, TypeScript), and the generator of its TypeScript bindings.
//!
//! # Shape
//!
//! - **Commands** ([`command`]): request/response calls from the UI. Each is a
//!   marker type implementing [`IpcCommand`], which names the Tauri command
//!   and fixes its argument and result types. Every command takes exactly one
//!   parameter, `args`, so the UI calls `invoke(name, { args })` and the shell
//!   declares `fn name(args: <C as IpcCommand>::Args)`; commands whose
//!   `Args` is `()` take no parameter.
//! - **Events** ([`event`]): notifications from the shell, each a marker type
//!   implementing [`IpcEvent`] with a Tauri event name and a payload type.
//! - **Errors** ([`IpcError`]): every command returns
//!   `Result<Output, IpcError>`; the UI receives the error as the rejection
//!   value. Errors are codes with structured details for the UI to localise,
//!   and never carry file-system paths or internal messages.
//!
//! Bulk media data does not travel through this contract: encoded packets use
//! binary responses (ADR-0003), and the document travels only on request
//! ([`command::GetProject`]), never inside events.
//!
//! The contract is independent of Tauri, so the local API (`kinora-api`,
//! JSON-RPC and MCP) can expose the same commands with the same types.
//!
//! # Bindings
//!
//! [`bindings::typescript`] renders every type reachable from the contract,
//! plus `Commands` and `Events` maps, into
//! `apps/desktop/ui/src/lib/ipc/bindings.ts`. A test fails when the
//! checked-in file differs; `cargo run -p kinora-ipc --example generate`
//! rewrites it.

pub mod bindings;
pub mod command;
mod error;
pub mod event;

pub use command::{COMMAND_NAMES, IpcCommand};
pub use error::{Entity, IncidentId, IoErrorKind, IpcError, ProjectProblem};
pub use event::{EVENT_NAMES, IpcEvent};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Counter of document states in the current session: every applied command,
/// undo or redo increments it. Requests that edit carry the revision they were
/// made against, so an edit computed from a stale UI state is rejected with
/// [`IpcError::RevisionConflict`] instead of being applied to the wrong
/// document. Not stored in `project.json`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS,
)]
pub struct Revision(pub u32);

impl Revision {
    /// The next revision, or `None` after `u32::MAX` edits.
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
