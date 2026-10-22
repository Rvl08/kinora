//! Property tests: values survive a save and reload exactly.

use kinora_core::{Rational, Time};
use kinora_project::id::ClipId;
use kinora_project::value::{Animated, Easing, Keyframe, ParamValue};
use kinora_project::{EditCommand, from_json, to_json};
use proptest::prelude::*;

const SAMPLE: &str = include_str!("fixtures/v0/sample.kinora/project.json");

/// Timebases on the edit grid (ADR-0004 amendment 2): whole seconds,
/// PAL/film/NTSC frames, audio sample rates, MPEG-TS ticks. Off-grid times
/// are rejected by the validator; `document.rs` covers that.
const TIMEBASES: &[u32] = &[
    1, 2, 10, 24, 25, 30, 50, 60, 1000, 24_000, 30_000, 44_100, 48_000, 60_000, 90_000,
];

fn time() -> impl Strategy<Value = Time> {
    // Up to 5·10^6 s each, so sums stay within the grid's ±147 days.
    (0_i64..=5_000_000, prop::sample::select(TIMEBASES))
        .prop_map(|(num, den)| Time::new(num, den).expect("in range by construction"))
}

fn finite_f64() -> impl Strategy<Value = f64> {
    any::<f64>().prop_filter("finite", |v| v.is_finite())
}

proptest! {
    /// Every finite `f64`, including subnormals and -0.0, reads back bit for
    /// bit (`serde_json/float_roundtrip`).
    #[test]
    fn floats_round_trip_bit_exactly(value in finite_f64()) {
        let json = serde_json::to_string(&ParamValue::Float(value)).expect("serialise");
        let ParamValue::Float(back) = serde_json::from_str(&json).expect("parse") else {
            panic!("variant changed");
        };
        prop_assert_eq!(back.to_bits(), value.to_bits());
    }

    #[test]
    fn keyframes_round_trip(times in prop::collection::vec(time(), 1..8), values in prop::collection::vec(finite_f64(), 8)) {
        let animated = Animated::Keyframes {
            keyframes: times
                .iter()
                .zip(&values)
                .map(|(&at, &value)| Keyframe { at, value, easing: Easing::EaseInOut })
                .collect(),
        };
        let json = serde_json::to_string(&animated).expect("serialise");
        let back: Animated<f64> = serde_json::from_str(&json).expect("parse");
        prop_assert_eq!(back, animated);
    }

    #[test]
    fn edit_commands_round_trip(start in time(), at in time()) {
        let command = EditCommand::Batch {
            label: "Move and split".into(),
            commands: vec![
                EditCommand::MoveClip { clip: ClipId::generate(), track: "0192f1c4-0000-7000-8000-000000000301".parse().expect("uuid"), start },
                EditCommand::SplitClip { clip: ClipId::generate(), at, new_clip: ClipId::generate() },
            ],
        };
        let json = serde_json::to_string(&command).expect("serialise");
        prop_assert_eq!(serde_json::from_str::<EditCommand>(&json).expect("parse"), command);
    }

    /// Moving and resizing the last clip of a track to any valid place gives
    /// a document that saves and reloads unchanged.
    #[test]
    fn documents_with_arbitrary_times_round_trip(start in time(), duration in time()) {
        let mut project = from_json(SAMPLE).expect("the sample is valid");
        let clip = project.timelines[0].tracks[0].clips.last_mut().expect("a clip");
        let earliest = Time::from_secs(9).expect("in range");
        clip.start = earliest.checked_add(start).expect("in range");
        clip.duration = duration.checked_add(Time::from_rational(Rational::new(1, 1000).expect("valid"))).expect("in range");
        let json = to_json(&project).expect("valid document");
        prop_assert_eq!(from_json(&json).expect("reads back"), project);
    }
}
