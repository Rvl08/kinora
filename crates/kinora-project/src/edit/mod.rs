//! The editing core: applies [`EditCommand`]s to a [`Project`] and records
//! how to undo them (ADR-0004 §3, amendment 2).
//!
//! # Guarantees
//!
//! - **Atomic.** [`apply`] either applies the whole command (a batch
//!   included) or returns an error and leaves the document exactly as it was.
//! - **Valid in, valid out.** Each command checks its own rules first, for
//!   precise errors ([`EditRule`]). After it is applied, the validator runs
//!   over the whole document as a safety net. A result that breaks any
//!   format rule is rolled back and reported as [`EditError::Invalid`].
//! - **Exactly reversible.** The returned [`Patch`] restores the previous
//!   document bit for bit, and applying it returns the patch that redoes
//!   the command.
//! - **Deterministic.** The same command on the same document always gives
//!   the same result. New identifiers come from the command or are derived
//!   from it, never generated, so a journal replays identically.
//!
//! [`Editor`] adds the session around this: revisions, undo and redo
//! history, the saved state, and the journal ([`journal`]).
//!
//! # Rules every command follows
//!
//! - Commands never change a locked track's contents ([`EditRule::TrackLocked`]).
//!   `updateTrack` still works, so a track can be unlocked.
//! - Times must lie on the edit grid ([`EditRule::OffGrid`]).
//! - Clips on one track never overlap ([`EditRule::Overlap`]). Commands do
//!   not push other clips aside, except `removeClip` with `ripple`.
//! - A transition belongs to its cut. A command that moves either clip's
//!   edge at that cut (move, trim, speed change) removes the transition.
//!   Undo brings it back.

mod clip_ops;
mod error;
mod history;
pub mod journal;
mod patch;
mod tx;

pub use error::{EditError, EditRule, EntityKind};
pub use history::{Editor, EditorState, HistoryError, Outcome, Replay, Revision, Step};
pub use patch::Patch;

use kinora_core::Time;

use crate::command::{ClipEdge, EditCommand};
use crate::id::{ClipId, TrackId};
use crate::media::MediaAsset;
use crate::model::{Clip, ClipContent, Project, TrackKind};
use crate::validate::validate;

use clip_ops::{end_of, require_grid};
use patch::{Op, TrackAt, TrackHeader};
use tx::Tx;

/// Deepest nesting of batches accepted. Batches come from the UI, the
/// Copilot and scripts; the limit keeps recursion bounded for any input.
pub const MAX_BATCH_DEPTH: usize = 8;

/// Most commands one edit may contain, counting every command of every
/// nested batch. Bounds the work and the undo memory of a single edit.
pub const MAX_COMMANDS_PER_EDIT: usize = 100_000;

/// Applies `command` to `project`, which must be valid (as produced by
/// [`from_json`](crate::from_json) or a previous edit), and returns the patch
/// that undoes it. The patch is empty when the command changes nothing.
///
/// # Errors
/// See [`EditError`]. On error, `project` is unchanged.
pub fn apply(project: &mut Project, command: &EditCommand) -> Result<Patch, EditError> {
    check_size(command)?;
    let mut tx = Tx::new(project);
    if let Err(error) = execute(&mut tx, command) {
        tx.rollback();
        return Err(error);
    }
    let issues = validate(tx.project);
    if !issues.is_empty() {
        tx.rollback();
        return Err(EditError::Invalid { issues });
    }
    Ok(tx.commit())
}

/// Checks [`MAX_BATCH_DEPTH`] and [`MAX_COMMANDS_PER_EDIT`] without
/// recursion.
fn check_size(command: &EditCommand) -> Result<(), EditError> {
    let mut pending = vec![(command, 1_usize)];
    let mut count = 0_usize;
    while let Some((command, depth)) = pending.pop() {
        count += 1;
        if count > MAX_COMMANDS_PER_EDIT {
            return Err(EditError::rejected(
                EditRule::BatchTooLarge,
                format!("an edit may contain at most {MAX_COMMANDS_PER_EDIT} commands"),
            ));
        }
        if let EditCommand::Batch { commands, .. } = command {
            if depth > MAX_BATCH_DEPTH {
                return Err(EditError::rejected(
                    EditRule::BatchTooLarge,
                    format!("batches may be nested at most {MAX_BATCH_DEPTH} deep"),
                ));
            }
            pending.extend(commands.iter().map(|c| (c, depth + 1)));
        }
    }
    Ok(())
}

/// Carries out one command inside a transaction. Depth is bounded by
/// [`check_size`].
fn execute(tx: &mut Tx<'_>, command: &EditCommand) -> Result<(), EditError> {
    match command {
        EditCommand::Batch { commands, .. } => {
            for (index, command) in commands.iter().enumerate() {
                execute(tx, command).map_err(|error| EditError::InBatch {
                    index,
                    error: Box::new(error),
                })?;
            }
            Ok(())
        }
        EditCommand::ImportMedia { asset } => {
            tx.project
                .require_new_id(asset.id.as_uuid(), EntityKind::Media)?;
            let index = tx.project.media.len();
            tx.run(Op::InsertMedia {
                index,
                asset: asset.clone(),
            })
        }
        EditCommand::RemoveMedia { media } => remove_media(tx, media.as_uuid()),
        EditCommand::AddTrack {
            timeline,
            index,
            track,
        } => {
            let t = tx
                .project
                .timeline_index(timeline.as_uuid())
                .ok_or_else(|| EditError::not_found(EntityKind::Timeline, timeline.as_uuid()))?;
            let index = usize::try_from(*index).unwrap_or(usize::MAX);
            if index > tx.project.timelines[t].tracks.len() {
                return Err(EditError::rejected(
                    EditRule::Index,
                    format!("track index {index} is past the end of the timeline"),
                ));
            }
            tx.project
                .require_new_id(track.id.as_uuid(), EntityKind::Track)?;
            tx.run(Op::InsertTrack {
                timeline: t,
                index,
                track: Box::new(track.clone()),
            })
        }
        EditCommand::RemoveTrack { track } => {
            let at = find_track(tx.project, *track)?;
            require_unlocked(tx.project, at)?;
            tx.run(Op::RemoveTrack {
                timeline: at.timeline,
                index: at.track,
            })
        }
        EditCommand::UpdateTrack {
            track,
            name,
            muted,
            hidden,
            locked,
        } => {
            let at = find_track(tx.project, *track)?;
            let current = TrackHeader::of(tx.project.track(at)?);
            let header = TrackHeader {
                name: name.clone().unwrap_or_else(|| current.name.clone()),
                muted: muted.unwrap_or(current.muted),
                hidden: hidden.unwrap_or(current.hidden),
                locked: locked.unwrap_or(current.locked),
            };
            if header == current {
                return Ok(());
            }
            tx.run(Op::SetTrackHeader {
                at,
                header: Box::new(header),
            })
        }
        EditCommand::InsertClip { track, clip } => insert_clip(tx, *track, clip),
        EditCommand::RemoveClip { clip, ripple } => remove_clip(tx, *clip, *ripple),
        EditCommand::MoveClip { clip, track, start } => move_clip(tx, *clip, *track, *start),
        EditCommand::TrimClip { clip, edge, to } => trim_clip(tx, *clip, *edge, *to),
        EditCommand::SplitClip { clip, at, new_clip } => split_clip(tx, *clip, *at, *new_clip),
        EditCommand::SetSpeed { clip, speed } => {
            let (at, index, current) = find_clip(tx.project, *clip)?;
            require_unlocked(tx.project, at)?;
            let changed = clip_ops::with_speed(&current, *speed, asset_of(tx.project, &current))?;
            replace_resized(tx, at, index, &current, changed)
        }
        EditCommand::AddEffect {
            clip,
            index,
            effect,
        } => {
            tx.project
                .require_new_id(effect.id.as_uuid(), EntityKind::Effect)?;
            edit_clip(tx, *clip, |clip, _| {
                let effects = &mut visual_of(clip)?.effects;
                let index = usize::try_from(*index).unwrap_or(usize::MAX);
                if index > effects.len() {
                    return Err(EditError::rejected(
                        EditRule::Index,
                        format!("effect index {index} is past the end of the stack"),
                    ));
                }
                effects.insert(index, effect.clone());
                Ok(())
            })
        }
        EditCommand::RemoveEffect { clip, effect } => edit_clip(tx, *clip, |clip, _| {
            let effects = &mut visual_of(clip)?.effects;
            let index = effects
                .iter()
                .position(|e| e.id == *effect)
                .ok_or_else(|| EditError::not_found(EntityKind::Effect, effect.as_uuid()))?;
            effects.remove(index);
            Ok(())
        }),
        EditCommand::SetEffectParam {
            clip,
            effect,
            param,
            value,
        } => edit_clip(tx, *clip, |clip, _| {
            let instance = visual_of(clip)?
                .effects
                .iter_mut()
                .find(|e| e.id == *effect)
                .ok_or_else(|| EditError::not_found(EntityKind::Effect, effect.as_uuid()))?;
            instance.params.insert(param.clone(), value.clone());
            Ok(())
        }),
        EditCommand::RenameClip { clip, name } => edit_clip(tx, *clip, |clip, _| {
            clip.name.clone_from(name);
            Ok(())
        }),
        EditCommand::SetClipEnabled { clip, enabled } => edit_clip(tx, *clip, |clip, _| {
            clip.enabled = *enabled;
            Ok(())
        }),
        EditCommand::SetTransform { clip, transform } => edit_clip(tx, *clip, |clip, _| {
            visual_of(clip)?.transform = transform.clone();
            Ok(())
        }),
        EditCommand::SetCrop { clip, crop } => edit_clip(tx, *clip, |clip, _| {
            visual_of(clip)?.crop = *crop;
            Ok(())
        }),
        EditCommand::SetOpacity { clip, opacity } => edit_clip(tx, *clip, |clip, _| {
            visual_of(clip)?.opacity = opacity.clone();
            Ok(())
        }),
        EditCommand::SetBlendMode { clip, blend_mode } => edit_clip(tx, *clip, |clip, _| {
            visual_of(clip)?.blend_mode = *blend_mode;
            Ok(())
        }),
        EditCommand::SetAudioMix { clip, audio } => edit_clip(tx, *clip, |clip, kinds| {
            let allowed = match kinds.track {
                TrackKind::Audio => true,
                TrackKind::Video => kinds.media == Some(crate::media::MediaKind::Video),
                TrackKind::Text | TrackKind::Caption => false,
            };
            if !allowed {
                return Err(not_applicable("this clip has no sound"));
            }
            clip.audio = Some(audio.clone());
            Ok(())
        }),
        EditCommand::SetText { clip, content } => edit_clip(tx, *clip, |clip, _| {
            let ClipContent::Text(text) = &mut clip.content else {
                return Err(not_applicable("only text clips have a title"));
            };
            *text = content.clone();
            Ok(())
        }),
        EditCommand::SetCaption { clip, content } => edit_clip(tx, *clip, |clip, _| {
            let ClipContent::Caption(caption) = &mut clip.content else {
                return Err(not_applicable("only caption clips have a caption"));
            };
            *caption = content.clone();
            Ok(())
        }),
        EditCommand::AddTransition { track, transition } => {
            let at = find_track(tx.project, *track)?;
            require_unlocked(tx.project, at)?;
            tx.project
                .require_new_id(transition.id.as_uuid(), EntityKind::Transition)?;
            require_grid(transition.duration, "the transition duration")?;
            if !transition.duration.is_positive() {
                return Err(EditError::rejected(
                    EditRule::OutOfRange,
                    "a transition needs a positive duration",
                ));
            }
            let track = tx.project.track(at)?;
            let find = |id: ClipId| track.clips.iter().find(|c| c.id == id);
            let adjacent = match (find(transition.from), find(transition.to)) {
                (Some(from), Some(to)) => from.end() == Some(to.start),
                _ => false,
            };
            if !adjacent {
                return Err(EditError::rejected(
                    EditRule::NotAdjacent,
                    "`from` must end exactly where `to` starts, on this track",
                ));
            }
            let too_long = [transition.from, transition.to].iter().any(|&id| {
                track
                    .clips
                    .iter()
                    .any(|c| c.id == id && transition.duration > c.duration)
            });
            if too_long {
                return Err(EditError::rejected(
                    EditRule::TransitionTooLong,
                    "the transition is longer than one of its clips",
                ));
            }
            if track
                .transitions
                .iter()
                .any(|t| t.from == transition.from || t.to == transition.to)
            {
                return Err(EditError::rejected(
                    EditRule::TransitionExists,
                    "this cut already has a transition",
                ));
            }
            let index = track.transitions.len();
            tx.run(Op::InsertTransition {
                at,
                index,
                transition: Box::new(transition.clone()),
            })
        }
        EditCommand::RemoveTransition { transition } => {
            let (at, index) = tx
                .project
                .transition_at(transition.as_uuid())
                .ok_or_else(|| {
                    EditError::not_found(EntityKind::Transition, transition.as_uuid())
                })?;
            require_unlocked(tx.project, at)?;
            tx.run(Op::RemoveTransition { at, index })
        }
        EditCommand::AddMarker { timeline, marker } => {
            let t = tx
                .project
                .timeline_index(timeline.as_uuid())
                .ok_or_else(|| EditError::not_found(EntityKind::Timeline, timeline.as_uuid()))?;
            tx.project
                .require_new_id(marker.id.as_uuid(), EntityKind::Marker)?;
            require_grid(marker.start, "the marker start")?;
            require_grid(marker.duration, "the marker duration")?;
            if marker.start.is_negative() || marker.duration.is_negative() {
                return Err(EditError::rejected(
                    EditRule::OutOfRange,
                    "a marker cannot start before the timeline or have a negative length",
                ));
            }
            let markers = &tx.project.timelines[t].markers;
            let index = markers.partition_point(|m| m.start <= marker.start);
            tx.run(Op::InsertMarker {
                timeline: t,
                index,
                marker: Box::new(marker.clone()),
            })
        }
        EditCommand::RemoveMarker { marker } => {
            let (timeline, index) = tx
                .project
                .marker_at(marker.as_uuid())
                .ok_or_else(|| EditError::not_found(EntityKind::Marker, marker.as_uuid()))?;
            tx.run(Op::RemoveMarker { timeline, index })
        }
    }
}

fn not_applicable(message: &str) -> EditError {
    EditError::rejected(EditRule::NotApplicable, message)
}

fn find_track(project: &Project, id: TrackId) -> Result<TrackAt, EditError> {
    project
        .track_at(id.as_uuid())
        .ok_or_else(|| EditError::not_found(EntityKind::Track, id.as_uuid()))
}

/// The clip's position and a copy of it.
fn find_clip(project: &Project, id: ClipId) -> Result<(TrackAt, usize, Clip), EditError> {
    let (at, index) = project
        .clip_at(id.as_uuid())
        .ok_or_else(|| EditError::not_found(EntityKind::Clip, id.as_uuid()))?;
    let clip = project
        .track(at)?
        .clips
        .get(index)
        .ok_or(EditError::Internal("a clip position does not exist"))?;
    Ok((at, index, clip.clone()))
}

fn require_unlocked(project: &Project, at: TrackAt) -> Result<(), EditError> {
    let track = project.track(at)?;
    if track.locked {
        Err(EditError::rejected(
            EditRule::TrackLocked,
            format!("track {} is locked", track.id),
        ))
    } else {
        Ok(())
    }
}

/// The media asset a clip plays, if it is a media clip.
fn asset_of<'a>(project: &'a Project, clip: &Clip) -> Option<&'a MediaAsset> {
    match &clip.content {
        ClipContent::Media(content) => project.media(content.media),
        _ => None,
    }
}

fn visual_of(clip: &mut Clip) -> Result<&mut crate::model::Visual, EditError> {
    clip.visual
        .as_mut()
        .ok_or_else(|| not_applicable("this clip has no picture"))
}

/// The kinds a property setter needs to decide whether it applies.
struct ClipKinds {
    track: TrackKind,
    media: Option<crate::media::MediaKind>,
}

/// Changes a clip's properties in place (not its timing): checks the lock,
/// lets `change` edit a copy, and replaces the clip if anything changed.
fn edit_clip(
    tx: &mut Tx<'_>,
    id: ClipId,
    change: impl FnOnce(&mut Clip, &ClipKinds) -> Result<(), EditError>,
) -> Result<(), EditError> {
    let (at, index, current) = find_clip(tx.project, id)?;
    require_unlocked(tx.project, at)?;
    let kinds = ClipKinds {
        track: tx.project.track(at)?.kind,
        media: asset_of(tx.project, &current).map(|a| a.kind),
    };
    let mut changed = current.clone();
    change(&mut changed, &kinds)?;
    if changed == current {
        return Ok(());
    }
    tx.run(Op::ReplaceClip {
        at,
        index,
        clip: Box::new(changed),
    })
}

/// Fails with [`EditRule::Overlap`] when `[start, end)` meets a clip of the
/// track other than `skip`.
fn require_room(
    project: &Project,
    at: TrackAt,
    start: Time,
    end: Time,
    skip: Option<usize>,
) -> Result<(), EditError> {
    let track = project.track(at)?;
    // Clips are sorted and disjoint, so only the ones starting before `end`
    // can overlap, and of those only the last few; scanning back from the
    // insertion point stops at the first clip that ends before `start`.
    let upto = track.clips.partition_point(|c| c.start < end);
    for (index, other) in track.clips[..upto].iter().enumerate().rev() {
        if Some(index) == skip {
            continue;
        }
        let other_end = end_of(other)?;
        if other_end <= start {
            break;
        }
        return Err(EditError::rejected(
            EditRule::Overlap,
            format!("the clip would overlap clip {}", other.id),
        ));
    }
    Ok(())
}

/// Removes every transition of the track that involves `clip` at the given
/// cut(s): as the outgoing clip (`from`) and/or the incoming one (`to`).
fn drop_transitions(
    tx: &mut Tx<'_>,
    at: TrackAt,
    clip: ClipId,
    as_from: bool,
    as_to: bool,
) -> Result<(), EditError> {
    let indices: Vec<usize> = tx
        .project
        .track(at)?
        .transitions
        .iter()
        .enumerate()
        .filter(|(_, t)| (as_from && t.from == clip) || (as_to && t.to == clip))
        .map(|(index, _)| index)
        .collect();
    for index in indices.into_iter().rev() {
        tx.run(Op::RemoveTransition { at, index })?;
    }
    Ok(())
}

/// Replaces a clip whose start or duration may have changed: drops the
/// transitions at the edges that moved and checks for room.
fn replace_resized(
    tx: &mut Tx<'_>,
    at: TrackAt,
    index: usize,
    current: &Clip,
    changed: Clip,
) -> Result<(), EditError> {
    if changed == *current {
        return Ok(());
    }
    let start_moved = changed.start != current.start;
    let end_moved = changed.end() != current.end();
    require_room(
        tx.project,
        at,
        changed.start,
        end_of(&changed)?,
        Some(index),
    )?;
    drop_transitions(tx, at, current.id, end_moved, start_moved)?;
    tx.run(Op::ReplaceClip {
        at,
        index,
        clip: Box::new(changed),
    })?;
    require_transitions_fit(tx.project, at)
}

/// Fails with [`EditRule::TransitionTooLong`] when a transition of the track
/// is longer than one of its clips (after a clip got shorter).
fn require_transitions_fit(project: &Project, at: TrackAt) -> Result<(), EditError> {
    let track = project.track(at)?;
    let duration = |id: ClipId| track.clips.iter().find(|c| c.id == id).map(|c| c.duration);
    for transition in &track.transitions {
        let fits = |clip| duration(clip).is_none_or(|d| transition.duration <= d);
        if !(fits(transition.from) && fits(transition.to)) {
            return Err(EditError::rejected(
                EditRule::TransitionTooLong,
                format!(
                    "transition {} would be longer than one of its clips",
                    transition.id
                ),
            ));
        }
    }
    Ok(())
}

fn remove_media(tx: &mut Tx<'_>, id: uuid::Uuid) -> Result<(), EditError> {
    let index = tx
        .project
        .media_index(id)
        .ok_or_else(|| EditError::not_found(EntityKind::Media, id))?;
    let used_by_clip = tx.project.timelines.iter().any(|t| {
        t.tracks.iter().any(|k| {
            k.clips
                .iter()
                .any(|c| matches!(&c.content, ClipContent::Media(m) if m.media.as_uuid() == id))
        })
    });
    let used_as_source = tx.project.media.iter().any(|m| {
        matches!(&m.origin, crate::media::MediaOrigin::Generated { source: Some(s), .. } if s.as_uuid() == id)
    });
    if used_by_clip || used_as_source {
        return Err(EditError::rejected(
            EditRule::InUse,
            "the media is still used by a clip or a generated asset",
        ));
    }
    tx.run(Op::RemoveMedia { index })
}

fn insert_clip(tx: &mut Tx<'_>, track: TrackId, clip: &Clip) -> Result<(), EditError> {
    let at = find_track(tx.project, track)?;
    require_unlocked(tx.project, at)?;
    tx.project
        .require_new_id(clip.id.as_uuid(), EntityKind::Clip)?;
    require_grid(clip.start, "the clip start")?;
    require_grid(clip.duration, "the clip duration")?;
    if !clip.duration.is_positive() {
        return Err(EditError::rejected(
            EditRule::EmptyClip,
            "a clip needs a positive duration",
        ));
    }
    if clip.start.is_negative() {
        return Err(EditError::rejected(
            EditRule::OutOfRange,
            "a clip cannot start before the timeline",
        ));
    }
    let end = end_of(clip)?;
    require_room(tx.project, at, clip.start, end, None)?;
    let index = tx
        .project
        .track(at)?
        .clips
        .partition_point(|c| c.start < clip.start);
    tx.run(Op::InsertClip {
        at,
        index,
        clip: Box::new(clip.clone()),
    })
}

fn remove_clip(tx: &mut Tx<'_>, id: ClipId, ripple: bool) -> Result<(), EditError> {
    let (at, index, clip) = find_clip(tx.project, id)?;
    require_unlocked(tx.project, at)?;
    drop_transitions(tx, at, id, true, true)?;
    tx.run(Op::RemoveClip { at, index })?;
    let later_clips = tx.project.track(at)?.clips.len() > index;
    if ripple && later_clips {
        // Later clips start at or after the removed clip's end, so moving
        // them back by its duration cannot overlap or go below zero.
        let delta = Time::ZERO
            .checked_sub(clip.duration)
            .ok_or(EditError::Internal("a clip duration is out of range"))?;
        tx.run(Op::ShiftClips {
            at,
            from: index,
            delta,
        })?;
    }
    Ok(())
}

fn move_clip(tx: &mut Tx<'_>, id: ClipId, track: TrackId, start: Time) -> Result<(), EditError> {
    let (from, index, clip) = find_clip(tx.project, id)?;
    let to = find_track(tx.project, track)?;
    if from == to && clip.start == start {
        return Ok(());
    }
    require_unlocked(tx.project, from)?;
    require_unlocked(tx.project, to)?;
    if from.timeline != to.timeline {
        return Err(EditError::rejected(
            EditRule::OtherTimeline,
            "clips can only move between tracks of the same timeline",
        ));
    }
    if tx.project.track(from)?.kind != tx.project.track(to)?.kind {
        return Err(EditError::rejected(
            EditRule::TrackKind,
            "clips can only move to a track of the same kind",
        ));
    }
    require_grid(start, "the new start")?;
    if start.is_negative() {
        return Err(EditError::rejected(
            EditRule::OutOfRange,
            "a clip cannot start before the timeline",
        ));
    }
    let mut moved = clip;
    moved.start = start;
    let end = end_of(&moved)?;

    drop_transitions(tx, from, id, true, true)?;
    tx.run(Op::RemoveClip { at: from, index })?;
    require_room(tx.project, to, start, end, None)?;
    let index = tx
        .project
        .track(to)?
        .clips
        .partition_point(|c| c.start < start);
    tx.run(Op::InsertClip {
        at: to,
        index,
        clip: Box::new(moved),
    })
}

fn trim_clip(tx: &mut Tx<'_>, id: ClipId, edge: ClipEdge, to: Time) -> Result<(), EditError> {
    let (at, index, current) = find_clip(tx.project, id)?;
    require_unlocked(tx.project, at)?;
    let trimmed = clip_ops::trim(&current, edge, to, asset_of(tx.project, &current))?;
    replace_resized(tx, at, index, &current, trimmed)
}

fn split_clip(tx: &mut Tx<'_>, id: ClipId, cut: Time, right_id: ClipId) -> Result<(), EditError> {
    let (at, index, current) = find_clip(tx.project, id)?;
    require_unlocked(tx.project, at)?;
    tx.project
        .require_new_id(right_id.as_uuid(), EntityKind::Clip)?;
    let (left, right) = clip_ops::split(&current, cut, right_id, asset_of(tx.project, &current))?;
    // The outgoing transition now leaves from the right half.
    let outgoing: Vec<usize> = tx
        .project
        .track(at)?
        .transitions
        .iter()
        .enumerate()
        .filter(|(_, t)| t.from == id)
        .map(|(index, _)| index)
        .collect();
    for transition in outgoing {
        let mut changed = tx.project.track(at)?.transitions[transition].clone();
        changed.from = right_id;
        tx.run(Op::ReplaceTransition {
            at,
            index: transition,
            transition: Box::new(changed),
        })?;
    }
    tx.run(Op::ReplaceClip {
        at,
        index,
        clip: Box::new(left),
    })?;
    tx.run(Op::InsertClip {
        at,
        index: index + 1,
        clip: Box::new(right),
    })?;
    require_transitions_fit(tx.project, at)
}
