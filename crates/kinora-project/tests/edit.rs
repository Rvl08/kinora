//! The editing core on the sample project: each command's behaviour, its
//! refusals, and exact undo.

use kinora_core::{Rational, Time};
use kinora_project::command::ClipEdge;
use kinora_project::edit::{self, EditError, EditRule, EntityKind};
use kinora_project::id::{ClipId, EffectInstanceId, MarkerId, MediaId, TrackId, TransitionId};
use kinora_project::model::{Clip, ClipContent, Marker, MarkerColor, Speed, Transition};
use kinora_project::value::{Animated, ParamValue};
use kinora_project::{EditCommand, Project, from_json, to_json};

const SAMPLE: &str = include_str!("fixtures/v0/sample.kinora/project.json");

/// The sample's identifiers end in a three-digit number.
fn id<T: std::str::FromStr>(suffix: u32) -> T
where
    T::Err: std::fmt::Debug,
{
    format!("0192f1c4-0000-7000-8000-000000000{suffix}")
        .parse()
        .expect("sample id")
}

fn sample() -> Project {
    from_json(SAMPLE).expect("the sample is valid")
}

fn t(num: i64, den: u32) -> Time {
    Time::new(num, den).expect("valid time")
}

fn secs(s: i64) -> Time {
    Time::from_secs(s).expect("valid time")
}

fn clip(project: &Project, suffix: u32) -> &Clip {
    let id: ClipId = id(suffix);
    project
        .timelines
        .iter()
        .flat_map(|t| &t.tracks)
        .flat_map(|k| &k.clips)
        .find(|c| c.id == id)
        .expect("clip exists")
}

fn source_in(clip: &Clip) -> Time {
    match &clip.content {
        ClipContent::Media(media) => media.source_in,
        _ => panic!("not a media clip"),
    }
}

/// Applies the command, checks that the patch undoes it exactly and that
/// the inverse of the undo redoes it exactly, and returns the edited document.
fn applied(project: &Project, command: &EditCommand) -> Project {
    let mut edited = project.clone();
    let undo = edit::apply(&mut edited, command).expect("the command applies");
    assert!(
        to_json(&edited).is_ok(),
        "the result saves and reads back identically"
    );
    let after = edited.clone();
    let redo = undo.apply(&mut edited).expect("undo applies");
    assert_eq!(&edited, project, "undo restores the document exactly");
    redo.apply(&mut edited).expect("redo applies");
    assert_eq!(edited, after, "redo restores the edit exactly");
    after
}

/// Applies a command that must be refused and checks the document is
/// unchanged.
fn refused(project: &Project, command: &EditCommand) -> EditError {
    let mut edited = project.clone();
    let error = edit::apply(&mut edited, command).expect_err("the command is refused");
    assert_eq!(&edited, project, "a refused command changes nothing");
    error
}

fn rule(error: &EditError) -> Option<EditRule> {
    match error.root() {
        EditError::Rejected { rule, .. } => Some(*rule),
        _ => None,
    }
}

#[test]
fn split_cuts_the_source_and_keeps_animation_and_transitions() {
    let project = sample();
    let right: ClipId = ClipId::generate();
    let edited = applied(
        &project,
        &EditCommand::SplitClip {
            clip: id(501),
            at: secs(2),
            new_clip: right,
        },
    );
    let left = clip(&edited, 501);
    let track = &edited.timelines[0].tracks[0];
    let right_clip = track
        .clips
        .iter()
        .find(|c| c.id == right)
        .expect("right half");
    assert_eq!((left.start, left.duration), (secs(0), secs(2)));
    assert_eq!((right_clip.start, right_clip.duration), (secs(2), secs(3)));
    assert_eq!(source_in(left), secs(2));
    assert_eq!(source_in(right_clip), secs(4));
    // The right half's keyframes moved with it: the second keyframe was at
    // 5 s into the clip, now 3 s into the right half; the first lies before it.
    let Animated::Keyframes { keyframes } = &right_clip
        .visual
        .as_ref()
        .expect("visual")
        .transform
        .position
    else {
        panic!("keyframed");
    };
    assert_eq!(keyframes[0].at, secs(-2));
    assert_eq!(keyframes[1].at, secs(3));
    // Fades: the fade-out belongs to the right half only.
    assert_eq!(left.audio.as_ref().expect("audio").fade_out, Time::ZERO);
    assert_eq!(right_clip.audio.as_ref().expect("audio").fade_out, t(1, 2));
    // The outgoing transition now leaves from the right half.
    assert_eq!(track.transitions[0].from, right);
    // The copied effect has a derived, stable identifier.
    let original: EffectInstanceId = id(601);
    let copied = right_clip.visual.as_ref().expect("visual").effects[0].id;
    assert_ne!(copied, original);
    let again = applied(
        &project,
        &EditCommand::SplitClip {
            clip: id(501),
            at: secs(2),
            new_clip: right,
        },
    );
    assert_eq!(again, edited, "the same command gives the same document");
}

#[test]
fn split_refuses_positions_outside_or_off_the_grid() {
    let project = sample();
    let split = |at| EditCommand::SplitClip {
        clip: id(501),
        at,
        new_clip: ClipId::generate(),
    };
    assert_eq!(
        rule(&refused(&project, &split(secs(0)))),
        Some(EditRule::OutsideClip)
    );
    assert_eq!(
        rule(&refused(&project, &split(secs(5)))),
        Some(EditRule::OutsideClip)
    );
    assert_eq!(
        rule(&refused(&project, &split(t(1, 1001)))),
        Some(EditRule::OffGrid)
    );
    let reused = EditCommand::SplitClip {
        clip: id(501),
        at: secs(2),
        new_clip: id(502),
    };
    assert_eq!(
        rule(&refused(&project, &reused)),
        Some(EditRule::DuplicateId)
    );
}

#[test]
fn trimming_the_start_keeps_content_in_place() {
    let project = sample();
    let edited = applied(
        &project,
        &EditCommand::TrimClip {
            clip: id(501),
            edge: ClipEdge::Start,
            to: secs(1),
        },
    );
    let trimmed = clip(&edited, 501);
    assert_eq!((trimmed.start, trimmed.duration), (secs(1), secs(4)));
    assert_eq!(source_in(trimmed), secs(3));
    let Animated::Keyframes { keyframes } =
        &trimmed.visual.as_ref().expect("visual").transform.position
    else {
        panic!("keyframed");
    };
    assert_eq!(keyframes[0].at, secs(-1));
    // The transition sits on the other edge and stays.
    assert_eq!(edited.timelines[0].tracks[0].transitions.len(), 1);
}

#[test]
fn trimming_the_end_at_a_transition_removes_it_and_undo_restores_it() {
    let project = sample();
    let edited = applied(
        &project,
        &EditCommand::TrimClip {
            clip: id(501),
            edge: ClipEdge::End,
            to: secs(4),
        },
    );
    assert!(edited.timelines[0].tracks[0].transitions.is_empty());
    // `applied` already checked that undo brings the transition back.
}

#[test]
fn trims_respect_neighbours_media_and_locks() {
    let project = sample();
    let trim = |clip: u32, edge, to| EditCommand::TrimClip {
        clip: id(clip),
        edge,
        to,
    };
    // 502 starts where 501 ends.
    assert_eq!(
        rule(&refused(&project, &trim(502, ClipEdge::Start, secs(4)))),
        Some(EditRule::Overlap)
    );
    // 509 plays beach.mp4 (30 s) from 2 s: at most 28 s long.
    assert_eq!(
        rule(&refused(&project, &trim(509, ClipEdge::End, secs(29)))),
        Some(EditRule::SourceRange)
    );
    applied(&project, &trim(509, ClipEdge::End, secs(28)));
    // 501 plays from 2 s: its start can move back 2 s at most.
    applied(&project, &trim(501, ClipEdge::Start, Time::ZERO));
    // Trimming an edge to where it already is changes nothing.
    let mut unchanged = project.clone();
    let patch = edit::apply(&mut unchanged, &trim(505, ClipEdge::End, secs(10))).expect("applies");
    assert!(patch.is_empty());
    assert_eq!(
        rule(&refused(&project, &trim(501, ClipEdge::End, Time::ZERO))),
        Some(EditRule::EmptyClip)
    );
    // 504 is on the locked logo track.
    assert_eq!(
        rule(&refused(&project, &trim(504, ClipEdge::End, secs(2)))),
        Some(EditRule::TrackLocked)
    );
}

#[test]
fn unlocking_a_track_allows_edits() {
    let project = sample();
    let edited = applied(
        &project,
        &EditCommand::UpdateTrack {
            track: id(302),
            name: None,
            muted: None,
            hidden: None,
            locked: Some(false),
        },
    );
    applied(
        &edited,
        &EditCommand::TrimClip {
            clip: id(504),
            edge: ClipEdge::End,
            to: secs(2),
        },
    );
}

#[test]
fn ripple_delete_closes_the_gap() {
    let project = sample();
    let edited = applied(
        &project,
        &EditCommand::RemoveClip {
            clip: id(502),
            ripple: true,
        },
    );
    let track = &edited.timelines[0].tracks[0];
    assert_eq!(track.clips.len(), 2);
    assert_eq!(clip(&edited, 503).start, secs(5));
    assert!(track.transitions.is_empty(), "its transition went with it");

    let plain = applied(
        &project,
        &EditCommand::RemoveClip {
            clip: id(502),
            ripple: false,
        },
    );
    assert_eq!(clip(&plain, 503).start, secs(9));
}

#[test]
fn moves_check_kind_timeline_lock_and_room() {
    let project = sample();
    let moved = applied(
        &project,
        &EditCommand::MoveClip {
            clip: id(507),
            track: id(304),
            start: secs(20),
        },
    );
    assert_eq!(clip(&moved, 507).start, secs(20));
    let to = |clip: u32, track: u32, start| EditCommand::MoveClip {
        clip: id(clip),
        track: id(track),
        start,
    };
    assert_eq!(
        rule(&refused(&project, &to(503, 302, secs(20)))),
        Some(EditRule::TrackLocked)
    );
    assert_eq!(
        rule(&refused(&project, &to(505, 301, secs(20)))),
        Some(EditRule::TrackKind)
    );
    assert_eq!(
        rule(&refused(&project, &to(503, 306, secs(20)))),
        Some(EditRule::OtherTimeline)
    );
    assert_eq!(
        rule(&refused(&project, &to(503, 301, secs(4)))),
        Some(EditRule::Overlap)
    );
    // Moving 501 drops its transition; undo (checked by `applied`) restores it.
    let shifted = applied(&project, &to(501, 301, secs(20)));
    assert!(shifted.timelines[0].tracks[0].transitions.is_empty());
}

#[test]
fn speed_changes_keep_the_source_and_resize_the_clip() {
    let project = sample();
    let speed = |clip: u32, num, den| EditCommand::SetSpeed {
        clip: id(clip),
        speed: Speed::Constant {
            rate: Rational::new(num, den).expect("valid"),
        },
    };
    // 509 plays 1001/200 s of source; at 2× it lasts half as long.
    let faster = applied(&project, &speed(509, 2, 1));
    assert_eq!(clip(&faster, 509).duration, t(1001, 400));
    assert_eq!(source_in(clip(&faster, 509)), secs(2));
    // At 1/3× the duration is rounded down to the flick.
    let slower = applied(&project, &speed(509, 1, 3));
    assert!(clip(&slower, 509).duration.is_on_grid());
    // 502 plays 8 s of source at 2×; at 1× it would cover 503.
    assert_eq!(
        rule(&refused(&project, &speed(502, 1, 1))),
        Some(EditRule::Overlap)
    );
    assert_eq!(
        rule(&refused(&project, &speed(503, 2, 1))),
        Some(EditRule::NotApplicable)
    );
    assert_eq!(
        rule(&refused(&project, &speed(509, 0, 1))),
        Some(EditRule::OutOfRange)
    );
}

#[test]
fn media_in_use_cannot_be_removed() {
    let project = sample();
    let remove = |media: MediaId| EditCommand::RemoveMedia { media };
    assert_eq!(
        rule(&refused(&project, &remove(id(101)))),
        Some(EditRule::InUse)
    );
    let mut asset = project.media[0].clone();
    asset.id = MediaId::generate();
    let imported = applied(
        &project,
        &EditCommand::ImportMedia {
            asset: Box::new(asset.clone()),
        },
    );
    assert_eq!(imported.media.len(), 5);
    applied(&imported, &remove(asset.id));
    let error = refused(&project, &remove(MediaId::generate()));
    assert!(matches!(
        error,
        EditError::NotFound {
            entity: EntityKind::Media,
            ..
        }
    ));
}

#[test]
fn batches_are_atomic() {
    let project = sample();
    let batch = EditCommand::Batch {
        label: "Rename and split".into(),
        commands: vec![
            EditCommand::RenameClip {
                clip: id(501),
                name: Some("Pantai".into()),
            },
            EditCommand::SplitClip {
                clip: id(501),
                at: secs(9),
                new_clip: ClipId::generate(),
            },
        ],
    };
    let error = refused(&project, &batch);
    assert_eq!(error.command_pointer(), "/params/commands/1");
    assert_eq!(rule(&error), Some(EditRule::OutsideClip));

    // Later commands see the effect of earlier ones.
    let right = ClipId::generate();
    let ok = EditCommand::Batch {
        label: "Split twice".into(),
        commands: vec![
            EditCommand::SplitClip {
                clip: id(501),
                at: secs(2),
                new_clip: right,
            },
            EditCommand::SplitClip {
                clip: right,
                at: secs(4),
                new_clip: ClipId::generate(),
            },
        ],
    };
    let edited = applied(&project, &ok);
    assert_eq!(edited.timelines[0].tracks[0].clips.len(), 5);
}

#[test]
fn deep_or_huge_batches_are_refused() {
    let project = sample();
    let mut nested = EditCommand::Batch {
        label: "x".into(),
        commands: Vec::new(),
    };
    for _ in 0..edit::MAX_BATCH_DEPTH {
        nested = EditCommand::Batch {
            label: "x".into(),
            commands: vec![nested],
        };
    }
    assert_eq!(
        rule(&refused(&project, &nested)),
        Some(EditRule::BatchTooLarge)
    );
    let rename = EditCommand::RenameClip {
        clip: id(501),
        name: None,
    };
    let huge = EditCommand::Batch {
        label: "x".into(),
        commands: vec![rename; edit::MAX_COMMANDS_PER_EDIT],
    };
    assert_eq!(
        rule(&refused(&project, &huge)),
        Some(EditRule::BatchTooLarge)
    );
}

#[test]
fn transitions_need_adjacent_clips_and_a_free_cut() {
    let project = sample();
    let add = |from: u32, to: u32| EditCommand::AddTransition {
        track: id(301),
        transition: Transition {
            id: TransitionId::generate(),
            from: id(from),
            to: id(to),
            duration: t(1, 2),
            effect: project.timelines[0].tracks[0].transitions[0].effect.clone(),
            params: Default::default(),
        },
    };
    let edited = applied(&project, &add(502, 503));
    assert_eq!(edited.timelines[0].tracks[0].transitions.len(), 2);
    assert_eq!(
        rule(&refused(&project, &add(501, 502))),
        Some(EditRule::TransitionExists)
    );
    assert_eq!(
        rule(&refused(&project, &add(501, 503))),
        Some(EditRule::NotAdjacent)
    );
    applied(
        &project,
        &EditCommand::RemoveTransition {
            transition: id(701),
        },
    );
}

#[test]
fn clips_cannot_get_shorter_than_their_transitions() {
    let project = sample();
    // The 0.5 s transition 701 starts 502 (5 s to 9 s).
    let trim = EditCommand::TrimClip {
        clip: id(502),
        edge: ClipEdge::End,
        to: t(53, 10),
    };
    assert_eq!(
        rule(&refused(&project, &trim)),
        Some(EditRule::TransitionTooLong)
    );
    let split = EditCommand::SplitClip {
        clip: id(501),
        at: t(48, 10),
        new_clip: ClipId::generate(),
    };
    assert_eq!(
        rule(&refused(&project, &split)),
        Some(EditRule::TransitionTooLong)
    );
}

#[test]
fn markers_stay_ordered() {
    let project = sample();
    let marker = Marker {
        id: MarkerId::generate(),
        start: secs(3),
        duration: Time::ZERO,
        label: "Tengah".into(),
        color: MarkerColor::Green,
    };
    let edited = applied(
        &project,
        &EditCommand::AddMarker {
            timeline: id(201),
            marker: marker.clone(),
        },
    );
    let starts: Vec<Time> = edited.timelines[0]
        .markers
        .iter()
        .map(|m| m.start)
        .collect();
    assert_eq!(starts, vec![secs(0), secs(3), secs(5)]);
    applied(&edited, &EditCommand::RemoveMarker { marker: marker.id });
}

#[test]
fn property_setters_check_what_the_clip_has() {
    let project = sample();
    applied(
        &project,
        &EditCommand::SetOpacity {
            clip: id(507),
            opacity: Animated::constant(0.5),
        },
    );
    applied(
        &project,
        &EditCommand::SetClipEnabled {
            clip: id(503),
            enabled: false,
        },
    );
    let audio = clip(&project, 505).audio.clone().expect("audio");
    let on_title = EditCommand::SetAudioMix {
        clip: id(507),
        audio: audio.clone(),
    };
    assert_eq!(
        rule(&refused(&project, &on_title)),
        Some(EditRule::NotApplicable)
    );
    // Video media may carry sound.
    applied(
        &project,
        &EditCommand::SetAudioMix {
            clip: id(502),
            audio,
        },
    );
    let text = EditCommand::SetText {
        clip: id(508),
        content: match &clip(&project, 507).content {
            ClipContent::Text(text) => text.clone(),
            _ => panic!("text"),
        },
    };
    assert_eq!(
        rule(&refused(&project, &text)),
        Some(EditRule::NotApplicable)
    );
    // Setting a value to what it is changes nothing and needs no undo step.
    let mut unchanged = project.clone();
    let patch = edit::apply(
        &mut unchanged,
        &EditCommand::SetClipEnabled {
            clip: id(503),
            enabled: true,
        },
    )
    .expect("applies");
    assert!(patch.is_empty());
}

#[test]
fn the_validator_catches_what_commands_do_not_check() {
    let project = sample();
    // A parameter whose keyframes change type is a format error.
    let error = refused(
        &project,
        &EditCommand::SetEffectParam {
            clip: id(501),
            effect: id(601),
            param: "radius".into(),
            value: Animated::Keyframes {
                keyframes: vec![
                    kinora_project::value::Keyframe {
                        at: Time::ZERO,
                        value: ParamValue::Float(1.0),
                        easing: Default::default(),
                    },
                    kinora_project::value::Keyframe {
                        at: secs(1),
                        value: ParamValue::Bool(true),
                        easing: Default::default(),
                    },
                ],
            },
        },
    );
    assert!(matches!(error, EditError::Invalid { .. }), "{error:?}");
    // A clip of the wrong kind for its track.
    let mut title = clip(&project, 507).clone();
    title.id = ClipId::generate();
    title.start = secs(30);
    let error = refused(
        &project,
        &EditCommand::InsertClip {
            track: id::<TrackId>(301),
            clip: Box::new(title),
        },
    );
    assert!(matches!(error, EditError::Invalid { .. }), "{error:?}");
}
