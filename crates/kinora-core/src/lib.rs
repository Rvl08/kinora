//! Shared vocabulary of the Kinora engine.
//!
//! Exact time (ADR-0004): every position, duration and rate in the document
//! model is a [`Rational`], never a float. Floats appear only at the edges
//! (drawing the timeline, feeding a decoder) through explicit conversions such
//! as [`Time::as_secs_f64`].
//!
//! The crate depends on the standard library only. Serialisation, JSON Schema
//! and TypeScript support are opt-in through the `serde`, `schemars` and
//! `ts-rs` features.

mod rational;
mod time;

pub use rational::{MAX_SAFE_INTEGER, Rational, RationalError};
pub use time::{FLICKS_PER_SECOND, FrameRate, Time};
