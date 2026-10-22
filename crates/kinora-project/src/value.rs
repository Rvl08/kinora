//! Animatable values: vectors, colours, keyframes and effect parameters.
//!
//! Values are `f64`, not rationals: they are visual quantities, not time. The
//! keyframe *times* are exact. Non-finite floats cannot be stored: JSON has no
//! NaN or infinity, and [`to_json`](crate::to_json) refuses a document that
//! would not read back.

use std::collections::BTreeMap;

use kinora_core::Time;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A 2-D vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f64,
    /// Vertical component (positive is down, as on screen).
    pub y: f64,
}

impl Vec2 {
    /// `(0, 0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    /// `(1, 1)`.
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };
}

/// An RGBA colour: sRGB-encoded components in `0..=1`, straight (not
/// premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Color {
    /// Red.
    pub r: f64,
    /// Green.
    pub g: f64,
    /// Blue.
    pub b: f64,
    /// Opacity.
    pub a: f64,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// Opaque white.
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);

    /// An opaque colour.
    pub const fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// `true` when every component is finite and within `0..=1`.
    pub fn is_valid(&self) -> bool {
        [self.r, self.g, self.b, self.a]
            .iter()
            .all(|c| (0.0..=1.0).contains(c))
    }
}

/// How a value moves from one keyframe to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum Easing {
    /// Constant rate.
    #[default]
    Linear,
    /// Keep this keyframe's value until the next keyframe, then jump.
    Hold,
    /// Slow start.
    EaseIn,
    /// Slow end.
    EaseOut,
    /// Slow start and end.
    EaseInOut,
    /// A CSS-style cubic Bézier through `(0,0)`, `(x1,y1)`, `(x2,y2)`, `(1,1)`.
    /// `x1` and `x2` must be within `0..=1`.
    CubicBezier {
        /// First control point, x.
        x1: f64,
        /// First control point, y.
        y1: f64,
        /// Second control point, x.
        x2: f64,
        /// Second control point, y.
        y2: f64,
    },
}

/// A value at a point in time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(rename = "Keyframe_{T}")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Keyframe<T> {
    /// When, relative to the start of the owning clip, so moving the clip
    /// moves its animation. Within `0..=clip.duration`.
    pub at: Time,
    /// The value at `at`.
    pub value: T,
    /// Interpolation from this keyframe to the next one.
    pub easing: Easing,
}

/// A property that is either constant or animated by keyframes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[schemars(rename = "Animated_{T}")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum Animated<T> {
    /// The same value for the whole clip.
    Static {
        /// The value.
        value: T,
    },
    /// Interpolated between keyframes, held before the first and after the
    /// last. At least one keyframe, ordered by strictly increasing `at`.
    Keyframes {
        /// The keyframes.
        keyframes: Vec<Keyframe<T>>,
    },
}

impl<T> Animated<T> {
    /// A constant value.
    pub const fn constant(value: T) -> Self {
        Self::Static { value }
    }

    /// Every value this property takes at a keyframe (or its constant value).
    pub fn values(&self) -> impl Iterator<Item = &T> {
        let (single, many) = match self {
            Self::Static { value } => (Some(value), &[][..]),
            Self::Keyframes { keyframes } => (None, keyframes.as_slice()),
        };
        single.into_iter().chain(many.iter().map(|k| &k.value))
    }
}

/// The value of one effect or transition parameter.
///
/// Externally tagged so the type is explicit and compact:
/// `{ "float": 0.5 }`, `{ "color": { "r": 1, "g": 0, "b": 0, "a": 1 } }`.
/// The effect's manifest (`effects/<id>/effect.toml`) declares which type each
/// parameter has; every keyframe of a parameter has the same type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum ParamValue {
    /// A real number.
    Float(f64),
    /// An integer.
    Int(i32),
    /// A switch.
    Bool(bool),
    /// A point or a size.
    Vec2(Vec2),
    /// A colour.
    Color(Color),
    /// One of the options the effect declares, by its identifier.
    Choice(String),
}

impl ParamValue {
    /// The variant name, for diagnostics and type checks.
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Float(_) => "float",
            Self::Int(_) => "int",
            Self::Bool(_) => "bool",
            Self::Vec2(_) => "vec2",
            Self::Color(_) => "color",
            Self::Choice(_) => "choice",
        }
    }
}

/// Parameters by name. A `BTreeMap` keeps the serialised order stable.
pub type Params<T> = BTreeMap<String, T>;
