//! The editing session: the open document, its revision, undo and redo.

use std::collections::VecDeque;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::command::EditCommand;
use crate::model::Project;
use crate::validate::{Issue, validate};

use super::journal::{self, Entry, Header, RecordedStep, Recovered};
use super::{EditError, Patch, apply};
use crate::version::CURRENT_SCHEMA_VERSION;

/// Counter of document states in the current session: every applied command,
/// undo or redo increments it. Requests that edit carry the revision they were
/// made against, so an edit computed from a stale UI state is rejected with
/// [`HistoryError::RevisionConflict`] instead of being applied to the wrong
/// document. Not stored in `project.json`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Revision(pub u32);

impl Revision {
    /// The next revision, or `None` after `u32::MAX` edits.
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Why a session operation failed. The document is unchanged on error.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HistoryError {
    /// The request was made against an older revision.
    #[error("request made against revision {expected}, the document is at {current}")]
    RevisionConflict {
        /// The revision the request carried.
        expected: Revision,
        /// The current revision.
        current: Revision,
    },
    /// There is nothing to undo.
    #[error("nothing to undo")]
    NothingToUndo,
    /// There is nothing to redo.
    #[error("nothing to redo")]
    NothingToRedo,
    /// The session reached `u32::MAX` revisions; save and reopen.
    #[error("the session has run out of revisions")]
    RevisionsExhausted,
    /// The command was refused.
    #[error(transparent)]
    Edit(#[from] EditError),
}

/// The session state the UI shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorState {
    /// Current revision.
    pub revision: Revision,
    /// `true` when the document differs from the last saved (or opened) one.
    pub dirty: bool,
    /// Label of the step undo would revert.
    pub undo_label: Option<String>,
    /// Label of the step redo would re-apply.
    pub redo_label: Option<String>,
}

/// The result of a successful [`Editor::apply`], [`Editor::undo`] or
/// [`Editor::redo`].
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The state after the operation.
    pub state: EditorState,
    /// What to append to the journal, or `None` when nothing changed (a
    /// command that was a no-op). The shell writes exactly these entries, so
    /// the journal always replays to the same revisions.
    pub journal: Option<Entry>,
}

/// One undo step.
///
/// Every document state of a session has an identifier; a step leads from
/// `before` to `after`. The editor tracks the current state's identifier, so
/// "unsaved changes" is simply "the current state is not the saved one",
/// whatever mix of undo, redo, forgotten history and journal replay led
/// there.
#[derive(Clone, Debug)]
pub struct Step {
    /// When the step was made, from the same counter as state identifiers;
    /// tells whether the current journal can rebuild it.
    created: u64,
    /// The state the step starts from.
    before: u64,
    /// The state the step leads to.
    after: u64,
    /// Undo history label.
    label: String,
    /// Applying it undoes the step (on the undo stack) or redoes it (on the
    /// redo stack).
    patch: Patch,
}

impl Step {
    /// The undo history label.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// An open document with its editing history.
///
/// Every change goes through [`Editor::apply`], [`Editor::undo`] or
/// [`Editor::redo`], each guarded by the caller's revision; this is the one
/// editing path for the UI, the Copilot, the local API and scripts
/// (ADR-0004).
#[derive(Debug)]
pub struct Editor {
    project: Project,
    revision: Revision,
    /// Oldest step first.
    undo: VecDeque<Step>,
    /// Most recently undone step last.
    redo: Vec<Step>,
    /// Source of state and step identifiers.
    next_id: u64,
    /// The current document state.
    current: u64,
    /// The state last saved (or opened).
    saved: u64,
    /// Steps created before this were made before the current journal
    /// began, so replaying the journal cannot rebuild them.
    journal_floor: u64,
    history_limit: usize,
}

/// The result of [`Editor::recover`].
#[derive(Clone, Debug, PartialEq)]
pub struct Replay {
    /// How many journal entries were replayed.
    pub replayed: usize,
    /// The entry that could not be replayed (its index in the journal's
    /// entries) and why. Replay stops there; the editor holds the state just
    /// before it.
    pub failure: Option<(usize, HistoryError)>,
}

impl Editor {
    /// Undo steps kept by default; older ones are forgotten.
    pub const DEFAULT_HISTORY_LIMIT: usize = 500;

    /// Opens a session on a document, at revision 0 and clean.
    ///
    /// # Errors
    /// Every rule the document breaks; the editor only works on valid
    /// documents.
    pub fn new(project: Project) -> Result<Self, Vec<Issue>> {
        let issues = validate(&project);
        if !issues.is_empty() {
            return Err(issues);
        }
        Ok(Self {
            project,
            revision: Revision(0),
            undo: VecDeque::new(),
            redo: Vec::new(),
            next_id: 1,
            current: 0,
            saved: 0,
            journal_floor: 0,
            history_limit: Self::DEFAULT_HISTORY_LIMIT,
        })
    }

    /// Keeps at most `limit` undo steps (at least one).
    #[must_use]
    pub fn with_history_limit(mut self, limit: usize) -> Self {
        self.history_limit = limit.max(1);
        self.trim_history();
        self
    }

    /// The current document.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Ends the session and returns the document.
    pub fn into_project(self) -> Project {
        self.project
    }

    /// The current revision.
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// The state the UI shows.
    pub fn state(&self) -> EditorState {
        EditorState {
            revision: self.revision,
            dirty: self.saved != self.current,
            undo_label: self.undo.back().map(|s| s.label.clone()),
            redo_label: self.redo.last().map(|s| s.label.clone()),
        }
    }

    /// Records that the current document was written to disk.
    pub fn mark_saved(&mut self) {
        self.saved = self.current;
    }

    /// Starts a new journal for the `project.json` whose bytes have the
    /// [`fingerprint`](journal::fingerprint) `base`: call it after opening
    /// and after every save, then write the returned header as the journal's
    /// first line, replacing the old journal.
    pub fn begin_journal(&mut self, base: String) -> Header {
        self.journal_floor = self.next_id;
        Header {
            format: journal::FORMAT,
            schema_version: CURRENT_SCHEMA_VERSION,
            project: self.project.id,
            base,
            revision: self.revision,
        }
    }

    /// Opens a session on a saved document and replays its journal (see
    /// [`journal::read`]), rebuilding the edits and undo history of a session
    /// that ended without saving. The shell keeps appending to the same
    /// journal afterwards.
    ///
    /// Every entry goes through the same checks as a live edit, so a damaged
    /// or hostile journal cannot produce an invalid document: replay stops
    /// at the first entry that fails and reports it.
    ///
    /// # Errors
    /// The issues of an invalid `project`.
    pub fn recover(project: Project, journal: Recovered) -> Result<(Self, Replay), Vec<Issue>> {
        let mut editor = Self::new(project)?;
        editor.revision = journal.header.revision;
        let mut replay = Replay {
            replayed: 0,
            failure: None,
        };
        for (index, entry) in journal.entries.into_iter().enumerate() {
            let expected = entry.revision();
            match editor.replay_entry(entry) {
                Ok(()) if editor.revision == expected => replay.replayed += 1,
                Ok(()) => {
                    // A journalled command never is a no-op; this one was.
                    replay.failure = Some((
                        index,
                        HistoryError::RevisionConflict {
                            expected,
                            current: editor.revision,
                        },
                    ));
                    break;
                }
                Err(error) => {
                    replay.failure = Some((index, error));
                    break;
                }
            }
        }
        Ok((editor, replay))
    }

    /// Replays one entry. The live undo stack is the part from before the
    /// journal (not rebuilt) with the replayed part on top, and likewise for
    /// redo; so an undo or redo takes the replayed stack while it has steps,
    /// and the step recorded in the entry only once it is empty.
    fn replay_entry(&mut self, entry: Entry) -> Result<(), HistoryError> {
        let base = self.revision;
        match entry {
            Entry::Apply { command, .. } => self.apply(base, &command).map(drop),
            Entry::Undo { step, .. } => match step {
                Some(step) if self.undo.is_empty() => {
                    let next = self.next_revision()?;
                    let redo = self.apply_recorded(step.patch)?;
                    let before = self.new_id();
                    self.redo.push(Step {
                        created: before,
                        before,
                        after: self.current,
                        label: step.label,
                        patch: redo,
                    });
                    self.current = before;
                    self.revision = next;
                    Ok(())
                }
                _ => self.undo(base).map(drop),
            },
            Entry::Redo { step, .. } => match step {
                Some(step) if self.redo.is_empty() => {
                    let next = self.next_revision()?;
                    let undo = self.apply_recorded(step.patch)?;
                    let after = self.new_id();
                    self.undo.push_back(Step {
                        created: after,
                        before: self.current,
                        after,
                        label: step.label,
                        patch: undo,
                    });
                    self.current = after;
                    self.revision = next;
                    self.trim_history();
                    Ok(())
                }
                _ => self.redo(base).map(drop),
            },
        }
    }

    /// Applies a patch read from the journal, which is untrusted: the result
    /// must pass the validator or it is reverted.
    fn apply_recorded(&mut self, patch: Patch) -> Result<Patch, HistoryError> {
        let inverse = patch.apply(&mut self.project)?;
        let issues = validate(&self.project);
        if issues.is_empty() {
            Ok(inverse)
        } else {
            // The inverse of a patch that was just applied restores the
            // previous document.
            let _ = inverse.apply(&mut self.project);
            Err(EditError::Invalid { issues }.into())
        }
    }

    fn new_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn next_revision(&self) -> Result<Revision, HistoryError> {
        self.revision.next().ok_or(HistoryError::RevisionsExhausted)
    }

    /// The journal record of a step being undone or redone: the step itself
    /// when the journal cannot rebuild it.
    fn recorded(&self, step: &Step) -> Option<RecordedStep> {
        (step.created < self.journal_floor).then(|| RecordedStep {
            label: step.label.clone(),
            patch: step.patch.clone(),
        })
    }

    /// Applies a command as one undo step and clears the redo history.
    ///
    /// # Errors
    /// [`HistoryError::RevisionConflict`] when `base` is not the current
    /// revision, [`HistoryError::Edit`] when the command is refused.
    pub fn apply(
        &mut self,
        base: Revision,
        command: &EditCommand,
    ) -> Result<Outcome, HistoryError> {
        self.require_revision(base)?;
        let next = self.next_revision()?;
        let patch = apply(&mut self.project, command)?;
        if patch.is_empty() {
            return Ok(Outcome {
                state: self.state(),
                journal: None,
            });
        }
        self.revision = next;
        self.redo.clear();
        let after = self.new_id();
        self.undo.push_back(Step {
            created: after,
            before: self.current,
            after,
            label: command.label().to_owned(),
            patch,
        });
        self.current = after;
        self.trim_history();
        Ok(Outcome {
            state: self.state(),
            journal: Some(Entry::Apply {
                revision: next,
                command: Box::new(command.clone()),
            }),
        })
    }

    /// Reverts the most recent step.
    ///
    /// # Errors
    /// [`HistoryError::RevisionConflict`], [`HistoryError::NothingToUndo`].
    pub fn undo(&mut self, base: Revision) -> Result<Outcome, HistoryError> {
        self.require_revision(base)?;
        let next = self.next_revision()?;
        let step = self.undo.pop_back().ok_or(HistoryError::NothingToUndo)?;
        let recorded = self.recorded(&step);
        match step.patch.clone().apply(&mut self.project) {
            Ok(redo) => {
                self.current = step.before;
                self.redo.push(Step {
                    patch: redo,
                    ..step
                });
                self.revision = next;
                Ok(Outcome {
                    state: self.state(),
                    journal: Some(Entry::Undo {
                        revision: next,
                        step: recorded,
                    }),
                })
            }
            Err(error) => {
                self.undo.push_back(step);
                Err(error.into())
            }
        }
    }

    /// Re-applies the most recently undone step.
    ///
    /// # Errors
    /// [`HistoryError::RevisionConflict`], [`HistoryError::NothingToRedo`].
    pub fn redo(&mut self, base: Revision) -> Result<Outcome, HistoryError> {
        self.require_revision(base)?;
        let next = self.next_revision()?;
        let step = self.redo.pop().ok_or(HistoryError::NothingToRedo)?;
        let recorded = self.recorded(&step);
        match step.patch.clone().apply(&mut self.project) {
            Ok(undo) => {
                self.current = step.after;
                self.undo.push_back(Step {
                    patch: undo,
                    ..step
                });
                self.revision = next;
                self.trim_history();
                Ok(Outcome {
                    state: self.state(),
                    journal: Some(Entry::Redo {
                        revision: next,
                        step: recorded,
                    }),
                })
            }
            Err(error) => {
                self.redo.push(step);
                Err(error.into())
            }
        }
    }

    fn require_revision(&self, base: Revision) -> Result<(), HistoryError> {
        if base == self.revision {
            Ok(())
        } else {
            Err(HistoryError::RevisionConflict {
                expected: base,
                current: self.revision,
            })
        }
    }

    /// Forgets the oldest steps beyond the limit. A saved state that is no
    /// longer reachable simply never matches the current one again.
    fn trim_history(&mut self) {
        let excess = self.undo.len().saturating_sub(self.history_limit);
        self.undo.drain(..excess);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{TimelineId, TrackId};
    use crate::model::{ProjectSettings, Track, TrackKind};

    fn track(name: &str) -> Track {
        Track {
            id: TrackId::generate(),
            kind: TrackKind::Video,
            name: name.into(),
            muted: false,
            hidden: false,
            locked: false,
            clips: Vec::new(),
            transitions: Vec::new(),
        }
    }

    fn add_track(timeline: TimelineId, name: &str) -> EditCommand {
        EditCommand::AddTrack {
            timeline,
            index: 0,
            track: track(name),
        }
    }

    fn editor() -> (Editor, TimelineId) {
        let project = Project::new("Test", ProjectSettings::default());
        let timeline = project.timelines[0].id;
        (Editor::new(project).expect("valid"), timeline)
    }

    #[test]
    fn undo_and_redo_restore_documents_and_revisions() {
        let (mut editor, timeline) = editor();
        let original = editor.project().clone();
        let applied = editor
            .apply(Revision(0), &add_track(timeline, "V1"))
            .expect("applies");
        assert_eq!(applied.state.revision, Revision(1));
        assert!(applied.state.dirty);
        assert_eq!(applied.state.undo_label.as_deref(), Some("addTrack"));
        let after = editor.project().clone();

        let undone = editor.undo(Revision(1)).expect("undoes");
        assert_eq!(editor.project(), &original);
        assert_eq!(undone.state.revision, Revision(2));
        assert!(!undone.state.dirty, "back at the opened state");
        assert_eq!(undone.state.redo_label.as_deref(), Some("addTrack"));
        assert_eq!(
            undone.journal,
            Some(Entry::Undo {
                revision: Revision(2),
                step: None
            })
        );

        editor.redo(Revision(2)).expect("redoes");
        assert_eq!(editor.project(), &after);
        assert_eq!(
            editor.undo(Revision(2)),
            Err(HistoryError::RevisionConflict {
                expected: Revision(2),
                current: Revision(3),
            })
        );
    }

    #[test]
    fn empty_histories_and_stale_revisions_are_refused() {
        let (mut editor, timeline) = editor();
        assert_eq!(editor.undo(Revision(0)), Err(HistoryError::NothingToUndo));
        assert_eq!(editor.redo(Revision(0)), Err(HistoryError::NothingToRedo));
        editor
            .apply(Revision(0), &add_track(timeline, "V1"))
            .expect("applies");
        // A double click sends the same request twice.
        assert!(matches!(
            editor.apply(Revision(0), &add_track(timeline, "V2")),
            Err(HistoryError::RevisionConflict { .. })
        ));
        assert_eq!(editor.revision(), Revision(1));
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let (mut editor, timeline) = editor();
        editor
            .apply(Revision(0), &add_track(timeline, "V1"))
            .expect("applies");
        editor.undo(Revision(1)).expect("undoes");
        editor
            .apply(Revision(2), &add_track(timeline, "V2"))
            .expect("applies");
        assert_eq!(editor.state().redo_label, None);
        assert_eq!(editor.redo(Revision(3)), Err(HistoryError::NothingToRedo));
    }

    #[test]
    fn no_op_commands_do_not_create_steps() {
        let (mut editor, _) = editor();
        let empty = EditCommand::Batch {
            label: "Nothing".into(),
            commands: Vec::new(),
        };
        let outcome = editor.apply(Revision(0), &empty).expect("applies");
        assert_eq!(outcome.journal, None);
        assert_eq!(outcome.state.revision, Revision(0));
        assert!(!outcome.state.dirty);
    }

    #[test]
    fn refused_commands_leave_everything_unchanged() {
        let (mut editor, _) = editor();
        let before = editor.project().clone();
        let error = editor
            .apply(Revision(0), &add_track(TimelineId::generate(), "V1"))
            .expect_err("unknown timeline");
        assert!(matches!(
            error,
            HistoryError::Edit(EditError::NotFound { .. })
        ));
        assert_eq!(editor.project(), &before);
        assert_eq!(editor.state().revision, Revision(0));
    }

    #[test]
    fn dirty_tracks_the_saved_state_through_undo_redo_and_trimming() {
        let (editor, timeline) = editor();
        let mut editor = editor.with_history_limit(2);
        let push = |editor: &mut Editor, name: &str| {
            editor
                .apply(editor.revision(), &add_track(timeline, name))
                .expect("applies")
                .state
        };
        assert!(push(&mut editor, "A").dirty);
        editor.mark_saved();
        assert!(!editor.state().dirty);
        assert!(push(&mut editor, "B").dirty);
        let undone = editor.undo(editor.revision()).expect("undoes B");
        assert!(!undone.state.dirty, "back at the save");
        editor.redo(editor.revision()).expect("redoes B");
        // A third step forgets "A"; the saved state (after A) is now the
        // bottom of the stack.
        push(&mut editor, "C");
        editor.undo(editor.revision()).expect("undoes C");
        let bottom = editor.undo(editor.revision()).expect("undoes B");
        assert!(!bottom.state.dirty, "the bottom is the saved state");
        assert_eq!(
            editor.undo(editor.revision()),
            Err(HistoryError::NothingToUndo)
        );
    }

    /// Regression: after "edit, save, undo" and a crash, the recovered
    /// session reported no unsaved changes although the document differs
    /// from the file, so closing it would not have prompted to save.
    #[test]
    fn recovery_after_an_undo_across_a_save_is_dirty() {
        let (mut editor, timeline) = editor();
        editor
            .apply(Revision(0), &add_track(timeline, "A"))
            .expect("applies");
        editor.mark_saved();
        let saved = editor.project().clone();
        let header = editor.begin_journal(journal::fingerprint(b"saved"));
        let undone = editor.undo(Revision(1)).expect("undoes A");
        assert!(undone.state.dirty);
        let entry = undone.journal.expect("journalled");
        assert!(
            matches!(&entry, Entry::Undo { step: Some(_), .. }),
            "a step older than the journal is recorded in it"
        );

        let journal = Recovered {
            header,
            entries: vec![entry],
            stop: None,
        };
        let (recovered, replay) = Editor::recover(saved, journal).expect("valid");
        assert_eq!(replay.failure, None);
        assert_eq!(recovered.project(), editor.project());
        assert!(recovered.state().dirty);
        assert_eq!(recovered.state().redo_label.as_deref(), Some("addTrack"));
    }

    #[test]
    fn forgetting_the_opened_state_makes_it_unreachable() {
        let (editor, timeline) = editor();
        let mut editor = editor.with_history_limit(1);
        editor
            .apply(Revision(0), &add_track(timeline, "A"))
            .expect("applies");
        editor
            .apply(Revision(1), &add_track(timeline, "B"))
            .expect("applies");
        let undone = editor.undo(Revision(2)).expect("undoes B");
        assert!(
            undone.state.dirty,
            "A is still applied and cannot be undone"
        );
    }
}
