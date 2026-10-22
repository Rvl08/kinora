//! The edit vocabulary: every change to a document is an [`EditCommand`]
//! (ADR-0004 §3).
//!
//! The UI, the Copilot, the local API and scripts all produce these values,
//! the journal (`journal.jsonl`) stores them, and undo applies their inverses.
//! Commands carry every identifier they create (e.g. the right half of a
//! split), so replaying a journal is deterministic.
//!
//! **Phase 0 draft:** this module fixes the wire shape (`{ "op", "params" }`)
//! and a first set of operations. `validate → apply → inverse` arrives with
//! the editing core in Phase 1, together with the remaining operations.

use kinora_core::Time;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{ClipId, EffectInstanceId, MediaId, TimelineId, TrackId};
use crate::media::MediaAsset;
use crate::model::{Clip, EffectInstance, Speed, Track};
use crate::value::{Animated, ParamValue};

/// One change to a document, serialised as `{ "op": "<name>", "params": { … } }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "op",
    content = "params",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum EditCommand {
    /// Several commands applied atomically as one undo step (e.g. "remove
    /// filler words" = dozens of splits and ripple deletes).
    Batch {
        /// Undo history label, e.g. "Remove filler words".
        label: String,
        /// The commands, applied in order.
        commands: Vec<EditCommand>,
    },
    /// Adds an asset to the media pool.
    ImportMedia {
        /// The asset, with a fresh id.
        asset: Box<MediaAsset>,
    },
    /// Removes an asset that no clip uses.
    RemoveMedia {
        /// The asset.
        media: MediaId,
    },
    /// Inserts a track into a timeline.
    AddTrack {
        /// The timeline.
        timeline: TimelineId,
        /// Position in compositing order; `0` is the bottom.
        index: u32,
        /// The track, with a fresh id.
        track: Track,
    },
    /// Removes a track and its clips.
    RemoveTrack {
        /// The track.
        track: TrackId,
    },
    /// Places a clip on a track where there is room.
    InsertClip {
        /// The track.
        track: TrackId,
        /// The clip, with a fresh id.
        clip: Box<Clip>,
    },
    /// Removes a clip, optionally closing the gap.
    RemoveClip {
        /// The clip.
        clip: ClipId,
        /// Shifts later clips on the track left by the clip's duration.
        ripple: bool,
    },
    /// Moves a clip in time and/or to another track of the same kind.
    MoveClip {
        /// The clip.
        clip: ClipId,
        /// Destination track (may be the current one).
        track: TrackId,
        /// New start.
        start: Time,
    },
    /// Moves one edge of a clip; trimming the start also moves the source
    /// in-point so the content stays in place.
    TrimClip {
        /// The clip.
        clip: ClipId,
        /// Which edge.
        edge: ClipEdge,
        /// New timeline position of that edge.
        to: Time,
    },
    /// Cuts a clip in two at a timeline position strictly inside it.
    SplitClip {
        /// The clip; keeps the left part.
        clip: ClipId,
        /// Where to cut.
        at: Time,
        /// Identifier for the new right part.
        new_clip: ClipId,
    },
    /// Changes a media clip's speed, keeping its start and source in-point.
    SetSpeed {
        /// The clip.
        clip: ClipId,
        /// The new speed.
        speed: Speed,
    },
    /// Inserts an effect into a clip's effect stack.
    AddEffect {
        /// The clip.
        clip: ClipId,
        /// Position in the stack; effects apply in order.
        index: u32,
        /// The effect, with a fresh id.
        effect: EffectInstance,
    },
    /// Removes an effect from a clip.
    RemoveEffect {
        /// The clip.
        clip: ClipId,
        /// The effect.
        effect: EffectInstanceId,
    },
    /// Sets one effect parameter, constant or keyframed.
    SetEffectParam {
        /// The clip.
        clip: ClipId,
        /// The effect.
        effect: EffectInstanceId,
        /// Parameter name.
        param: String,
        /// The new value.
        value: Animated<ParamValue>,
    },
}

/// An edge of a clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum ClipEdge {
    /// The start (in-point).
    Start,
    /// The end (out-point).
    End,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn wire_shape_is_op_and_params() {
        let clip = ClipId::generate();
        let right = ClipId::generate();
        let command = EditCommand::Batch {
            label: "Split".into(),
            commands: vec![EditCommand::SplitClip {
                clip,
                at: Time::from_secs(2).expect("in range"),
                new_clip: right,
            }],
        };
        let value = serde_json::to_value(&command).expect("serialise");
        assert_eq!(
            value,
            json!({
                "op": "batch",
                "params": {
                    "label": "Split",
                    "commands": [{
                        "op": "splitClip",
                        "params": {
                            "clip": clip.to_string(),
                            "at": { "num": 2, "den": 1 },
                            "newClip": right.to_string()
                        }
                    }]
                }
            })
        );
        assert_eq!(
            serde_json::from_value::<EditCommand>(value).expect("parse"),
            command
        );
    }

    #[test]
    fn rejects_unknown_ops_and_fields() {
        let clip = ClipId::generate().to_string();
        for bad in [
            json!({ "op": "explode", "params": {} }),
            json!({ "op": "removeClip", "params": { "clip": clip, "ripple": true, "force": true } }),
            json!({ "op": "removeClip", "params": { "clip": clip } }),
            json!({ "op": "removeClip", "params": { "clip": clip, "ripple": true }, "extra": 1 }),
        ] {
            assert!(
                serde_json::from_value::<EditCommand>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }
}
