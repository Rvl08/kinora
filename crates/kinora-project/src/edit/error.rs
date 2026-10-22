//! Why an edit was refused.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::validate::Issue;

/// Why [`apply`](super::apply) refused a command. The document is unchanged
/// whenever an error is returned.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// A command names an entity that does not exist.
    #[error("{entity} {id} does not exist")]
    NotFound {
        /// What kind of entity.
        entity: EntityKind,
        /// Its identifier.
        id: Uuid,
    },
    /// The command is well-formed but breaks an editing rule.
    #[error("{rule}: {message}")]
    Rejected {
        /// Which rule, for the UI to explain in its own words.
        rule: EditRule,
        /// Details in English, for logs and developer tools.
        message: String,
    },
    /// The command would leave a document that breaks the format's rules
    /// (the validator's safety net, for example a parameter of the wrong
    /// type inside an inserted clip).
    #[error("the result breaks {} document rule(s); first: {}", .issues.len(), .issues.first().map(ToString::to_string).unwrap_or_default())]
    Invalid {
        /// Every rule the result breaks, with JSON Pointers into the
        /// resulting document.
        issues: Vec<Issue>,
    },
    /// A command inside a batch failed; the whole batch was rolled back.
    #[error("command {index} of the batch: {error}")]
    InBatch {
        /// Position of the failing command in the batch.
        index: usize,
        /// What went wrong there.
        error: Box<EditError>,
    },
    /// An internal invariant does not hold. This is a bug in Kinora, never
    /// the caller's fault; the shell logs it as an incident.
    #[error("internal editing error: {0}")]
    Internal(&'static str),
}

impl EditError {
    pub(crate) fn rejected(rule: EditRule, message: impl Into<String>) -> Self {
        Self::Rejected {
            rule,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(entity: EntityKind, id: Uuid) -> Self {
        Self::NotFound { entity, id }
    }

    /// The innermost error, looking through [`EditError::InBatch`].
    pub fn root(&self) -> &Self {
        match self {
            Self::InBatch { error, .. } => error.root(),
            other => other,
        }
    }

    /// The position of the failing command, as a JSON Pointer into the
    /// command, e.g. `/params/commands/2`; empty for the command itself.
    pub fn command_pointer(&self) -> String {
        let mut pointer = String::new();
        let mut error = self;
        while let Self::InBatch {
            index,
            error: inner,
        } = error
        {
            pointer.push_str(&format!("/params/commands/{index}"));
            error = inner;
        }
        pointer
    }
}

/// Kinds of document entities, for [`EditError::NotFound`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityKind {
    /// A media asset.
    Media,
    /// A timeline.
    Timeline,
    /// A track.
    Track,
    /// A clip.
    Clip,
    /// An effect on a clip.
    Effect,
    /// A transition.
    Transition,
    /// A marker.
    Marker,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Media => "media",
            Self::Timeline => "timeline",
            Self::Track => "track",
            Self::Clip => "clip",
            Self::Effect => "effect",
            Self::Transition => "transition",
            Self::Marker => "marker",
        })
    }
}

/// The editing rules a command can break. Serialised in camelCase; the UI
/// shows a localised explanation per rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum EditRule {
    /// The track is locked.
    TrackLocked,
    /// The clip would overlap another clip on the track.
    Overlap,
    /// The position is not strictly inside the clip.
    OutsideClip,
    /// The clip would have no duration left.
    EmptyClip,
    /// The clip would play before the start or past the end of its media.
    SourceRange,
    /// The content or the destination track is of the wrong kind.
    TrackKind,
    /// Clips cannot move between timelines.
    OtherTimeline,
    /// An insertion index is past the end of the list.
    Index,
    /// The identifier of a new entity is already used.
    DuplicateId,
    /// The media is still used by a clip or derived asset.
    InUse,
    /// The property does not exist on this clip (no picture, no sound, not
    /// media, not text).
    NotApplicable,
    /// A time is not a whole number of flicks (ADR-0004 amendment 2).
    OffGrid,
    /// A time or count is beyond the supported range.
    OutOfRange,
    /// The two clips of a transition are not adjacent on one track.
    NotAdjacent,
    /// The cut already has a transition.
    TransitionExists,
    /// A transition would be longer than one of its clips.
    TransitionTooLong,
    /// Batches are nested too deeply or hold too many commands.
    BatchTooLarge,
}

impl fmt::Display for EditRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The serde name, so logs and the wire agree.
        let name = match self {
            Self::TrackLocked => "trackLocked",
            Self::Overlap => "overlap",
            Self::OutsideClip => "outsideClip",
            Self::EmptyClip => "emptyClip",
            Self::SourceRange => "sourceRange",
            Self::TrackKind => "trackKind",
            Self::OtherTimeline => "otherTimeline",
            Self::Index => "index",
            Self::DuplicateId => "duplicateId",
            Self::InUse => "inUse",
            Self::NotApplicable => "notApplicable",
            Self::OffGrid => "offGrid",
            Self::OutOfRange => "outOfRange",
            Self::NotAdjacent => "notAdjacent",
            Self::TransitionExists => "transitionExists",
            Self::TransitionTooLong => "transitionTooLong",
            Self::BatchTooLarge => "batchTooLarge",
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_display_matches_the_wire_name() {
        for rule in [
            EditRule::TrackLocked,
            EditRule::OffGrid,
            EditRule::BatchTooLarge,
        ] {
            let wire = serde_json::to_value(rule).expect("serialise");
            assert_eq!(wire, rule.to_string());
        }
    }

    #[test]
    fn batch_errors_point_at_the_failing_command() {
        let inner = EditError::rejected(EditRule::Overlap, "x");
        let error = EditError::InBatch {
            index: 1,
            error: Box::new(EditError::InBatch {
                index: 4,
                error: Box::new(inner.clone()),
            }),
        };
        assert_eq!(error.root(), &inner);
        assert_eq!(
            error.command_pointer(),
            "/params/commands/1/params/commands/4"
        );
        assert_eq!(inner.command_pointer(), "");
    }
}
