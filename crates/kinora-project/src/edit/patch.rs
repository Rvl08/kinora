//! Primitive, exactly invertible changes to a document.
//!
//! Every [`EditCommand`](crate::EditCommand) is carried out as a short
//! sequence of [`Op`]s. Applying an op returns its inverse, which captures
//! whatever the op removed or replaced, so undo restores the previous
//! document bit for bit, including data a command dropped on purpose (fades
//! clamped by a trim, a transition removed by a move). Ops address entities
//! by position, which is valid because ops are applied, and undone, strictly
//! in sequence.
//!
//! Ops are not part of the public API. The journal records commands and
//! replays them; only an undo or redo of a step older than the journal
//! carries its patch ([`journal`](super::journal)), in a format versioned by
//! the journal's own format number.

use kinora_core::Time;
use serde::{Deserialize, Serialize};

use crate::media::MediaAsset;
use crate::model::{Clip, Marker, Project, Timeline, Track, Transition};

use super::EditError;

/// Position of a track: `project.timelines[timeline].tracks[track]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackAt {
    pub timeline: usize,
    pub track: usize,
}

/// A track's own properties, without its contents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackHeader {
    pub name: String,
    pub muted: bool,
    pub hidden: bool,
    pub locked: bool,
}

impl TrackHeader {
    pub(crate) fn of(track: &Track) -> Self {
        Self {
            name: track.name.clone(),
            muted: track.muted,
            hidden: track.hidden,
            locked: track.locked,
        }
    }
}

/// One primitive change. Values are boxed so the enum stays small.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Op {
    InsertMedia {
        index: usize,
        asset: Box<MediaAsset>,
    },
    RemoveMedia {
        index: usize,
    },
    InsertTrack {
        timeline: usize,
        index: usize,
        track: Box<Track>,
    },
    RemoveTrack {
        timeline: usize,
        index: usize,
    },
    SetTrackHeader {
        at: TrackAt,
        header: Box<TrackHeader>,
    },
    InsertClip {
        at: TrackAt,
        index: usize,
        clip: Box<Clip>,
    },
    RemoveClip {
        at: TrackAt,
        index: usize,
    },
    ReplaceClip {
        at: TrackAt,
        index: usize,
        clip: Box<Clip>,
    },
    /// Moves `clips[from..]` of a track by `delta` (ripple).
    ShiftClips {
        at: TrackAt,
        from: usize,
        delta: Time,
    },
    InsertTransition {
        at: TrackAt,
        index: usize,
        transition: Box<Transition>,
    },
    RemoveTransition {
        at: TrackAt,
        index: usize,
    },
    ReplaceTransition {
        at: TrackAt,
        index: usize,
        transition: Box<Transition>,
    },
    InsertMarker {
        timeline: usize,
        index: usize,
        marker: Box<Marker>,
    },
    RemoveMarker {
        timeline: usize,
        index: usize,
    },
}

/// A position that does not exist: a bug in the planner, never user input.
const BAD_POSITION: EditError =
    EditError::Internal("an edit addressed a position that does not exist");

fn timeline(project: &mut Project, index: usize) -> Result<&mut Timeline, EditError> {
    project.timelines.get_mut(index).ok_or(BAD_POSITION)
}

fn track(project: &mut Project, at: TrackAt) -> Result<&mut Track, EditError> {
    timeline(project, at.timeline)?
        .tracks
        .get_mut(at.track)
        .ok_or(BAD_POSITION)
}

/// `Vec::insert` that refuses an index past the end instead of panicking.
fn insert<T>(items: &mut Vec<T>, index: usize, item: T) -> Result<(), EditError> {
    if index > items.len() {
        return Err(BAD_POSITION);
    }
    items.insert(index, item);
    Ok(())
}

/// `Vec::remove` that refuses a missing index instead of panicking.
fn remove<T>(items: &mut Vec<T>, index: usize) -> Result<T, EditError> {
    if index >= items.len() {
        return Err(BAD_POSITION);
    }
    Ok(items.remove(index))
}

/// Swaps `item` into `items[index]` and returns the previous value.
fn replace<T>(items: &mut [T], index: usize, item: T) -> Result<T, EditError> {
    let slot = items.get_mut(index).ok_or(BAD_POSITION)?;
    Ok(std::mem::replace(slot, item))
}

impl Op {
    /// Applies the op and returns its inverse. On error the document is
    /// unchanged.
    pub(crate) fn apply(self, project: &mut Project) -> Result<Op, EditError> {
        Ok(match self {
            Op::InsertMedia { index, asset } => {
                insert(&mut project.media, index, *asset)?;
                Op::RemoveMedia { index }
            }
            Op::RemoveMedia { index } => Op::InsertMedia {
                index,
                asset: Box::new(remove(&mut project.media, index)?),
            },
            Op::InsertTrack {
                timeline: t,
                index,
                track,
            } => {
                insert(&mut timeline(project, t)?.tracks, index, *track)?;
                Op::RemoveTrack { timeline: t, index }
            }
            Op::RemoveTrack { timeline: t, index } => Op::InsertTrack {
                timeline: t,
                index,
                track: Box::new(remove(&mut timeline(project, t)?.tracks, index)?),
            },
            Op::SetTrackHeader { at, header } => {
                let track = track(project, at)?;
                let previous = TrackHeader::of(track);
                let TrackHeader {
                    name,
                    muted,
                    hidden,
                    locked,
                } = *header;
                (track.name, track.muted, track.hidden, track.locked) =
                    (name, muted, hidden, locked);
                Op::SetTrackHeader {
                    at,
                    header: Box::new(previous),
                }
            }
            Op::InsertClip { at, index, clip } => {
                insert(&mut track(project, at)?.clips, index, *clip)?;
                Op::RemoveClip { at, index }
            }
            Op::RemoveClip { at, index } => Op::InsertClip {
                at,
                index,
                clip: Box::new(remove(&mut track(project, at)?.clips, index)?),
            },
            Op::ReplaceClip { at, index, clip } => Op::ReplaceClip {
                at,
                index,
                clip: Box::new(replace(&mut track(project, at)?.clips, index, *clip)?),
            },
            Op::ShiftClips { at, from, delta } => {
                let clips = &mut track(project, at)?.clips;
                let moved = clips.get(from..).ok_or(BAD_POSITION)?;
                // Compute every new start before changing any, so a failure
                // leaves the track untouched.
                let starts = moved
                    .iter()
                    .map(|clip| clip.start.checked_add(delta))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(EditError::Internal("a ripple moved a clip out of range"))?;
                for (clip, start) in clips.iter_mut().skip(from).zip(starts) {
                    clip.start = start;
                }
                Op::ShiftClips {
                    at,
                    from,
                    delta: Time::ZERO
                        .checked_sub(delta)
                        .ok_or(EditError::Internal("a ripple offset is out of range"))?,
                }
            }
            Op::InsertTransition {
                at,
                index,
                transition,
            } => {
                insert(&mut track(project, at)?.transitions, index, *transition)?;
                Op::RemoveTransition { at, index }
            }
            Op::RemoveTransition { at, index } => Op::InsertTransition {
                at,
                index,
                transition: Box::new(remove(&mut track(project, at)?.transitions, index)?),
            },
            Op::ReplaceTransition {
                at,
                index,
                transition,
            } => Op::ReplaceTransition {
                at,
                index,
                transition: Box::new(replace(
                    &mut track(project, at)?.transitions,
                    index,
                    *transition,
                )?),
            },
            Op::InsertMarker {
                timeline: t,
                index,
                marker,
            } => {
                insert(&mut timeline(project, t)?.markers, index, *marker)?;
                Op::RemoveMarker { timeline: t, index }
            }
            Op::RemoveMarker { timeline: t, index } => Op::InsertMarker {
                timeline: t,
                index,
                marker: Box::new(remove(&mut timeline(project, t)?.markers, index)?),
            },
        })
    }
}

/// A sequence of ops applied in order: the record of one undo step.
///
/// Applying a patch returns the patch that reverts it, so undo and redo are
/// the same operation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    ops: Vec<Op>,
}

impl Patch {
    pub(crate) fn new(ops: Vec<Op>) -> Self {
        Self { ops }
    }

    /// `true` when the patch changes nothing (a command that was a no-op).
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Applies every op and returns the inverse patch. Does not validate
    /// the result; callers that apply a patch from outside the session (the
    /// journal) validate afterwards.
    ///
    /// A patch only fails when applied to a document other than the one it
    /// was made for (a bug, or a damaged journal); the ops applied so far are
    /// then reverted and [`EditError::Internal`] is returned.
    pub fn apply(self, project: &mut Project) -> Result<Patch, EditError> {
        let mut inverse = Vec::with_capacity(self.ops.len());
        for op in self.ops {
            match op.apply(project) {
                Ok(undo) => inverse.push(undo),
                Err(error) => {
                    revert(project, inverse);
                    return Err(error);
                }
            }
        }
        inverse.reverse();
        Ok(Patch { ops: inverse })
    }
}

/// Applies inverse ops, most recent first.
///
/// The inverse of an op that was just applied addresses positions that
/// exist by construction, so it cannot fail; the property tests check that
/// every command rolls back to the identical document. Should one fail
/// anyway, the remaining inverses still run so as much as possible is
/// restored.
pub(crate) fn revert(project: &mut Project, applied_inverses: Vec<Op>) {
    for op in applied_inverses.into_iter().rev() {
        let _ = op.apply(project);
    }
}
