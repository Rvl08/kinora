//! Regression tests for defects found in review of the editing core.

use kinora_core::{Rational, Time};
use kinora_project::command::ClipEdge;
use kinora_project::edit::{self, Editor, journal};
use kinora_project::id::TrackId;
use kinora_project::model::{Speed, Track, TrackKind};
use kinora_project::{EditCommand, Project, ProjectSettings, Revision, from_json, to_json};

const SAMPLE: &str = include_str!("fixtures/v0/sample.kinora/project.json");

fn id<T: std::str::FromStr>(suffix: u32) -> T
where
    T::Err: std::fmt::Debug,
{
    format!("0192f1c4-0000-7000-8000-000000000{suffix}")
        .parse()
        .expect("sample id")
}

fn rename(clip: u32, name: &str) -> EditCommand {
    EditCommand::RenameClip {
        clip: id(clip),
        name: Some(name.into()),
    }
}

/// Opens `file` with `journal_text` the way the shell does.
fn recover(file: &str, journal_text: &str) -> (Editor, edit::Replay) {
    let project = from_json(file).expect("valid file");
    let recovered = journal::read(
        journal_text,
        project.id,
        &journal::fingerprint(file.as_bytes()),
    )
    .expect("the journal continues the file");
    Editor::recover(project, recovered).expect("valid")
}

/// Review finding 1: after a recovery that stopped early (a torn last line),
/// entries appended to the old file sat behind the damaged line and were lost
/// by the next recovery. Recovery now hands back the clean journal to write.
#[test]
fn edits_after_a_partial_recovery_survive_the_next_crash() {
    let file = SAMPLE.to_owned();
    let mut editor = Editor::new(from_json(&file).expect("valid")).expect("valid");
    let mut text = editor
        .begin_journal(journal::fingerprint(file.as_bytes()))
        .to_line()
        .expect("header");
    let first = editor
        .apply(Revision(0), &rename(501, "A"))
        .expect("applies");
    text.push_str(&first.journal.expect("entry").to_line().expect("line"));
    let second = editor
        .apply(Revision(1), &rename(501, "B"))
        .expect("applies");
    let torn = second.journal.expect("entry").to_line().expect("line");
    text.push_str(&torn[..20]);

    let (mut recovered, replay) = recover(&file, &text);
    assert_eq!(replay.replayed, 1);
    let third = recovered
        .apply(recovered.revision(), &rename(502, "C"))
        .expect("applies")
        .journal
        .expect("entry")
        .to_line()
        .expect("line");

    // Appending to the old file would hide the new entry behind the torn
    // line: the next recovery would lose it.
    let naive = format!("{text}{third}");
    assert_eq!(recover(&file, &naive).1.replayed, 1);

    // The shell replaces the journal with the clean one, then appends.
    let mut text = replay.journal;
    text.push_str(&third);

    let (again, replay) = recover(&file, &text);
    assert_eq!(replay.failure, None);
    assert_eq!(replay.replayed, 2);
    assert_eq!(again.project(), recovered.project());
}

/// Review finding 2: recovery used the default history limit, so a session
/// with a larger limit could undo steps the replay had already forgotten.
#[test]
fn recovery_keeps_the_session_history_limit() {
    let project = Project::new("Limit", ProjectSettings::default());
    let file = to_json(&project).expect("valid");
    let timeline = project.timelines[0].id;
    // One more step than the default: the bug showed only above it.
    let limit = Editor::DEFAULT_HISTORY_LIMIT + 1;
    let mut editor = Editor::new(project)
        .expect("valid")
        .with_history_limit(limit);
    let mut text = editor
        .begin_journal(journal::fingerprint(file.as_bytes()))
        .to_line()
        .expect("header");
    let mut append = |outcome: edit::Outcome| {
        text.push_str(&outcome.journal.expect("entry").to_line().expect("line"));
    };
    for n in 0..limit {
        let track = Track {
            id: TrackId::generate(),
            kind: TrackKind::Video,
            name: format!("V{n}"),
            muted: false,
            hidden: false,
            locked: false,
            clips: Vec::new(),
            transitions: Vec::new(),
        };
        let command = EditCommand::AddTrack {
            timeline,
            index: 0,
            track,
        };
        append(editor.apply(editor.revision(), &command).expect("applies"));
    }
    for _ in 0..limit {
        append(editor.undo(editor.revision()).expect("undoes"));
    }
    let (recovered, replay) = recover(&file, &text);
    assert_eq!(replay.failure, None);
    assert_eq!(recovered.project(), editor.project());
    assert_eq!(recovered.state(), editor.state());
}

/// Review finding 3: setting a clip's speed to the speed it already has
/// changed its duration by a few flicks (rounding source → timeline).
#[test]
fn setting_the_current_speed_changes_nothing() {
    let mut project = from_json(SAMPLE).expect("valid");
    let speed = Speed::Constant {
        rate: Rational::new(1, 19).expect("valid"),
    };
    let set = EditCommand::SetSpeed {
        clip: id(509),
        speed,
    };
    edit::apply(&mut project, &set).expect("applies");
    let trim = EditCommand::TrimClip {
        clip: id(509),
        edge: ClipEdge::End,
        to: Time::from_secs(6).expect("valid"),
    };
    edit::apply(&mut project, &trim).expect("applies");
    let before = project.clone();
    let patch = edit::apply(&mut project, &set).expect("applies");
    assert!(patch.is_empty(), "same speed, same clip");
    assert_eq!(project, before);
}

/// Review finding 4: bad transition and marker times came back as the
/// generic `invalidResult` instead of a precise rule.
#[test]
fn transitions_and_markers_get_precise_rules() {
    use kinora_project::edit::{EditError, EditRule};
    use kinora_project::id::{MarkerId, TransitionId};
    use kinora_project::model::{Marker, MarkerColor, Transition};

    let project = from_json(SAMPLE).expect("valid");
    let rule = |command: &EditCommand| {
        let mut edited = project.clone();
        match edit::apply(&mut edited, command)
            .expect_err("refused")
            .root()
        {
            EditError::Rejected { rule, .. } => Some(*rule),
            _ => None,
        }
    };
    let transition = EditCommand::AddTransition {
        track: id(301),
        transition: Transition {
            id: TransitionId::generate(),
            from: id(502),
            to: id(503),
            duration: Time::ZERO,
            effect: project.timelines[0].tracks[0].transitions[0].effect.clone(),
            params: Default::default(),
        },
    };
    assert_eq!(rule(&transition), Some(EditRule::OutOfRange));
    let marker = EditCommand::AddMarker {
        timeline: id(201),
        marker: Marker {
            id: MarkerId::generate(),
            start: Time::from_secs(-1).expect("valid"),
            duration: Time::ZERO,
            label: "Sebelum".into(),
            color: MarkerColor::Red,
        },
    };
    assert_eq!(rule(&marker), Some(EditRule::OutOfRange));
}
