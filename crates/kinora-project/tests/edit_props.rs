//! Property tests of the editing session: random edit sequences, with undo,
//! redo and saves in between, must undo exactly, redo exactly, and recover
//! from the last save plus the journal to the very same document.

use kinora_core::{FrameRate, Rational, Time};
use kinora_project::command::ClipEdge;
use kinora_project::edit::{Editor, journal};
use kinora_project::id::ClipId;
use kinora_project::model::{Clip, Speed};
use kinora_project::value::Animated;
use kinora_project::{EditCommand, Project, from_json, to_json};
use proptest::prelude::*;

const SAMPLE: &str = include_str!("fixtures/v0/sample.kinora/project.json");

/// One step of a generated session, interpreted against the current
/// document so it mostly hits real clips.
#[derive(Clone, Debug)]
enum Action {
    Edit(Op),
    /// Two edits as one batch.
    Batch(Op, Op),
    Undo,
    Redo,
    Save,
}

#[derive(Clone, Debug)]
struct Op {
    kind: u8,
    /// Picks a clip (and a destination track) by index modulo the count.
    pick: usize,
    other: usize,
    /// A time: whole frames at 30 fps plus some 48 kHz samples, so
    /// sub-frame audio positions are exercised too.
    frames: i64,
    samples: i64,
    rate: usize,
}

fn op() -> impl Strategy<Value = Op> {
    (
        0_u8..10,
        any::<usize>(),
        any::<usize>(),
        0_i64..=600,
        0_i64..1600,
        0_usize..5,
    )
        .prop_map(|(kind, pick, other, frames, samples, rate)| Op {
            kind,
            pick,
            other,
            frames,
            samples,
            rate,
        })
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        6 => op().prop_map(Action::Edit),
        1 => (op(), op()).prop_map(|(a, b)| Action::Batch(a, b)),
        2 => Just(Action::Undo),
        1 => Just(Action::Redo),
        1 => Just(Action::Save),
    ]
}

fn clips(project: &Project) -> Vec<(&Clip, kinora_project::id::TrackId)> {
    project
        .timelines
        .iter()
        .flat_map(|t| &t.tracks)
        .flat_map(|k| k.clips.iter().map(move |c| (c, k.id)))
        .collect()
}

fn time(op: &Op) -> Time {
    let frames = Time::from_frames(op.frames, FrameRate::FPS_30).expect("in range");
    let samples = Time::new(op.samples, 48_000).expect("in range");
    frames.checked_add(samples).expect("in range")
}

/// Turns a generated op into a command on the current document. Commands
/// may well be refused; that is part of what is tested.
fn command(project: &Project, op: &Op) -> Option<EditCommand> {
    let all = clips(project);
    let (clip, track) = *all.get(op.pick % all.len().max(1))?;
    let tracks: Vec<_> = project
        .timelines
        .iter()
        .flat_map(|t| &t.tracks)
        .map(|k| k.id)
        .collect();
    let at = time(op);
    let rates = [(1, 2), (2, 1), (3, 2), (1, 3), (1, 1)];
    let (num, den) = rates[op.rate];
    Some(match op.kind {
        0 => EditCommand::SplitClip {
            clip: clip.id,
            at,
            new_clip: ClipId::generate(),
        },
        1 => EditCommand::TrimClip {
            clip: clip.id,
            edge: ClipEdge::Start,
            to: at,
        },
        2 => EditCommand::TrimClip {
            clip: clip.id,
            edge: ClipEdge::End,
            to: at,
        },
        3 => EditCommand::MoveClip {
            clip: clip.id,
            track: tracks[op.other % tracks.len()],
            start: at,
        },
        4 => EditCommand::RemoveClip {
            clip: clip.id,
            ripple: op.other.is_multiple_of(2),
        },
        5 => EditCommand::SetSpeed {
            clip: clip.id,
            speed: Speed::Constant {
                rate: Rational::new(num, den).expect("valid"),
            },
        },
        6 => EditCommand::RenameClip {
            clip: clip.id,
            name: Some(format!("Klip {}", op.other % 100)),
        },
        7 => EditCommand::SetOpacity {
            clip: clip.id,
            opacity: Animated::constant((op.other % 11) as f64 / 10.0),
        },
        8 => {
            let mut copy = clip.clone();
            copy.id = ClipId::generate();
            copy.start = at;
            if let Some(visual) = &mut copy.visual {
                // Effect ids must be fresh too.
                for effect in &mut visual.effects {
                    effect.id = kinora_project::id::EffectInstanceId::generate();
                }
            }
            EditCommand::InsertClip {
                track,
                clip: Box::new(copy),
            }
        }
        _ => EditCommand::UpdateTrack {
            track: tracks[op.other % tracks.len()],
            name: None,
            muted: Some(op.other.is_multiple_of(2)),
            hidden: None,
            locked: Some(op.other.is_multiple_of(7)),
        },
    })
}

/// The shell's side of saving: write the file, mark it saved, start a new
/// journal.
fn save(editor: &mut Editor) -> (String, String) {
    let file = to_json(editor.project()).expect("an edited document always saves");
    editor.mark_saved();
    let header = editor.begin_journal(journal::fingerprint(file.as_bytes()));
    (file, header.to_line().expect("header"))
}

proptest! {
    #[test]
    fn sessions_undo_redo_and_recover_exactly(actions in prop::collection::vec(action(), 1..40)) {
        let original = from_json(SAMPLE).expect("the sample is valid");
        let mut editor = Editor::new(original.clone()).expect("valid");
        let (mut saved_file, mut journal_text) = save(&mut editor);

        for action in &actions {
            let base = editor.revision();
            let before = editor.project().clone();
            let outcome = match action {
                Action::Edit(op) => command(editor.project(), op).map(|c| editor.apply(base, &c)),
                Action::Batch(a, b) => {
                    let commands: Vec<_> = [a, b].iter().filter_map(|op| command(editor.project(), op)).collect();
                    Some(editor.apply(base, &EditCommand::Batch { label: "Batch".into(), commands }))
                }
                Action::Undo => Some(editor.undo(base)),
                Action::Redo => Some(editor.redo(base)),
                Action::Save => {
                    (saved_file, journal_text) = save(&mut editor);
                    prop_assert!(!editor.state().dirty);
                    None
                }
            };
            match outcome {
                Some(Ok(outcome)) => {
                    if let Some(entry) = outcome.journal {
                        journal_text.push_str(&entry.to_line().expect("entry"));
                    } else {
                        prop_assert_eq!(editor.project(), &before, "no journal entry means no change");
                    }
                }
                Some(Err(_)) => prop_assert_eq!(editor.project(), &before, "a refusal changes nothing"),
                None => {}
            }
        }

        let final_project = editor.project().clone();
        let final_state = editor.state();
        // The result is a valid document that saves and reloads unchanged.
        let json = to_json(&final_project).expect("valid");
        prop_assert_eq!(from_json(&json).expect("reads back"), final_project.clone());

        // Recovery: the last saved file plus the journal give the same
        // document and the same undo and redo labels.
        let disk = from_json(&saved_file).expect("the saved file is valid");
        let recovered = journal::read(&journal_text, disk.id, &journal::fingerprint(saved_file.as_bytes()))
            .expect("the journal continues the saved file");
        prop_assert_eq!(recovered.stop, None);
        let (mut replayed, report) = Editor::recover(disk, recovered).expect("valid");
        prop_assert_eq!(&report.failure, &None);
        prop_assert_eq!(replayed.project(), &final_project);
        let state = replayed.state();
        prop_assert_eq!(state.revision, final_state.revision);
        prop_assert_eq!(state.dirty, final_state.dirty, "unsaved changes survive recovery");
        // The journal starts at the last save, so it rebuilds the history
        // made since then; without a save that is the whole history.
        if !actions.iter().any(|a| matches!(a, Action::Save)) {
            prop_assert_eq!(&state.undo_label, &final_state.undo_label);
            prop_assert_eq!(&state.redo_label, &final_state.redo_label);
        }

        // Undoing everything restores the original; redoing as many steps
        // restores the end (further redos would replay steps that were
        // undone during the session).
        let mut undone = 0;
        while editor.undo(editor.revision()).is_ok() {
            undone += 1;
        }
        prop_assert_eq!(editor.project(), &original);
        for _ in 0..undone {
            editor.redo(editor.revision()).expect("redoes what was undone");
        }
        prop_assert_eq!(editor.project(), &final_project);

        // The recovered session can keep undoing too, as far as its history
        // reaches, without ever producing an invalid document.
        while replayed.undo(replayed.revision()).is_ok() {
            prop_assert!(kinora_project::validate(replayed.project()).is_empty());
        }
    }
}
