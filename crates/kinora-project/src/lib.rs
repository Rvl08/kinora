//! The Kinora project document: `NAME.kinora/project.json` (ADR-0004).
//!
//! - [`Project`] is the whole document: settings, the media pool and the
//!   timelines with their tracks, clips, effects, keyframes and captions.
//! - Time is exact: every position and duration is a [`Time`] (a rational
//!   number of seconds), never a float.
//! - Every entity has a stable, time-ordered identifier (UUIDv7, [`id`]).
//! - The file carries an integer `schemaVersion`. [`from_json`] migrates older
//!   documents forward ([`version`]) and refuses newer ones, so an older build
//!   never silently drops data it does not understand.
//! - Parsing is strict: unknown fields are errors, and [`validate`] checks the
//!   rules the type system cannot express (references resolve, clips do not
//!   overlap, keyframes are ordered).
//! - [`EditCommand`] is the serialisable mutation vocabulary (ADR-0004 §3).
//!   This crate defines its shape; applying and inverting arrive in Phase 1.
//!
//! The JSON Schema in `schema/project.v0.json` is generated from these types
//! by [`schema::project_schema`] and checked by tests.

pub mod command;
pub mod id;
mod io;
pub mod media;
pub mod model;
pub mod schema;
pub mod text;
mod validate;
pub mod value;
pub mod version;

pub use command::{ClipEdge, EditCommand};
pub use io::{LoadError, MAX_DOCUMENT_BYTES, SaveError, from_json, to_json};
pub use kinora_core::{FrameRate, Rational, Time};
pub use model::{
    AudioFormat, AudioMix, BlendMode, ChannelLayout, Clip, ClipContent, ColorSpace, Crop,
    EffectInstance, EffectRef, Marker, MarkerColor, MediaContent, Project, ProjectSettings,
    SolidContent, Speed, Timeline, Track, TrackKind, Transform, Transition, VideoFormat, Visual,
};
pub use validate::{Issue, validate};
pub use version::{CURRENT_SCHEMA_VERSION, SchemaVersion};
