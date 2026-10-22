//! Time positions, durations and frame rates on top of [`Rational`].

use std::fmt;

use crate::{Rational, RationalError};

/// A point or a span on a time axis, in seconds, as an exact rational.
///
/// The same type serves positions (a clip's start), durations (its length)
/// and offsets (a keyframe relative to its clip); the field name says which.
/// Negative values are representable because offsets can be negative; the
/// document validator decides where they are allowed.
///
/// Serialised exactly like [`Rational`]: `{ "num": 1001, "den": 30000 }`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Time(Rational);

impl Time {
    /// Zero seconds.
    pub const ZERO: Self = Self(Rational::ZERO);

    /// `num / den` seconds.
    ///
    /// # Errors
    /// As [`Rational::new`].
    pub fn new(num: i64, den: u32) -> Result<Self, RationalError> {
        Rational::new(num, den).map(Self)
    }

    /// A whole number of seconds.
    ///
    /// # Errors
    /// As [`Rational::from_integer`].
    pub fn from_secs(secs: i64) -> Result<Self, RationalError> {
        Rational::from_integer(secs).map(Self)
    }

    /// Wraps a rational number of seconds.
    pub const fn from_rational(secs: Rational) -> Self {
        Self(secs)
    }

    /// The number of seconds as a rational.
    pub const fn as_rational(self) -> Rational {
        self.0
    }

    /// The start of frame `frame` at `rate`: `frame / rate` seconds.
    /// `None` when the result is out of range.
    pub fn from_frames(frame: i64, rate: FrameRate) -> Option<Self> {
        Rational::from_integer(frame)
            .ok()?
            .checked_div(rate.0)
            .map(Self)
    }

    /// The index of the frame shown at this time: `floor(self · rate)`.
    ///
    /// A frame is shown from its start up to, not including, the next start,
    /// so a time exactly on a boundary belongs to the later frame.
    pub fn frame_floor(self, rate: FrameRate) -> Option<i64> {
        self.0.checked_mul(rate.0).map(Rational::floor)
    }

    /// The frame boundary nearest to this time (halves round up).
    pub fn frame_round(self, rate: FrameRate) -> Option<i64> {
        self.0.checked_mul(rate.0).map(Rational::round)
    }

    /// This time moved back to the start of its frame at `rate`.
    pub fn snap_to_frame(self, rate: FrameRate) -> Option<Self> {
        Self::from_frames(self.frame_floor(rate)?, rate)
    }

    /// `self + rhs`, or `None` when out of range.
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    /// `self - rhs`, or `None` when out of range.
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// `self · factor`, e.g. a source duration from a timeline duration and a
    /// speed. `None` when out of range.
    pub fn checked_mul(self, factor: Rational) -> Option<Self> {
        self.0.checked_mul(factor).map(Self)
    }

    /// `self / divisor`, e.g. a timeline duration from a source duration and
    /// a speed. `None` when `divisor` is zero or the result is out of range.
    pub fn checked_div(self, divisor: Rational) -> Option<Self> {
        self.0.checked_div(divisor).map(Self)
    }

    /// `true` when this is exactly zero.
    pub const fn is_zero(self) -> bool {
        self.0.is_zero()
    }

    /// `true` when this is strictly positive.
    pub const fn is_positive(self) -> bool {
        self.0.is_positive()
    }

    /// `true` when this is strictly negative.
    pub const fn is_negative(self) -> bool {
        self.0.is_negative()
    }

    /// Seconds as `f64`. Lossy; for the UI and render edges only.
    pub fn as_secs_f64(self) -> f64 {
        self.0.to_f64()
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

/// Frames per second as an exact, strictly positive rational, e.g. NTSC
/// `30000/1001` instead of `29.97`.
///
/// Serialised like [`Rational`]: `{ "num": 30000, "den": 1001 }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "Rational"))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct FrameRate(Rational);

impl FrameRate {
    /// 24 fps (film).
    pub const FPS_24: Self = Self(Rational::from_canonical(24, 1));
    /// 25 fps (PAL).
    pub const FPS_25: Self = Self(Rational::from_canonical(25, 1));
    /// 30 fps.
    pub const FPS_30: Self = Self(Rational::from_canonical(30, 1));
    /// 50 fps.
    pub const FPS_50: Self = Self(Rational::from_canonical(50, 1));
    /// 60 fps.
    pub const FPS_60: Self = Self(Rational::from_canonical(60, 1));
    /// 23.976 fps: 24000/1001.
    pub const NTSC_23_976: Self = Self(Rational::from_canonical(24_000, 1001));
    /// 29.97 fps: 30000/1001.
    pub const NTSC_29_97: Self = Self(Rational::from_canonical(30_000, 1001));
    /// 59.94 fps: 60000/1001.
    pub const NTSC_59_94: Self = Self(Rational::from_canonical(60_000, 1001));

    /// `num / den` frames per second.
    ///
    /// # Errors
    /// As [`Rational::new`], plus [`RationalError::NotPositive`] for zero or
    /// negative rates.
    pub fn new(num: i64, den: u32) -> Result<Self, RationalError> {
        Self::try_from(Rational::new(num, den)?)
    }

    /// Frames per second as a rational.
    pub const fn as_rational(self) -> Rational {
        self.0
    }

    /// How long one frame lasts: `1 / rate` seconds.
    pub fn frame_duration(self) -> Option<Time> {
        self.0.recip().map(Time)
    }

    /// Frames per second as `f64`. Lossy; for display only.
    pub fn as_f64(self) -> f64 {
        self.0.to_f64()
    }
}

impl TryFrom<Rational> for FrameRate {
    type Error = RationalError;

    fn try_from(rate: Rational) -> Result<Self, Self::Error> {
        if rate.is_positive() {
            Ok(Self(rate))
        } else {
            Err(RationalError::NotPositive)
        }
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for FrameRate {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FrameRate".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        "kinora_core::FrameRate".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Frames per second as an exact, positive rational num/den, e.g. 30000/1001.",
            "type": "object",
            "properties": {
                "num": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": crate::MAX_SAFE_INTEGER
                },
                "den": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": u32::MAX
                }
            },
            "required": ["num", "den"],
            "additionalProperties": false
        })
    }
}

impl fmt::Display for FrameRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} fps", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(num: i64, den: u32) -> Time {
        Time::new(num, den).expect("valid time")
    }

    #[test]
    fn constants_are_canonical() {
        for rate in [
            FrameRate::FPS_24,
            FrameRate::FPS_25,
            FrameRate::FPS_30,
            FrameRate::FPS_50,
            FrameRate::FPS_60,
            FrameRate::NTSC_23_976,
            FrameRate::NTSC_29_97,
            FrameRate::NTSC_59_94,
        ] {
            let r = rate.as_rational();
            assert_eq!(Rational::new(r.num(), r.den()), Ok(r));
        }
    }

    #[test]
    fn ntsc_frames_are_exact() {
        let rate = FrameRate::NTSC_29_97;
        // One hour of NTSC video: 107892 frames + a fraction; no drift.
        let hour = Time::from_secs(3600).expect("in range");
        assert_eq!(hour.frame_floor(rate), Some(107_892));
        let frame = Time::from_frames(107_892, rate).expect("in range");
        assert_eq!(frame, t(107_892 * 1001, 30_000));
        assert!(frame <= hour);
        assert_eq!(rate.frame_duration(), Some(t(1001, 30_000)));
    }

    #[test]
    fn frame_boundaries_belong_to_the_later_frame() {
        let rate = FrameRate::FPS_25;
        assert_eq!(t(1, 25).frame_floor(rate), Some(1));
        assert_eq!(t(1, 26).frame_floor(rate), Some(0));
        assert_eq!(t(-1, 50).frame_floor(rate), Some(-1));
        assert_eq!(t(1, 50).frame_round(rate), Some(1));
        assert_eq!(t(3, 100).snap_to_frame(rate), Some(Time::ZERO));
        assert_eq!(t(5, 100).snap_to_frame(rate), Some(t(1, 25)));
    }

    #[test]
    fn speed_scales_exactly() {
        // A 10 s source played at 3/2 speed lasts 20/3 s on the timeline.
        let source = Time::from_secs(10).expect("in range");
        let speed = Rational::new(3, 2).expect("valid");
        let timeline = source.checked_div(speed).expect("in range");
        assert_eq!(timeline, t(20, 3));
        assert_eq!(timeline.checked_mul(speed), Some(source));
    }

    #[test]
    fn rejects_non_positive_rates() {
        assert_eq!(FrameRate::new(0, 1), Err(RationalError::NotPositive));
        assert_eq!(FrameRate::new(-30, 1), Err(RationalError::NotPositive));
        assert_eq!(FrameRate::new(30, 0), Err(RationalError::ZeroDenominator));
    }

    #[test]
    fn display() {
        assert_eq!(t(1001, 30_000).to_string(), "1001/30000s");
        assert_eq!(FrameRate::NTSC_29_97.to_string(), "30000/1001 fps");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serialises_as_a_bare_rational() {
        let json = serde_json::to_string(&t(1, 2)).expect("serialise");
        assert_eq!(json, r#"{"num":1,"den":2}"#);
        let rate: FrameRate = serde_json::from_str(r#"{"num":60000,"den":2002}"#).expect("parse");
        assert_eq!(rate, FrameRate::NTSC_29_97);
        assert!(serde_json::from_str::<FrameRate>(r#"{"num":0,"den":1}"#).is_err());
        assert!(serde_json::from_str::<FrameRate>(r#"{"num":-24,"den":1}"#).is_err());
    }
}
