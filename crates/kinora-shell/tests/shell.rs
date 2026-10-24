//! The shell against a real file system: creating, opening and saving
//! projects, the journal and crash recovery, and the recent list.
//!
//! A crash is simulated by dropping the shell without closing the project,
//! then opening the folder in a new shell, as after a restart.

mod common;

use std::fs;

use common::{
    Env, add_track, add_track_command, create, document, open, revision, text, track_names,
};
use kinora_ipc::command::{
    ApplyCommandArgs, CancelJobArgs, CloseProjectArgs, CreateProjectArgs, ForgetRecentProjectArgs,
    HistoryArgs, JobId, SaveProjectArgs,
};
use kinora_ipc::event::ChangeReason;
use kinora_ipc::{Entity, IoErrorKind, IpcError, Revision};
use kinora_project::{CURRENT_SCHEMA_VERSION, ProjectSettings};
use kinora_shell::{DISCARDED_JOURNAL_FILE, JOURNAL_FILE, PROJECT_FILE};

const KEEP: CloseProjectArgs = CloseProjectArgs {
    discard_changes: false,
};
const DISCARD: CloseProjectArgs = CloseProjectArgs {
    discard_changes: true,
};
const IN_PLACE: SaveProjectArgs = SaveProjectArgs { save_as: None };

fn save_as(path: &std::path::Path) -> SaveProjectArgs {
    SaveProjectArgs {
        save_as: Some(text(path)),
    }
}

#[test]
fn create_save_close_and_reopen() {
    let env = Env::new();
    let mut shell = env.shell();
    let opened = create(&mut shell, &env, "Holiday");
    assert_eq!(opened.recovered_edits, 0);
    assert_eq!(opened.migrated_from, None);
    assert!(!opened.snapshot.state.dirty);
    let folder = env.folder("Holiday");
    assert!(folder.join(PROJECT_FILE).is_file());
    assert!(folder.join(JOURNAL_FILE).is_file());

    assert!(add_track(&mut shell, "V1").dirty);
    let saved = shell.save_project(IN_PLACE).expect("saves");
    assert!(!saved.dirty);
    let expected = document(&shell);
    shell.close_project(KEEP).expect("closes");
    assert_eq!(shell.state(), None);
    assert_eq!(env.events.last().map(|e| e.state), Some(None));

    let reopened = open(&mut shell, &folder).expect("opens");
    assert_eq!(reopened.recovered_edits, 0);
    assert!(!reopened.snapshot.state.dirty);
    assert_eq!(reopened.snapshot.document, expected);
    assert_eq!(
        env.events.reasons(),
        [
            ChangeReason::Opened,
            ChangeReason::Edited,
            ChangeReason::Saved,
            ChangeReason::Closed,
            ChangeReason::Opened
        ]
    );
}

#[test]
fn a_crash_keeps_unsaved_edits() {
    let env = Env::new();
    let mut shell = env.shell();
    create(&mut shell, &env, "Crash");
    add_track(&mut shell, "A");
    add_track(&mut shell, "B");
    let expected = document(&shell);
    let at = revision(&shell);
    drop(shell);

    let mut shell = env.shell();
    let opened = open(&mut shell, &env.folder("Crash")).expect("opens");
    assert_eq!(opened.recovered_edits, 2);
    assert_eq!(opened.snapshot.document, expected);
    assert!(opened.snapshot.state.dirty);
    assert_eq!(opened.snapshot.state.revision, at);
    assert_eq!(opened.snapshot.state.undo_label.as_deref(), Some("addTrack"));

    // The recovered history works like the original one.
    shell.undo(HistoryArgs { base_revision: at }).expect("undoes");
    assert_eq!(track_names(&document(&shell)), ["A"]);
}

#[test]
fn a_torn_last_line_is_dropped_and_the_journal_is_replaced() {
    let env = Env::new();
    let folder = env.folder("Torn");
    let journal = folder.join(JOURNAL_FILE);
    let mut shell = env.shell();
    create(&mut shell, &env, "Torn");
    add_track(&mut shell, "A");
    let after_a = document(&shell);
    add_track(&mut shell, "B");
    drop(shell);

    // A crash in the middle of writing B's line.
    let written = fs::read_to_string(&journal).expect("reads");
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines.len(), 3, "header, A and B");
    let torn = &lines[2][..lines[2].len() / 2];
    fs::write(&journal, format!("{}\n{}\n{torn}", lines[0], lines[1])).expect("tears");

    let mut shell = env.shell();
    let opened = open(&mut shell, &folder).expect("opens");
    assert_eq!(opened.recovered_edits, 1);
    assert_eq!(opened.snapshot.document, after_a);
    let replaced = fs::read_to_string(&journal).expect("reads");
    assert_eq!(replaced, format!("{}\n{}\n", lines[0], lines[1]));

    // New entries follow the replayed ones and survive the next crash.
    add_track(&mut shell, "C");
    drop(shell);
    let mut shell = env.shell();
    let opened = open(&mut shell, &folder).expect("opens");
    assert_eq!(opened.recovered_edits, 2);
    assert_eq!(track_names(&opened.snapshot.document), ["C", "A"]);
}

#[test]
fn leftover_temporary_files_are_ignored_and_removed() {
    let env = Env::new();
    let folder = env.folder("Leftover");
    let mut shell = env.shell();
    create(&mut shell, &env, "Leftover");
    add_track(&mut shell, "A");
    shell.save_project(IN_PLACE).expect("saves");
    let saved = document(&shell);
    shell.close_project(KEEP).expect("closes");

    // A crash while saving: the temporary files were written (or half
    // written) but never renamed over their targets.
    let temp_project = folder.join(format!("{PROJECT_FILE}.tmp"));
    let temp_journal = folder.join(format!("{JOURNAL_FILE}.tmp"));
    fs::write(&temp_project, "{ \"schemaVersion\": 0, \"na").expect("writes");
    fs::write(&temp_journal, "garbage").expect("writes");

    let opened = open(&mut shell, &folder).expect("opens");
    assert_eq!(opened.snapshot.document, saved);
    assert_eq!(opened.recovered_edits, 0);
    assert!(!temp_project.exists());
    assert!(!temp_journal.exists());
}

/// edit-core-report §3.3: save, undo past the save, edit, crash. The undo
/// needs the step recorded in the journal, or the later edits are lost.
#[test]
fn undo_past_a_save_then_a_crash() {
    let env = Env::new();
    let mut shell = env.shell();
    let original = create(&mut shell, &env, "Undo").snapshot.document;
    add_track(&mut shell, "A");
    shell.save_project(IN_PLACE).expect("saves");
    let undone = shell
        .undo(HistoryArgs {
            base_revision: revision(&shell),
        })
        .expect("undoes A");
    assert!(undone.dirty);
    add_track(&mut shell, "B");
    add_track(&mut shell, "C");
    let expected = document(&shell);
    drop(shell);

    let mut shell = env.shell();
    let opened = open(&mut shell, &env.folder("Undo")).expect("opens");
    assert_eq!(opened.recovered_edits, 3);
    assert_eq!(opened.snapshot.document, expected);
    assert!(opened.snapshot.state.dirty);

    // Undoing C and B returns to the document without A, which differs
    // from the saved file.
    for _ in 0..2 {
        let base_revision = revision(&shell);
        shell.undo(HistoryArgs { base_revision }).expect("undoes");
    }
    assert_eq!(document(&shell), original);
    assert!(shell.state().expect("open").dirty);
}

/// A crash between replacing `project.json` and restarting the journal
/// leaves an old journal that no longer matches the file.
#[test]
fn a_stale_journal_is_set_aside() {
    let env = Env::new();
    let folder = env.folder("Stale");
    let journal = folder.join(JOURNAL_FILE);
    let mut shell = env.shell();
    create(&mut shell, &env, "Stale");
    add_track(&mut shell, "A");
    let old_journal = fs::read(&journal).expect("reads");
    shell.save_project(IN_PLACE).expect("saves");
    let saved = document(&shell);
    drop(shell);
    fs::write(&journal, &old_journal).expect("restores the old journal");

    let mut shell = env.shell();
    let opened = open(&mut shell, &folder).expect("opens");
    assert_eq!(opened.recovered_edits, 0);
    assert_eq!(opened.snapshot.document, saved);
    assert!(!opened.snapshot.state.dirty);
    let discarded = fs::read(folder.join(DISCARDED_JOURNAL_FILE)).expect("kept aside");
    assert_eq!(discarded, old_journal);
}

#[test]
fn unsaved_changes_block_closing_and_switching() {
    let env = Env::new();
    let mut shell = env.shell();
    create(&mut shell, &env, "Dirty");
    add_track(&mut shell, "A");
    assert_eq!(shell.close_project(KEEP), Err(IpcError::UnsavedChanges));
    assert!(shell.state().is_some());
    let other = CreateProjectArgs {
        parent_dir: text(&env.projects()),
        name: "Other".into(),
        settings: ProjectSettings::default(),
    };
    assert_eq!(
        shell.create_project(other),
        Err(IpcError::UnsavedChanges)
    );
    assert!(!env.folder("Other").exists());

    // Discarded changes stay recoverable until the next open.
    shell.close_project(DISCARD).expect("closes");
    shell.close_project(KEEP).expect("closing nothing is fine");
    let opened = open(&mut shell, &env.folder("Dirty")).expect("opens");
    assert_eq!(opened.recovered_edits, 1);
    assert!(opened.snapshot.state.dirty);

    // A clean close leaves nothing to recover.
    shell.save_project(IN_PLACE).expect("saves");
    shell.close_project(KEEP).expect("closes");
    let opened = open(&mut shell, &env.folder("Dirty")).expect("opens");
    assert_eq!(opened.recovered_edits, 0);
}

#[test]
fn stale_revisions_are_refused() {
    let env = Env::new();
    let mut shell = env.shell();
    let opened = create(&mut shell, &env, "Rev");
    let timeline = opened.snapshot.document.timelines[0].id;
    add_track(&mut shell, "A");
    let stale = ApplyCommandArgs {
        base_revision: Revision(0),
        command: add_track_command(timeline, "B"),
    };
    let conflict = IpcError::RevisionConflict {
        expected: Revision(0),
        current: Revision(1),
    };
    assert_eq!(shell.apply_command(stale), Err(conflict.clone()));
    let undo = HistoryArgs {
        base_revision: Revision(0),
    };
    assert_eq!(shell.undo(undo), Err(conflict));
    assert_eq!(track_names(&document(&shell)), ["A"]);
}

#[test]
fn commands_need_an_open_project() {
    let env = Env::new();
    let mut shell = env.shell();
    assert_eq!(shell.get_project(), Err(IpcError::NoProject));
    assert_eq!(shell.save_project(IN_PLACE), Err(IpcError::NoProject));
    let history = HistoryArgs {
        base_revision: Revision(0),
    };
    assert_eq!(shell.undo(history.clone()), Err(IpcError::NoProject));
    assert_eq!(shell.redo(history), Err(IpcError::NoProject));
    let job = JobId::generate();
    assert_eq!(
        shell.cancel_job(CancelJobArgs { job }),
        Err(IpcError::NotFound {
            entity: Entity::Job,
            id: job.to_string()
        })
    );
}

#[test]
fn create_checks_the_name_and_the_parent_folder() {
    let env = Env::new();
    let mut shell = env.shell();
    let args = |parent: String, name: &str| CreateProjectArgs {
        parent_dir: parent,
        name: name.to_owned(),
        settings: ProjectSettings::default(),
    };
    let parent = text(&env.projects());
    for name in ["", "a/b", "..", "CON", "trailing.", " x", "x\u{202E}y"] {
        let result = shell.create_project(args(parent.clone(), name));
        assert!(
            matches!(&result, Err(IpcError::InvalidArgument { argument, .. }) if argument == "name"),
            "{name:?}: {result:?}"
        );
    }
    let result = shell.create_project(args("relative".into(), "A"));
    assert!(
        matches!(&result, Err(IpcError::InvalidArgument { argument, .. }) if argument == "parentDir"),
        "{result:?}"
    );
    let missing = text(&env.projects().join("missing"));
    assert_eq!(
        shell.create_project(args(missing, "A")).map(drop),
        Err(IpcError::Io {
            kind: IoErrorKind::NotFound
        })
    );
    let file = env.projects().join("file.txt");
    fs::write(&file, "x").expect("writes");
    assert_eq!(
        shell.create_project(args(text(&file), "A")).map(drop),
        Err(IpcError::Io {
            kind: IoErrorKind::WrongKind
        })
    );

    // An existing folder is never reused or overwritten.
    create(&mut shell, &env, "Same");
    add_track(&mut shell, "kept");
    shell.save_project(IN_PLACE).expect("saves");
    assert_eq!(
        shell.create_project(args(parent, "Same")).map(drop),
        Err(IpcError::Io {
            kind: IoErrorKind::AlreadyExists
        })
    );
    assert_eq!(track_names(&document(&shell)), ["kept"]);
}

#[test]
fn open_accepts_only_project_folders() {
    let env = Env::new();
    let mut shell = env.shell();
    let empty = env.projects().join("Empty.kinora");
    fs::create_dir(&empty).expect("folder");
    assert!(matches!(
        open(&mut shell, &empty),
        Err(IpcError::ProjectInvalid { .. })
    ));

    let file = env.projects().join("file.txt");
    fs::write(&file, "x").expect("writes");
    assert_eq!(
        open(&mut shell, &file).map(drop),
        Err(IpcError::Io {
            kind: IoErrorKind::WrongKind
        })
    );
    assert_eq!(
        open(&mut shell, &env.projects().join("missing")).map(drop),
        Err(IpcError::Io {
            kind: IoErrorKind::NotFound
        })
    );
    let relative = shell.open_project(kinora_ipc::command::OpenProjectArgs {
        path: "Empty.kinora".into(),
    });
    assert!(matches!(
        relative,
        Err(IpcError::InvalidArgument { .. })
    ));

    fs::write(empty.join(PROJECT_FILE), "{ not json").expect("writes");
    assert!(matches!(
        open(&mut shell, &empty),
        Err(IpcError::ProjectInvalid { .. })
    ));
    let newer = format!(r#"{{"schemaVersion": {}}}"#, CURRENT_SCHEMA_VERSION + 1);
    fs::write(empty.join(PROJECT_FILE), newer).expect("writes");
    assert!(matches!(
        open(&mut shell, &empty),
        Err(IpcError::ProjectTooNew { .. })
    ));
    assert_eq!(shell.state(), None);
}

#[test]
fn save_as_continues_in_the_copy() {
    let env = Env::new();
    let mut shell = env.shell();
    let original = create(&mut shell, &env, "First").snapshot.document;
    add_track(&mut shell, "A");

    // Without the extension; the dialog may leave it out.
    let saved = shell
        .save_project(save_as(&env.projects().join("Second")))
        .expect("saves a copy");
    assert!(!saved.dirty);
    let second = env.folder("Second");
    assert!(second.join(PROJECT_FILE).is_file());
    assert!(!env.folder("First").join(JOURNAL_FILE).exists());

    // Edits now go to the copy's journal.
    add_track(&mut shell, "B");
    drop(shell);
    let mut shell = env.shell();
    let opened = open(&mut shell, &second).expect("opens");
    assert_eq!(opened.recovered_edits, 1);
    assert_eq!(track_names(&opened.snapshot.document), ["B", "A"]);

    // The first folder keeps its last saved state.
    shell.close_project(DISCARD).expect("closes");
    let first = open(&mut shell, &env.folder("First")).expect("opens");
    assert_eq!(first.snapshot.document, original);
    assert_eq!(first.recovered_edits, 0);

    // Another project's folder is never overwritten; the current folder
    // means a plain save.
    assert_eq!(
        shell.save_project(save_as(&second)),
        Err(IpcError::Io {
            kind: IoErrorKind::AlreadyExists
        })
    );
    shell
        .save_project(save_as(&env.folder("First")))
        .expect("saves in place");
    let bad = shell.save_project(save_as(&env.projects().join("CON")));
    assert!(
        matches!(&bad, Err(IpcError::InvalidArgument { argument, .. }) if argument == "saveAs"),
        "{bad:?}"
    );
}

#[test]
fn the_recent_list_follows_opens_moves_and_forgets() {
    let env = Env::new();
    let mut shell = env.shell();
    create(&mut shell, &env, "A");
    create(&mut shell, &env, "B");
    let resolved = |name: &str| text(&dunce::canonicalize(env.folder(name)).expect("exists"));
    let listed = shell.list_recent_projects().expect("lists");
    let paths: Vec<&str> = listed.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(paths, [resolved("B"), resolved("A")]);
    assert!(listed.iter().all(|p| p.available));
    assert_eq!(listed[0].name, "B");
    assert_eq!(listed[0].opened_at, 1_000);

    // Another spelling of the open folder opens nothing new.
    let spelled = env.projects().join("..").join("projects").join("B.kinora");
    open(&mut shell, &spelled).expect("opens");
    assert_eq!(shell.list_recent_projects().expect("lists").len(), 2);

    // "Find folder": A was moved; the UI opens the new place, then forgets
    // the old path.
    let old_a = resolved("A");
    fs::rename(env.folder("A"), env.folder("A moved")).expect("moves");
    let listed = shell.list_recent_projects().expect("lists");
    assert!(!listed[1].available, "{listed:?}");
    open(&mut shell, &env.folder("A moved")).expect("opens");
    let forget = |path: String| ForgetRecentProjectArgs { path };
    shell.forget_recent_project(forget(old_a)).expect("forgets");
    shell
        .forget_recent_project(forget("never listed".into()))
        .expect("absent is fine");
    // The open project is never forgotten.
    let open_path = resolved("A moved");
    shell
        .forget_recent_project(forget(open_path.clone()))
        .expect("ignored");
    let listed = shell.list_recent_projects().expect("lists");
    let paths: Vec<&str> = listed.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(paths, [open_path, resolved("B")]);

    // A new app start reads the same list.
    let mut restarted = env.shell();
    assert_eq!(restarted.list_recent_projects().expect("lists"), listed);
}
