//! A transaction over a document: applies ops as they are planned and can
//! roll all of them back.

use uuid::Uuid;

use crate::model::{Clip, Project, Track};

use super::patch::{Op, Patch, TrackAt, revert};
use super::{EditError, EditRule, EntityKind};

/// Applies ops to `project` immediately, so each step of a command (and
/// each command of a batch) is planned against the state the previous ones
/// produced, and remembers their inverses.
pub(crate) struct Tx<'a> {
    pub project: &'a mut Project,
    /// Inverses in the order their ops were applied.
    undo: Vec<Op>,
}

impl<'a> Tx<'a> {
    pub(crate) fn new(project: &'a mut Project) -> Self {
        Self {
            project,
            undo: Vec::new(),
        }
    }

    /// Applies one op.
    pub(crate) fn run(&mut self, op: Op) -> Result<(), EditError> {
        let inverse = op.apply(self.project)?;
        self.undo.push(inverse);
        Ok(())
    }

    /// Restores the document as it was when the transaction began.
    pub(crate) fn rollback(self) {
        revert(self.project, self.undo);
    }

    /// Keeps the changes and returns the patch that undoes them.
    pub(crate) fn commit(self) -> Patch {
        let mut ops = self.undo;
        ops.reverse();
        Patch::new(ops)
    }
}

/// Lookups by identifier. Linear scans: a document holds thousands of
/// entities at most, and every edit runs the validator over all of them
/// anyway.
impl Project {
    pub(crate) fn media_index(&self, id: Uuid) -> Option<usize> {
        self.media.iter().position(|m| m.id.as_uuid() == id)
    }

    pub(crate) fn timeline_index(&self, id: Uuid) -> Option<usize> {
        self.timelines.iter().position(|t| t.id.as_uuid() == id)
    }

    pub(crate) fn track_at(&self, id: Uuid) -> Option<TrackAt> {
        self.timelines.iter().enumerate().find_map(|(timeline, t)| {
            let track = t.tracks.iter().position(|k| k.id.as_uuid() == id)?;
            Some(TrackAt { timeline, track })
        })
    }

    pub(crate) fn track(&self, at: TrackAt) -> Result<&Track, EditError> {
        self.timelines
            .get(at.timeline)
            .and_then(|t| t.tracks.get(at.track))
            .ok_or(EditError::Internal("a track position does not exist"))
    }

    /// The track and index of a clip.
    pub(crate) fn clip_at(&self, id: Uuid) -> Option<(TrackAt, usize)> {
        self.tracks_with_position().find_map(|(at, track)| {
            let index = track.clips.iter().position(|c| c.id.as_uuid() == id)?;
            Some((at, index))
        })
    }

    pub(crate) fn transition_at(&self, id: Uuid) -> Option<(TrackAt, usize)> {
        self.tracks_with_position().find_map(|(at, track)| {
            let index = track
                .transitions
                .iter()
                .position(|t| t.id.as_uuid() == id)?;
            Some((at, index))
        })
    }

    /// The timeline index and marker index of a marker.
    pub(crate) fn marker_at(&self, id: Uuid) -> Option<(usize, usize)> {
        self.timelines.iter().enumerate().find_map(|(timeline, t)| {
            let index = t.markers.iter().position(|m| m.id.as_uuid() == id)?;
            Some((timeline, index))
        })
    }

    fn tracks_with_position(&self) -> impl Iterator<Item = (TrackAt, &Track)> {
        self.timelines.iter().enumerate().flat_map(|(timeline, t)| {
            t.tracks
                .iter()
                .enumerate()
                .map(move |(track, k)| (TrackAt { timeline, track }, k))
        })
    }

    /// `true` when any entity of the document already has this identifier.
    pub(crate) fn id_in_use(&self, id: Uuid) -> bool {
        let clip_has = |clip: &Clip| {
            clip.id.as_uuid() == id
                || clip
                    .visual
                    .iter()
                    .flat_map(|v| &v.effects)
                    .any(|e| e.id.as_uuid() == id)
        };
        self.id.as_uuid() == id
            || self.media.iter().any(|m| m.id.as_uuid() == id)
            || self.timelines.iter().any(|t| {
                t.id.as_uuid() == id
                    || t.markers.iter().any(|m| m.id.as_uuid() == id)
                    || t.tracks.iter().any(|k| {
                        k.id.as_uuid() == id
                            || k.transitions.iter().any(|x| x.id.as_uuid() == id)
                            || k.clips.iter().any(clip_has)
                    })
            })
    }

    /// Fails with [`EditRule::DuplicateId`] when `id` is taken.
    pub(crate) fn require_new_id(&self, id: Uuid, entity: EntityKind) -> Result<(), EditError> {
        if self.id_in_use(id) {
            Err(EditError::rejected(
                EditRule::DuplicateId,
                format!("the new {entity}'s identifier {id} is already used"),
            ))
        } else {
            Ok(())
        }
    }
}
