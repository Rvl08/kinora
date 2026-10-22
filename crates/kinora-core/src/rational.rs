//! Exact rational numbers with a JSON-safe range.

use std::cmp::Ordering;
use std::fmt;

/// Largest integer a JavaScript `number` holds exactly: 2^53 − 1.
///
/// Every [`Rational`] numerator stays within `±MAX_SAFE_INTEGER`, so the UI
/// reads document values with plain `JSON.parse` and no precision loss. A
/// numerator this large is about 285 years at 1/1,000,000 s resolution.
pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

/// An exact rational number `num / den`, the representation of time in the
/// document model (ADR-0004).
///
/// Invariants, upheld by every constructor and operation:
/// - `den > 0`;
/// - the fraction is in lowest terms, so equal values have equal fields and
///   the derived `Eq` and `Hash` agree with numeric equality;
/// - `|num| <= MAX_SAFE_INTEGER`.
///
/// Arithmetic is checked: an operation whose exact result breaks the
/// invariants returns `None` instead of rounding or wrapping.
///
/// Serialised as `{ "num": <integer>, "den": <integer> }`. Reading accepts any
/// equivalent fraction (`2/4`) and normalises it; writing is always canonical.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "RationalParts"))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Rational {
    /// Numerator, within ±(2^53 − 1).
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    num: i64,
    /// Denominator, at least 1.
    den: u32,
}

/// Why a [`Rational`] could not be constructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RationalError {
    /// The denominator was zero.
    ZeroDenominator,
    /// The reduced numerator exceeds ±[`MAX_SAFE_INTEGER`] or the reduced
    /// denominator exceeds `u32::MAX`.
    OutOfRange,
    /// A strictly positive value (a frame rate, a speed) was required.
    NotPositive,
}

impl fmt::Display for RationalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ZeroDenominator => "denominator is zero",
            Self::OutOfRange => "numerator or denominator is out of range",
            Self::NotPositive => "value must be greater than zero",
        })
    }
}

impl std::error::Error for RationalError {}

impl Rational {
    /// Zero.
    pub const ZERO: Self = Self { num: 0, den: 1 };
    /// One.
    pub const ONE: Self = Self { num: 1, den: 1 };

    /// Builds `num / den` in lowest terms.
    ///
    /// # Errors
    /// [`RationalError::ZeroDenominator`] when `den == 0`, and
    /// [`RationalError::OutOfRange`] when the reduced numerator exceeds
    /// ±[`MAX_SAFE_INTEGER`].
    pub fn new(num: i64, den: u32) -> Result<Self, RationalError> {
        if den == 0 {
            return Err(RationalError::ZeroDenominator);
        }
        Self::reduce(i128::from(num), i128::from(den)).ok_or(RationalError::OutOfRange)
    }

    /// Builds the integer `value`.
    ///
    /// # Errors
    /// [`RationalError::OutOfRange`] when `|value| > MAX_SAFE_INTEGER`.
    pub fn from_integer(value: i64) -> Result<Self, RationalError> {
        Self::new(value, 1)
    }

    /// Builds a value from fields that are already canonical. Only for
    /// constants; `debug_assert`s the invariants.
    pub(crate) const fn from_canonical(num: i64, den: u32) -> Self {
        debug_assert!(den > 0 && num.abs() <= MAX_SAFE_INTEGER);
        Self { num, den }
    }

    /// Reduces `num / den` (any signs, `den != 0`) and range-checks the result.
    fn reduce(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let sign = if (num < 0) != (den < 0) { -1 } else { 1 };
        let (n, d) = (num.unsigned_abs(), den.unsigned_abs());
        let g = gcd(n, d);
        let (n, d) = (n / g, d / g);
        let num = i64::try_from(n).ok()?;
        if num > MAX_SAFE_INTEGER {
            return None;
        }
        Some(Self {
            num: sign * num,
            den: u32::try_from(d).ok()?,
        })
    }

    /// The numerator (sign-carrying, in lowest terms).
    pub const fn num(self) -> i64 {
        self.num
    }

    /// The denominator (positive, in lowest terms).
    pub const fn den(self) -> u32 {
        self.den
    }

    /// `true` when the value is zero.
    pub const fn is_zero(self) -> bool {
        self.num == 0
    }

    /// `true` when the value is strictly greater than zero.
    pub const fn is_positive(self) -> bool {
        self.num > 0
    }

    /// `true` when the value is strictly less than zero.
    pub const fn is_negative(self) -> bool {
        self.num < 0
    }

    /// `self + rhs`, or `None` when the exact result is out of range.
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let (a, b, c, d) = self.wide(rhs);
        Self::reduce(a * d + c * b, b * d)
    }

    /// `self - rhs`, or `None` when the exact result is out of range.
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        let (a, b, c, d) = self.wide(rhs);
        Self::reduce(a * d - c * b, b * d)
    }

    /// `self * rhs`, or `None` when the exact result is out of range.
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let (a, b, c, d) = self.wide(rhs);
        Self::reduce(a * c, b * d)
    }

    /// `self / rhs`, or `None` when `rhs` is zero or the exact result is out
    /// of range.
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        let (a, b, c, d) = self.wide(rhs);
        Self::reduce(a * d, b * c)
    }

    /// `-self`. Never fails: the numerator range is symmetric.
    pub const fn neg(self) -> Self {
        Self {
            num: -self.num,
            den: self.den,
        }
    }

    /// `1 / self`, or `None` for zero or when the numerator does not fit the
    /// denominator's `u32` range.
    pub fn recip(self) -> Option<Self> {
        Self::reduce(i128::from(self.den), i128::from(self.num))
    }

    /// The largest integer `<= self`.
    pub const fn floor(self) -> i64 {
        self.num.div_euclid(self.den as i64)
    }

    /// The smallest integer `>= self`.
    pub const fn ceil(self) -> i64 {
        let den = self.den as i64;
        let floor = self.num.div_euclid(den);
        if self.num.rem_euclid(den) == 0 {
            floor
        } else {
            floor + 1
        }
    }

    /// The nearest integer; halves round towards positive infinity, so the
    /// result never depends on the sign convention of the caller.
    pub const fn round(self) -> i64 {
        // floor(x + 1/2) = floor((2·num + den) / (2·den)), computed without
        // overflow: |num| <= 2^53 and den < 2^32.
        let num = 2 * self.num + self.den as i64;
        num.div_euclid(2 * self.den as i64)
    }

    /// The closest `f64`. Lossy; for the UI and render edges only.
    #[allow(clippy::cast_precision_loss)] // |num| <= 2^53 is exact; the division rounds once.
    pub fn to_f64(self) -> f64 {
        self.num as f64 / f64::from(self.den)
    }

    /// Both operands widened to `i128` as `(num_l, den_l, num_r, den_r)`.
    /// Products of two of them stay below 2^107, so no step can overflow.
    fn wide(self, rhs: Self) -> (i128, i128, i128, i128) {
        (
            i128::from(self.num),
            i128::from(self.den),
            i128::from(rhs.num),
            i128::from(rhs.den),
        )
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    // gcd(0, 0) only arises for 0/0, which `reduce` rejects earlier.
    a.max(1)
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        // Denominators are positive, so cross-multiplying keeps the order.
        let (a, b, c, d) = self.wide(*other);
        (a * d).cmp(&(c * b))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Default for Rational {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

impl TryFrom<i64> for Rational {
    type Error = RationalError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::from_integer(value)
    }
}

/// The wire shape, before normalisation and range checks.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RationalParts {
    num: i64,
    den: u32,
}

#[cfg(feature = "serde")]
impl TryFrom<RationalParts> for Rational {
    type Error = RationalError;

    fn try_from(parts: RationalParts) -> Result<Self, Self::Error> {
        Self::new(parts.num, parts.den)
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for Rational {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Rational".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        "kinora_core::Rational".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Exact rational number num/den (ADR-0004). Readers normalise to lowest terms.",
            "type": "object",
            "properties": {
                "num": {
                    "description": "Numerator, within ±(2^53 − 1) so JavaScript reads it exactly.",
                    "type": "integer",
                    "minimum": -MAX_SAFE_INTEGER,
                    "maximum": MAX_SAFE_INTEGER
                },
                "den": {
                    "description": "Denominator, at least 1.",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn r(num: i64, den: u32) -> Rational {
        Rational::new(num, den).expect("valid rational")
    }

    #[test]
    fn normalises_to_lowest_terms() {
        assert_eq!(r(2, 4), r(1, 2));
        assert_eq!(r(-6, 4).num(), -3);
        assert_eq!(r(-6, 4).den(), 2);
        assert_eq!(r(0, 7), Rational::ZERO);
    }

    #[test]
    fn rejects_zero_denominator_and_unsafe_numerators() {
        assert_eq!(Rational::new(1, 0), Err(RationalError::ZeroDenominator));
        assert_eq!(
            Rational::new(MAX_SAFE_INTEGER + 1, 1),
            Err(RationalError::OutOfRange)
        );
        // Out of range unreduced, in range once reduced.
        assert_eq!(r(MAX_SAFE_INTEGER + 1, 2).num(), (MAX_SAFE_INTEGER + 1) / 2);
        assert_eq!(Rational::new(i64::MIN, 1), Err(RationalError::OutOfRange));
    }

    #[test]
    fn arithmetic_is_exact() {
        let ntsc_frame = r(1001, 30000);
        let three = ntsc_frame
            .checked_add(ntsc_frame)
            .and_then(|x| x.checked_add(ntsc_frame))
            .expect("in range");
        assert_eq!(three, r(3003, 30000));
        assert_eq!(three.checked_div(ntsc_frame), Some(r(3, 1)));
        assert_eq!(r(1, 3).checked_mul(r(3, 1)), Some(Rational::ONE));
        assert_eq!(r(1, 2).checked_sub(r(3, 4)), Some(r(-1, 4)));
        assert_eq!(r(1, 2).checked_div(Rational::ZERO), None);
    }

    #[test]
    fn overflow_returns_none() {
        let big = r(MAX_SAFE_INTEGER, 1);
        assert_eq!(big.checked_add(Rational::ONE), None);
        assert_eq!(big.checked_mul(r(2, 1)), None);
        // The denominator would exceed u32::MAX.
        assert_eq!(r(1, u32::MAX).checked_mul(r(1, 2)), None);
        assert_eq!(r(i64::from(u32::MAX) + 1, 1).recip(), None);
    }

    #[test]
    fn rounding() {
        assert_eq!(r(7, 2).floor(), 3);
        assert_eq!(r(-7, 2).floor(), -4);
        assert_eq!(r(7, 2).ceil(), 4);
        assert_eq!(r(-7, 2).ceil(), -3);
        assert_eq!(r(6, 2).ceil(), 3);
        assert_eq!(r(7, 2).round(), 4);
        assert_eq!(r(-7, 2).round(), -3);
        assert_eq!(r(-8, 3).round(), -3);
    }

    #[test]
    fn ordering_cross_multiplies() {
        assert!(r(1, 3) < r(1, 2));
        assert!(r(-1, 2) < r(-1, 3));
        assert!(r(1001, 30000) < r(1, 29));
        assert_eq!(r(2, 6).cmp(&r(1, 3)), Ordering::Equal);
    }

    #[test]
    fn formats() {
        assert_eq!(r(30000, 1001).to_string(), "30000/1001");
        assert_eq!(r(4, 2).to_string(), "2");
        assert_eq!(format!("{:?}", r(4, 2)), "2/1");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json_shape_and_validation() {
        let json = serde_json::to_string(&r(-1001, 30000)).expect("serialise");
        assert_eq!(json, r#"{"num":-1001,"den":30000}"#);

        let parsed: Rational = serde_json::from_str(r#"{"num":2,"den":4}"#).expect("parse");
        assert_eq!(parsed, r(1, 2));

        for bad in [
            r#"{"num":1,"den":0}"#,
            r#"{"num":1,"den":-1}"#,
            r#"{"num":1.5,"den":2}"#,
            r#"{"num":9007199254740992,"den":1}"#,
            r#"{"num":1,"den":2,"extra":0}"#,
            r#"{"num":1}"#,
            r#""1/2""#,
        ] {
            assert!(
                serde_json::from_str::<Rational>(bad).is_err(),
                "accepted {bad}"
            );
        }
    }
}
