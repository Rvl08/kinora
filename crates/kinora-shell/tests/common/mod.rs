//! Helpers shared by the shell's integration tests.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kinora_ipc::command::{
    ApplyCommandArgs, CreateProjectArgs, OpenProjectArgs, ProjectOpened, ProjectState,
};
use kinora_ipc::event::{ChangeReason, JobProgress, ProjectChanged};
use kinora_ipc::{IpcError, Revision};
use kinora_project::id::{TimelineId, TrackId};
use kinora_project::{EditCommand, Project, ProjectSettings, Track, TrackKind};
use kinora_shell::{EventSink, Shell};

/// Records every `project:changed` event.
#[derive(Clone, Default)]
pub struct Events(Arc<Mutex<Vec<ProjectChanged>>>);

impl Events {
    pub fn reasons(&self) -> Vec<ChangeReason> {
        let events = self.0.lock().expect("lock");
        events.iter().map(|event| event.reason).collect()
    }

    pub fn last(&self) -> Option<ProjectChanged> {
        self.0.lock().expect("lock").last().cloned()
    }
}

impl EventSink for Events {
    fn project_changed(&self, event: &ProjectChanged) {
        self.0.lock().expect("lock").push(event.clone());
    }

    fn job_progress(&self, _event: &JobProgress) {}
}

/// A temporary folder with `projects/` for project folders and `data/` for
/// the app's data.
pub struct Env {
    pub dir: tempfile::TempDir,
    pub events: Events,
}

impl Env {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir(dir.path().join("projects")).expect("projects folder");
        Self {
            dir,
            events: Events::default(),
        }
    }

    pub fn projects(&self) -> PathBuf {
        self.dir.path().join("projects")
    }

    pub fn data(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    /// A new shell, as after an app start.
    pub fn shell(&self) -> Shell {
        Shell::new(&self.data(), self.events.clone()).with_clock(|| 1_000)
    }

    /// The folder of the project `name`.
    pub fn folder(&self, name: &str) -> PathBuf {
        self.projects().join(format!("{name}.kinora"))
    }
}

pub fn text(path: &Path) -> String {
    path.to_str().expect("UTF-8 path").to_owned()
}

pub fn create(shell: &mut Shell, env: &Env, name: &str) -> ProjectOpened {
    shell
        .create_project(CreateProjectArgs {
            parent_dir: text(&env.projects()),
            name: name.to_owned(),
            settings: ProjectSettings::default(),
        })
        .expect("creates")
}

pub fn open(shell: &mut Shell, folder: &Path) -> Result<ProjectOpened, IpcError> {
    shell.open_project(OpenProjectArgs { path: text(folder) })
}

pub fn track(name: &str) -> Track {
    Track {
        id: TrackId::generate(),
        kind: TrackKind::Video,
        name: name.to_owned(),
        muted: false,
        hidden: false,
        locked: false,
        clips: Vec::new(),
        transitions: Vec::new(),
    }
}

pub fn add_track_command(timeline: TimelineId, name: &str) -> EditCommand {
    EditCommand::AddTrack {
        timeline,
        index: 0,
        track: track(name),
    }
}

/// Adds a track to the open project's first timeline.
pub fn add_track(shell: &mut Shell, name: &str) -> ProjectState {
    let snapshot = shell.get_project().expect("open");
    let command = add_track_command(snapshot.document.timelines[0].id, name);
    shell
        .apply_command(ApplyCommandArgs {
            base_revision: snapshot.state.revision,
            command,
        })
        .expect("applies")
}

pub fn document(shell: &Shell) -> Project {
    shell.get_project().expect("open").document
}

pub fn revision(shell: &Shell) -> Revision {
    shell.state().expect("open").revision
}

pub fn track_names(project: &Project) -> Vec<String> {
    let tracks = &project.timelines[0].tracks;
    tracks.iter().map(|track| track.name.clone()).collect()
}
