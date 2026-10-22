//! Pure functions that compute a changed clip: trimming, splitting and
//! changing speed. They never touch the document; the planner places the
//! results with ops.
//!
//! # Source ranges
//!
//! A media clip plays `sourceIn .. sourceIn + source_duration(duration)`
//! ([`Speed::source_duration`]), forwards or, with `reverse`, backwards.
//! When an edge moves, the source point under the *other* edge stays fixed,
//! so the content under the untouched edge does not move:
//!
//! | Edge moved | Forwards | Reversed |
//! |---|---|---|
//! | start | source end fixed: `sourceIn` moves | `sourceIn` fixed |
//! | end | `sourceIn` fixed | source end fixed: `sourceIn` moves |
//!
//! Keeping an end fixed (instead of re-deriving it from the in-point) means
//! a clip that ends exactly at the end of its media still does after any
//! trim or split, even when speed rounding is involved.

use kinora_core::Time;
use uuid::Uuid;

use crate::command::ClipEdge;
use crate::id::{ClipId, EffectInstanceId};
use crate::media::MediaAsset;
use crate::model::{AudioMix, Clip, ClipContent, MediaContent, Speed};
use crate::value::Animated;

use super::{EditError, EditRule};

fn out_of_range(what: &str) -> EditError {
    EditError::rejected(EditRule::OutOfRange, format!("{what} is out of range"))
}

fn add(a: Time, b: Time, what: &str) -> Result<Time, EditError> {
    a.checked_add(b).ok_or_else(|| out_of_range(what))
}

fn sub(a: Time, b: Time, what: &str) -> Result<Time, EditError> {
    a.checked_sub(b).ok_or_else(|| out_of_range(what))
}

/// Fails with [`EditRule::OffGrid`] unless `time` is on the edit grid.
pub(crate) fn require_grid(time: Time, what: &str) -> Result<(), EditError> {
    if time.is_on_grid() {
        Ok(())
    } else {
        Err(EditError::rejected(
            EditRule::OffGrid,
            format!("{what} ({time}) is not a whole number of flicks"),
        ))
    }
}

/// The clip's end, `start + duration`.
pub(crate) fn end_of(clip: &Clip) -> Result<Time, EditError> {
    clip.end().ok_or_else(|| out_of_range("the clip's end"))
}

/// How many source seconds play during `duration` at `speed`.
fn source_len(speed: Speed, duration: Time) -> Result<Time, EditError> {
    speed
        .source_duration(duration)
        .ok_or_else(|| out_of_range("the source duration"))
}

/// The media content of a clip whose asset has a duration (video or audio),
/// with that duration. Images and non-media clips have no source range.
fn timed_media<'a>(
    clip: &'a mut Clip,
    asset: Option<&MediaAsset>,
) -> Option<(&'a mut MediaContent, Time)> {
    match (&mut clip.content, asset.and_then(|a| a.info.duration)) {
        (ClipContent::Media(content), Some(length)) => Some((content, length)),
        _ => None,
    }
}

/// Checks that `source_in .. source_in + len` lies within `0..=length`.
fn require_source_range(source_in: Time, len: Time, length: Time) -> Result<(), EditError> {
    if source_in.is_negative() {
        return Err(EditError::rejected(
            EditRule::SourceRange,
            "the clip would start before the beginning of its media",
        ));
    }
    if add(source_in, len, "the source end")? > length {
        return Err(EditError::rejected(
            EditRule::SourceRange,
            "the clip would play past the end of its media",
        ));
    }
    Ok(())
}

/// Moves every keyframe and caption word of the clip by `-offset`, so they
/// stay on the same content when the clip's start moves by `offset`.
fn shift_relative_times(clip: &mut Clip, offset: Time) -> Result<(), EditError> {
    if offset.is_zero() {
        return Ok(());
    }
    if let Some(visual) = &mut clip.visual {
        shift_animated(&mut visual.transform.position, offset)?;
        shift_animated(&mut visual.transform.scale, offset)?;
        shift_animated(&mut visual.transform.rotation, offset)?;
        shift_animated(&mut visual.opacity, offset)?;
        for effect in &mut visual.effects {
            for value in effect.params.values_mut() {
                shift_animated(value, offset)?;
            }
        }
    }
    if let Some(audio) = &mut clip.audio {
        shift_animated(&mut audio.gain_db, offset)?;
    }
    if let ClipContent::Caption(caption) = &mut clip.content {
        for word in &mut caption.words {
            word.start = sub(word.start, offset, "a caption word")?;
            word.end = sub(word.end, offset, "a caption word")?;
        }
    }
    Ok(())
}

fn shift_animated<T>(animated: &mut Animated<T>, offset: Time) -> Result<(), EditError> {
    if let Animated::Keyframes { keyframes } = animated {
        for keyframe in keyframes {
            keyframe.at = sub(keyframe.at, offset, "a keyframe")?;
        }
    }
    Ok(())
}

/// Shortens fades that no longer fit the clip: the fade-in first, then the
/// fade-out within what is left.
pub(crate) fn clamp_fades(audio: &mut Option<AudioMix>, duration: Time) -> Result<(), EditError> {
    if let Some(audio) = audio {
        audio.fade_in = audio.fade_in.min(duration);
        let room = sub(duration, audio.fade_in, "the fade")?;
        audio.fade_out = audio.fade_out.min(room);
    }
    Ok(())
}

/// The clip with one edge moved to `to`.
pub(crate) fn trim(
    clip: &Clip,
    edge: ClipEdge,
    to: Time,
    asset: Option<&MediaAsset>,
) -> Result<Clip, EditError> {
    require_grid(to, "the new edge")?;
    let end = end_of(clip)?;
    let (start, duration) = match edge {
        ClipEdge::Start => (to, sub(end, to, "the duration")?),
        ClipEdge::End => (clip.start, sub(to, clip.start, "the duration")?),
    };
    if !duration.is_positive() {
        return Err(EditError::rejected(
            EditRule::EmptyClip,
            "the trim would leave the clip with no duration",
        ));
    }
    if start.is_negative() {
        return Err(EditError::rejected(
            EditRule::OutOfRange,
            "a clip cannot start before the timeline",
        ));
    }

    let mut trimmed = clip.clone();
    trimmed.start = start;
    trimmed.duration = duration;
    if let Some((content, length)) = timed_media(&mut trimmed, asset) {
        let new_len = source_len(content.speed, duration)?;
        // The source point under the untouched edge stays fixed (module docs).
        if (edge == ClipEdge::Start) != content.reverse {
            let old_len = source_len(content.speed, clip.duration)?;
            let source_end = add(content.source_in, old_len, "the source end")?;
            content.source_in = sub(source_end, new_len, "the source in-point")?;
        }
        require_source_range(content.source_in, new_len, length)?;
    }
    if edge == ClipEdge::Start {
        shift_relative_times(&mut trimmed, sub(start, clip.start, "the trim")?)?;
    }
    clamp_fades(&mut trimmed.audio, duration)?;
    Ok(trimmed)
}

/// The two halves of the clip cut at `at`: the left one keeps the clip's
/// identifier, the right one gets `right_id`.
///
/// Both halves keep every keyframe and caption word (the right half's
/// shifted to its new start), so the animation continues across the cut.
/// Effects of the right half get identifiers derived from `right_id` and the
/// original effect's identifier (UUIDv5), so replaying the same command
/// yields the same document.
pub(crate) fn split(
    clip: &Clip,
    at: Time,
    right_id: ClipId,
    asset: Option<&MediaAsset>,
) -> Result<(Clip, Clip), EditError> {
    require_grid(at, "the split position")?;
    let end = end_of(clip)?;
    if at <= clip.start || at >= end {
        return Err(EditError::rejected(
            EditRule::OutsideClip,
            format!(
                "{at} is not strictly inside the clip ({} to {end})",
                clip.start
            ),
        ));
    }
    let left_duration = sub(at, clip.start, "the left half")?;
    let right_duration = sub(end, at, "the right half")?;

    let mut left = clip.clone();
    left.duration = left_duration;
    let mut right = clip.clone();
    right.id = right_id;
    right.start = at;
    right.duration = right_duration;

    if let ClipContent::Media(content) = &clip.content
        && let Some(length) = asset.and_then(|a| a.info.duration)
    {
        let total = source_len(content.speed, clip.duration)?;
        let source_end = add(content.source_in, total, "the source end")?;
        // The half that touches the original source end keeps it exactly.
        let (late, late_duration) = if content.reverse {
            (&mut left, left_duration)
        } else {
            (&mut right, right_duration)
        };
        if let Some((late_content, _)) = timed_media(late, asset) {
            let late_len = source_len(late_content.speed, late_duration)?;
            late_content.source_in = sub(source_end, late_len, "the source in-point")?;
            require_source_range(late_content.source_in, late_len, length)?;
        }
    }

    shift_relative_times(&mut right, left_duration)?;
    if let Some(visual) = &mut right.visual {
        for effect in &mut visual.effects {
            effect.id = derived_effect_id(right_id, effect.id);
        }
    }
    if let Some(audio) = &mut left.audio {
        audio.fade_out = Time::ZERO;
    }
    if let Some(audio) = &mut right.audio {
        audio.fade_in = Time::ZERO;
    }
    clamp_fades(&mut left.audio, left_duration)?;
    clamp_fades(&mut right.audio, right_duration)?;
    Ok((left, right))
}

/// The identifier of a copy of `effect` on the clip `owner` (RFC 9562
/// name-based UUIDv5: namespace = the owner, name = the original id's
/// bytes). Stable across builds, which journal replay relies on.
pub(crate) fn derived_effect_id(owner: ClipId, effect: EffectInstanceId) -> EffectInstanceId {
    EffectInstanceId::from_uuid(Uuid::new_v5(&owner.as_uuid(), effect.as_uuid().as_bytes()))
}

/// The clip at a new speed, keeping its start, its source in-point and the
/// length of source it plays; the timeline duration follows (rounded down so
/// the source range never grows past the media).
pub(crate) fn with_speed(
    clip: &Clip,
    speed: Speed,
    asset: Option<&MediaAsset>,
) -> Result<Clip, EditError> {
    let Speed::Constant { rate } = speed;
    if !rate.is_positive() {
        return Err(EditError::rejected(
            EditRule::OutOfRange,
            "speed must be greater than zero",
        ));
    }
    let mut changed = clip.clone();
    let Some((content, length)) = timed_media(&mut changed, asset) else {
        return Err(EditError::rejected(
            EditRule::NotApplicable,
            "only video and audio clips have a speed",
        ));
    };
    let source = source_len(content.speed, clip.duration)?;
    let duration = speed
        .timeline_duration(source)
        .ok_or_else(|| out_of_range("the new duration"))?;
    if !duration.is_positive() {
        return Err(EditError::rejected(
            EditRule::EmptyClip,
            "at this speed the clip would have no duration",
        ));
    }
    content.speed = speed;
    require_source_range(content.source_in, source_len(speed, duration)?, length)?;
    changed.duration = duration;
    end_of(&changed)?;
    clamp_fades(&mut changed.audio, duration)?;
    Ok(changed)
}
