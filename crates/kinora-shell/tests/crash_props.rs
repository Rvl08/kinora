//! Property test: random sessions of edits, undo, redo, saves, crashes and
//! closes on a real file system. After every crash or close, opening the
//! folder again gives back exactly the document, revision and unsaved state
//! the session had.

mod common;

use common::{Env, add_track_command, create, open};
use kinora_ipc::IpcError;
use kinora_ipc::command::{ApplyCommandArgs, CloseProjectArgs, HistoryArgs, SaveProjectArgs};
use kinora_project::EditCommand;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Action {
    Add,
    Rename(usize),
    Undo,
    Redo,
    Save,
    Crash,
    CloseDiscarding,
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => Just(Action::Add),
        2 => any::<usize>().prop_map(Action::Rename),
        2 => Just(Action::Undo),
        1 => Just(Action::Redo),
        1 => Just(Action::Save),
        1 => Just(Action::Crash),
        1 => Just(Action::CloseDiscarding),
    ]
}

proptest! {
    // Every case touches the disk and syncs it many times.
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn reopening_restores_the_session(actions in proptest::collection::vec(action(), 1..24)) {
        let env = Env::new();
        let folder = env.folder("Props");
        let mut shell = env.shell();
        create(&mut shell, &env, "Props");
        for action in actions {
            let snapshot = shell.get_project().expect("open");
            let base_revision = snapshot.state.revision;
            let tracks = &snapshot.document.timelines[0].tracks;
            match action {
                Action::Add => {
                    let command = add_track_command(snapshot.document.timelines[0].id, "T");
                    shell
                        .apply_command(ApplyCommandArgs { base_revision, command })
                        .expect("adds a track");
                }
                Action::Rename(seed) => {
                    if tracks.is_empty() {
                        continue;
                    }
                    let command = EditCommand::UpdateTrack {
                        track: tracks[seed % tracks.len()].id,
                        name: Some(format!("T{}", seed % 7)),
                        muted: None,
                        hidden: None,
                        locked: None,
                    };
                    shell
                        .apply_command(ApplyCommandArgs { base_revision, command })
                        .expect("renames a track");
                }
                Action::Undo => {
                    let result = shell.undo(HistoryArgs { base_revision });
                    prop_assert!(
                        matches!(result, Ok(_) | Err(IpcError::NothingToUndo)),
                        "{result:?}"
                    );
                }
                Action::Redo => {
                    let result = shell.redo(HistoryArgs { base_revision });
                    prop_assert!(
                        matches!(result, Ok(_) | Err(IpcError::NothingToRedo)),
                        "{result:?}"
                    );
                }
                Action::Save => {
                    shell.save_project(SaveProjectArgs { save_as: None }).expect("saves");
                }
                Action::Crash | Action::CloseDiscarding => {
                    if matches!(action, Action::CloseDiscarding) {
                        shell
                            .close_project(CloseProjectArgs { discard_changes: true })
                            .expect("closes");
                    }
                    drop(shell);
                    shell = env.shell();
                    let opened = open(&mut shell, &folder).expect("reopens");
                    prop_assert_eq!(&opened.snapshot.document, &snapshot.document);
                    prop_assert_eq!(opened.snapshot.state.dirty, snapshot.state.dirty);
                    prop_assert_eq!(opened.snapshot.state.revision, base_revision);
                }
            }
        }
    }
}
