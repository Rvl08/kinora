//! IPC events: notifications from the shell to the UI.
//!
//! Event names follow Tauri 2's rule (ASCII letters, digits, `-`, `/`, `:`
//! and `_`), so they use `:` where the planning documents wrote `.`
//! (`project:changed`, not `project.changed`).

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::IpcError;
use crate::command::{JobId, ProjectState};

/// An event the shell emits.
pub trait IpcEvent {
    /// The Tauri event name.
    const NAME: &'static str;
    /// When it is emitted, for the generated bindings.
    const DOC: &'static str;
    /// The payload.
    type Payload: Serialize + DeserializeOwned + TS + 'static;
}

/// Visits every event type; implemented by the bindings generator.
pub trait EventVisitor {
    /// Called once per event, in declaration order.
    fn visit<E: IpcEvent>(&mut self);
}

macro_rules! events {
    ($( $(#[doc = $doc:literal])* $marker:ident = $name:literal ($payload:ty); )*) => {
        $(
            $(#[doc = $doc])*
            #[derive(Debug)]
            pub enum $marker {}

            impl IpcEvent for $marker {
                const NAME: &'static str = $name;
                const DOC: &'static str = concat!($($doc, "\n",)*);
                type Payload = $payload;
            }
        )*

        /// Every event name, in declaration order.
        pub const EVENT_NAMES: &[&str] = &[$($name),*];

        /// Calls `visitor` for every event, in declaration order.
        pub fn visit_events(visitor: &mut impl EventVisitor) {
            $( visitor.visit::<$marker>(); )*
        }
    };
}

events! {
    /// The open project changed: an edit, undo, redo, save, open or close,
    /// from this window or from the local API. Carries the state only; the UI
    /// fetches the document with `get_project` when it needs it.
    ProjectChangedEvent = "project:changed" (ProjectChanged);
    /// A background job changed state or made progress. Emitted at most ten
    /// times a second per job, and always for the final state.
    JobProgressEvent = "job:progress" (JobProgress);
}

/// Payload of `project:changed`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectChanged {
    /// What happened.
    pub reason: ChangeReason,
    /// The new state; `null` after the project was closed.
    pub state: Option<ProjectState>,
}

/// Why `project:changed` was emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeReason {
    /// A project was created or opened.
    Opened,
    /// An edit was applied.
    Edited,
    /// An edit was undone.
    Undone,
    /// An edit was redone.
    Redone,
    /// The project was saved.
    Saved,
    /// The project was closed.
    Closed,
}

/// Payload of `job:progress`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobProgress {
    /// The job.
    pub job: JobId,
    /// What the job does.
    pub kind: JobKind,
    /// Where it stands.
    pub state: JobState,
}

/// Kinds of background job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum JobKind {
    /// Rendering a timeline to a file.
    Export,
    /// Building a proxy for smooth preview.
    Proxy,
    /// Speech to text.
    Transcription,
    /// Thumbnails and waveforms.
    Analysis,
    /// Downloading an AI model.
    ModelDownload,
}

/// The state of a job.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum JobState {
    /// Waiting for a worker.
    Queued,
    /// Working.
    Running {
        /// Fraction done, `0..=1`; `null` when unknown.
        progress: Option<f64>,
        /// Estimated milliseconds left; `null` when unknown.
        eta_ms: Option<u32>,
    },
    /// Finished successfully.
    Succeeded,
    /// Finished with an error.
    Failed {
        /// Why.
        error: IpcError,
    },
    /// Stopped on request.
    Cancelled,
}
