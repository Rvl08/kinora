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
/// The session's revision counter, owned by the editing core
/// ([`kinora_project::Editor`]); requests that edit carry it.
pub use kinora_project::Revision;
