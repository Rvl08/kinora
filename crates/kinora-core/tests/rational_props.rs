//! Property tests for exact rational time (ADR-0004).

use kinora_core::{FLICKS_PER_SECOND, FrameRate, MAX_SAFE_INTEGER, Rational, Rounding, Time};
use proptest::prelude::*;

/// Rationals in the range a timeline actually uses: up to ±10^9 over
/// denominators up to 10^6 (sample rates, 90 kHz clocks, NTSC timebases).
fn rational() -> impl Strategy<Value = Rational> {
    (-1_000_000_000_i64..=1_000_000_000, 1_u32..=1_000_000)
        .prop_map(|(num, den)| Rational::new(num, den).expect("in range by construction"))
}

fn nonzero_rational() -> impl Strategy<Value = Rational> {
    rational().prop_filter("non-zero", |r| !r.is_zero())
}

fn frame_rate() -> impl Strategy<Value = FrameRate> {
    prop_oneof![
        Just(FrameRate::FPS_24),
        Just(FrameRate::FPS_25),
        Just(FrameRate::FPS_30),
        Just(FrameRate::FPS_60),
        Just(FrameRate::NTSC_23_976),
        Just(FrameRate::NTSC_29_97),
        Just(FrameRate::NTSC_59_94),
        (1_i64..=1000, 1_u32..=1001).prop_map(|(n, d)| FrameRate::new(n, d).expect("positive")),
    ]
}

/// Grid times up to about four hours, the length of a long edit.
fn grid_time() -> impl Strategy<Value = Time> {
    (-10_000_000_000_000_i64..=10_000_000_000_000)
        .prop_map(|flicks| Time::from_flicks(flicks).expect("in range by construction"))
}

/// Speeds an editor offers: 1/100× to 100×, any denominator up to 1000.
fn speed() -> impl Strategy<Value = Rational> {
    (1_i64..=100_000, 1_u32..=1000)
        .prop_map(|(n, d)| Rational::new(n, d).expect("valid"))
        .prop_filter("1/100..=100", |r| {
            *r >= Rational::new(1, 100).expect("valid")
                && *r <= Rational::new(100, 1).expect("valid")
        })
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn is_canonical(r: Rational) -> bool {
    r.den() > 0
        && r.num().abs() <= MAX_SAFE_INTEGER
        && gcd(r.num().unsigned_abs(), u64::from(r.den())) == 1
}

proptest! {
    #[test]
    fn construction_is_canonical_and_never_panics(num in any::<i64>(), den in any::<u32>()) {
        match Rational::new(num, den) {
            Ok(r) => {
                prop_assert!(is_canonical(r));
                // Same value: num·r.den == r.num·den.
                prop_assert_eq!(i128::from(num) * i128::from(r.den()), i128::from(r.num()) * i128::from(den));
            }
            Err(_) => prop_assert!(den == 0 || num.unsigned_abs() > MAX_SAFE_INTEGER.unsigned_abs()),
        }
    }

    #[test]
    fn scaling_numerator_and_denominator_gives_the_same_value(r in rational(), k in 1_u32..=1000) {
        let scaled = Rational::new(r.num() * i64::from(k), r.den() * k);
        prop_assert_eq!(scaled, Ok(r));
    }

    #[test]
    fn operations_on_extremes_never_panic(
        a in (any::<i64>(), any::<u32>()),
        b in (any::<i64>(), any::<u32>()),
    ) {
        if let (Ok(a), Ok(b)) = (Rational::new(a.0, a.1), Rational::new(b.0, b.1)) {
            for x in [a.checked_add(b), a.checked_sub(b), a.checked_mul(b), a.checked_div(b), a.recip()]
                .into_iter()
                .flatten()
            {
                prop_assert!(is_canonical(x));
            }
            let _ = (a.floor(), a.ceil(), a.round(), a.to_f64(), a.cmp(&b));
        }
    }

    #[test]
    fn addition_is_commutative_and_associative(a in rational(), b in rational(), c in rational()) {
        prop_assert_eq!(a.checked_add(b), b.checked_add(a));
        let left = a.checked_add(b).and_then(|ab| ab.checked_add(c));
        let right = b.checked_add(c).and_then(|bc| a.checked_add(bc));
        if let (Some(left), Some(right)) = (left, right) {
            prop_assert_eq!(left, right);
        }
    }

    #[test]
    fn subtraction_inverts_addition(a in rational(), b in rational()) {
        if let Some(sum) = a.checked_add(b) {
            prop_assert_eq!(sum.checked_sub(b), Some(a));
        }
    }

    #[test]
    fn division_inverts_multiplication(a in rational(), b in nonzero_rational()) {
        if let Some(product) = a.checked_mul(b) {
            prop_assert_eq!(product.checked_div(b), Some(a));
        }
    }

    #[test]
    fn multiplication_distributes_over_addition(a in rational(), b in rational(), c in rational()) {
        let left = b.checked_add(c).and_then(|bc| a.checked_mul(bc));
        let right = a.checked_mul(b).zip(a.checked_mul(c)).and_then(|(ab, ac)| ab.checked_add(ac));
        if let (Some(left), Some(right)) = (left, right) {
            prop_assert_eq!(left, right);
        }
    }

    #[test]
    fn order_matches_the_sign_of_the_difference(a in rational(), b in rational()) {
        // The difference's denominator may not fit u32; compare exactly instead.
        let cross = i128::from(a.num()) * i128::from(b.den()) - i128::from(b.num()) * i128::from(a.den());
        prop_assert_eq!(a.cmp(&b), cross.cmp(&0));
        if let Some(diff) = a.checked_sub(b) {
            prop_assert_eq!(a.cmp(&b), diff.num().cmp(&0));
        }
    }

    #[test]
    fn order_agrees_with_f64_when_values_are_apart(a in rational(), b in rational()) {
        let (x, y) = (a.to_f64(), b.to_f64());
        if (x - y).abs() > 1e-6 * x.abs().max(y.abs()).max(1.0) {
            prop_assert_eq!(a < b, x < y);
        }
    }

    #[test]
    fn floor_ceil_round_bracket_the_value(a in rational()) {
        let (floor, ceil, round) = (a.floor(), a.ceil(), a.round());
        let at = |n: i64| Rational::from_integer(n).expect("small");
        prop_assert!(at(floor) <= a && a < at(floor + 1));
        prop_assert!(at(ceil - 1) < a && a <= at(ceil));
        prop_assert!(round == floor || round == ceil);
        // |a - round| <= 1/2
        let half = Rational::new(1, 2).expect("valid");
        let distance = a.checked_sub(at(round)).expect("small");
        prop_assert!(distance <= half && distance.neg() <= half);
    }

    #[test]
    fn frames_round_trip(frame in -10_000_000_i64..=10_000_000, rate in frame_rate()) {
        let start = Time::from_frames(frame, rate).expect("in range");
        prop_assert_eq!(start.frame_floor(rate), Some(frame));
        prop_assert_eq!(start.snap_to_frame(rate), Some(start));
    }

    #[test]
    fn frame_floor_finds_the_containing_frame(secs in rational(), rate in frame_rate()) {
        let time = Time::from_rational(secs);
        let frame = time.frame_floor(rate).expect("in range");
        let start = Time::from_frames(frame, rate).expect("in range");
        let next = Time::from_frames(frame + 1, rate).expect("in range");
        prop_assert!(start <= time && time < next);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json_round_trip_is_exact(a in rational()) {
        let json = serde_json::to_string(&a).expect("serialise");
        let back: Rational = serde_json::from_str(&json).expect("parse");
        prop_assert_eq!(back, a);
        let time: Time = serde_json::from_str(&json).expect("parse as time");
        prop_assert_eq!(time.as_rational(), a);
    }

    #[test]
    fn grid_times_add_without_leaving_the_grid(a in grid_time(), b in grid_time()) {
        let sum = a.checked_add(b).expect("grid sums never overflow the denominator");
        prop_assert_eq!(sum.flicks(), Some(a.flicks().expect("on grid") + b.flicks().expect("on grid")));
        let diff = a.checked_sub(b).expect("grid differences never overflow the denominator");
        prop_assert!(diff.is_on_grid());
    }

    #[test]
    fn grid_round_trip(flicks in -MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER) {
        let time = Time::from_flicks(flicks).expect("in range");
        prop_assert_eq!(time.flicks(), Some(flicks));
        prop_assert_eq!(time.round_to_grid(), Some(time));
    }

    #[test]
    fn rounding_to_the_grid_is_within_half_a_flick(secs in rational()) {
        let time = Time::from_rational(secs);
        // The grid spans ±(2^53 − 1) flicks; beyond it there is nothing to
        // round to. Compare in i128 flicks: |secs · F| <= 2^53 − 1 (+ ½).
        let scaled = i128::from(secs.num()) * i128::from(FLICKS_PER_SECOND);
        let limit = (i128::from(MAX_SAFE_INTEGER) * 2 + 1) * i128::from(secs.den());
        let Some(rounded) = time.round_to_grid() else {
            prop_assert!(2 * scaled.abs() >= limit, "in range but not rounded: {}", secs);
            return Ok(());
        };
        prop_assert!(rounded.is_on_grid());
        // |time - rounded| <= 1/2 flick, compared exactly.
        let half_flick = Rational::new(1, 2 * u32::try_from(FLICKS_PER_SECOND).expect("fits")).expect("valid");
        let distance = time.as_rational().checked_sub(rounded.as_rational());
        if let Some(distance) = distance {
            prop_assert!(distance <= half_flick && distance.neg() <= half_flick);
        }
    }

    #[test]
    fn scaling_is_monotonic_and_close(a in grid_time(), b in grid_time(), rate in speed()) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        for rounding in [Rounding::Nearest, Rounding::Down] {
            let slo = lo.scale_to_grid(rate, rounding).expect("in range");
            let shi = hi.scale_to_grid(rate, rounding).expect("in range");
            prop_assert!(slo <= shi, "rounding keeps order");
            // Within one flick of the exact product, and never above it when
            // rounding down.
            let exact = i128::from(hi.flicks().expect("on grid")) * i128::from(rate.num());
            let scaled = i128::from(shi.flicks().expect("on grid")) * i128::from(rate.den());
            prop_assert!((scaled - exact).abs() <= i128::from(rate.den()));
            if rounding == Rounding::Down {
                prop_assert!(scaled <= exact);
            }
        }
    }

    /// The editing core derives a timeline duration from a source length by
    /// rounding down, so mapping it back never plays more than that length.
    #[test]
    fn rounding_down_never_grows_the_source_range(source in grid_time(), rate in speed()) {
        let source = if source.is_negative() { Time::ZERO.checked_sub(source).expect("in range") } else { source };
        let timeline = source.div_to_grid(rate, Rounding::Down).expect("in range");
        let back = timeline.scale_to_grid(rate, Rounding::Nearest).expect("in range");
        prop_assert!(back <= source);
    }

    #[test]
    fn aligned_frames_lie_on_the_grid(frame in -10_000_000_i64..=10_000_000, rate in frame_rate()) {
        let start = Time::from_frames(frame, rate).expect("in range");
        // The grid spans ±(2^53 − 1) flicks, about ±147 days.
        if rate.is_grid_aligned() && start.as_secs_f64().abs() < 1.2e7 {
            prop_assert!(start.is_on_grid());
        }
    }
}
