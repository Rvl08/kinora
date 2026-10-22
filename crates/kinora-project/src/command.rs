//! The edit vocabulary: every change to a document is an [`EditCommand`]
//! (ADR-0004 §3).
//!
//! The UI, the Copilot, the local API and scripts all produce these values,
//! the journal (`journal.jsonl`) stores them, and undo applies their inverses.
//! Commands carry every identifier they create (e.g. the right half of a
//! split), so replaying a journal is deterministic.
//!
//! [`crate::edit`] validates and applies them and records how to undo them.
//! Every time a command carries must lie on the edit grid (ADR-0004
//! amendment 2); the UI snaps positions to timeline frames before sending.

use kinora_core::Time;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{ClipId, EffectInstanceId, MarkerId, MediaId, TimelineId, TrackId, TransitionId};
use crate::media::MediaAsset;
use crate::model::{
    AudioMix, BlendMode, Clip, Crop, EffectInstance, Marker, Speed, Track, Transform, Transition,
};
use crate::text::{CaptionContent, TextContent};
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
    /// Changes a track's name and switches; `null` leaves a field as it is.
    /// Allowed on locked tracks, so they can be unlocked.
    UpdateTrack {
        /// The track.
        track: TrackId,
        /// New display name.
        name: Option<String>,
        /// Silences the track's audio.
        muted: Option<bool>,
        /// Hides the track's pictures.
        hidden: Option<bool>,
        /// Protects the track from edits.
        locked: Option<bool>,
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
    /// Changes a media clip's speed, keeping its start, its source in-point
    /// and the length of source it plays; the timeline duration follows.
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
    /// Sets or clears a clip's display name.
    RenameClip {
        /// The clip.
        clip: ClipId,
        /// The new name; `null` shows the content's name.
        name: Option<String>,
    },
    /// Enables or disables a clip.
    SetClipEnabled {
        /// The clip.
        clip: ClipId,
        /// `false` keeps the clip but neither draws nor plays it.
        enabled: bool,
    },
    /// Replaces a clip's placement on the canvas.
    SetTransform {
        /// The clip; must have picture properties.
        clip: ClipId,
        /// The new transform.
        transform: Transform,
    },
    /// Replaces a clip's crop.
    SetCrop {
        /// The clip; must have picture properties.
        clip: ClipId,
        /// The new crop.
        crop: Crop,
    },
    /// Replaces a clip's opacity, constant or keyframed.
    SetOpacity {
        /// The clip; must have picture properties.
        clip: ClipId,
        /// The new opacity.
        opacity: Animated<f64>,
    },
    /// Changes how a clip combines with the layers below.
    SetBlendMode {
        /// The clip; must have picture properties.
        clip: ClipId,
        /// The new blend mode.
        blend_mode: BlendMode,
    },
    /// Replaces a clip's gain and fades.
    SetAudioMix {
        /// The clip; on an audio track, or video media on a video track.
        clip: ClipId,
        /// The new sound properties.
        audio: AudioMix,
    },
    /// Replaces the text and style of a title.
    SetText {
        /// The clip; must be a text clip.
        clip: ClipId,
        /// The new content.
        content: TextContent,
    },
    /// Replaces the text, words and style of a caption cue.
    SetCaption {
        /// The clip; must be a caption clip.
        clip: ClipId,
        /// The new content.
        content: CaptionContent,
    },
    /// Adds a transition across the cut between two adjacent clips.
    AddTransition {
        /// The track holding both clips.
        track: TrackId,
        /// The transition, with a fresh id.
        transition: Transition,
    },
    /// Removes a transition.
    RemoveTransition {
        /// The transition.
        transition: TransitionId,
    },
    /// Adds a marker to a timeline, keeping markers ordered by start.
    AddMarker {
        /// The timeline.
        timeline: TimelineId,
        /// The marker, with a fresh id.
        marker: Marker,
    },
    /// Removes a marker.
    RemoveMarker {
        /// The marker.
        marker: MarkerId,
    },
}

impl EditCommand {
    /// The wire name of the operation, e.g. `"splitClip"`.
    pub const fn op_name(&self) -> &'static str {
        match self {
            Self::Batch { .. } => "batch",
            Self::ImportMedia { .. } => "importMedia",
            Self::RemoveMedia { .. } => "removeMedia",
            Self::AddTrack { .. } => "addTrack",
            Self::RemoveTrack { .. } => "removeTrack",
            Self::UpdateTrack { .. } => "updateTrack",
            Self::InsertClip { .. } => "insertClip",
            Self::RemoveClip { .. } => "removeClip",
            Self::MoveClip { .. } => "moveClip",
            Self::TrimClip { .. } => "trimClip",
            Self::SplitClip { .. } => "splitClip",
            Self::SetSpeed { .. } => "setSpeed",
            Self::AddEffect { .. } => "addEffect",
            Self::RemoveEffect { .. } => "removeEffect",
            Self::SetEffectParam { .. } => "setEffectParam",
            Self::RenameClip { .. } => "renameClip",
            Self::SetClipEnabled { .. } => "setClipEnabled",
            Self::SetTransform { .. } => "setTransform",
            Self::SetCrop { .. } => "setCrop",
            Self::SetOpacity { .. } => "setOpacity",
            Self::SetBlendMode { .. } => "setBlendMode",
            Self::SetAudioMix { .. } => "setAudioMix",
            Self::SetText { .. } => "setText",
            Self::SetCaption { .. } => "setCaption",
            Self::AddTransition { .. } => "addTransition",
            Self::RemoveTransition { .. } => "removeTransition",
            Self::AddMarker { .. } => "addMarker",
            Self::RemoveMarker { .. } => "removeMarker",
        }
    }

    /// The undo-history label: a batch's own label, otherwise the operation
    /// name, which the UI translates.
    pub fn label(&self) -> &str {
        match self {
            Self::Batch { label, .. } => label,
            other => other.op_name(),
        }
    }
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
    fn op_names_match_the_wire_tag() {
        let commands = [
            EditCommand::RemoveMedia {
                media: MediaId::generate(),
            },
            EditCommand::SetClipEnabled {
                clip: ClipId::generate(),
                enabled: false,
            },
            EditCommand::RemoveMarker {
                marker: MarkerId::generate(),
            },
            EditCommand::Batch {
                label: "Tidy up".into(),
                commands: Vec::new(),
            },
        ];
        for command in commands {
            let value = serde_json::to_value(&command).expect("serialise");
            assert_eq!(value["op"], command.op_name());
        }
        let batch = EditCommand::Batch {
            label: "Tidy up".into(),
            commands: Vec::new(),
        };
        assert_eq!(batch.label(), "Tidy up");
        let rename = EditCommand::RenameClip {
            clip: ClipId::generate(),
            name: None,
        };
        assert_eq!(rename.label(), "renameClip");
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
