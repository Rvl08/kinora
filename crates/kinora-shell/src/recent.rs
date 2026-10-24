//! The recent-projects list, kept in the app's data folder.
//!
//! The file is small, versioned JSON, replaced atomically on every change:
//!
//! ```json
//! { "version": 1, "projects": [{ "path": "D:\\Video\\Holiday.kinora", "name": "Holiday", "openedAt": 1792800000000, "format": { … }, "duration": { … } }] }
//! ```
//!
//! It is a convenience, never the only copy of anything: a file that cannot
//! be read as this format is moved aside to [`CORRUPT_RECENT_FILE`] and the
//! list starts empty. The name, format and duration of each project are
//! recorded when it is opened or saved, so listing never parses the projects
//! themselves; whether each folder still holds a `project.json` is checked
//! every time the list is read.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kinora_ipc::command::RecentProject;
use kinora_project::{Time, VideoFormat};
use serde::{Deserialize, Serialize};

use crate::fsx;
use crate::layout::PROJECT_FILE;

/// The list's file name in the app's data folder.
pub const RECENT_FILE: &str = "recent-projects.json";

/// Where an unreadable list is moved, replacing an older one.
pub const CORRUPT_RECENT_FILE: &str = "recent-projects.corrupt.json";

/// Most projects kept on the list.
pub const MAX_RECENT_PROJECTS: usize = 20;

/// Version of the file format.
const FORMAT: u32 = 1;

/// Largest list file read; twenty entries take a few kilobytes.
const MAX_FILE_BYTES: usize = 1024 * 1024;

/// Largest timestamp the UI can hold exactly (2^53 − 1).
pub(crate) const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// The file as stored.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListFile {
    version: u32,
    projects: Vec<Entry>,
}

/// Only the version, read first so a newer format is recognised as such.
#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

/// One project on the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Entry {
    /// The project folder, as `RecentProject::path`.
    pub(crate) path: String,
    /// The project's display name.
    pub(crate) name: String,
    /// When it was last opened, in milliseconds since the Unix epoch.
    pub(crate) opened_at: u64,
    /// Canvas and frame rate of its first timeline.
    pub(crate) format: Option<VideoFormat>,
    /// Length of its first timeline.
    pub(crate) duration: Option<Time>,
}

/// The list, loaded on first use.
#[derive(Debug)]
pub(crate) struct RecentProjects {
    file: PathBuf,
    /// `None` until the file has been read.
    entries: Option<Vec<Entry>>,
}

impl RecentProjects {
    /// The list stored in `data_dir`, which is created when the list is
    /// first written.
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            file: data_dir.join(RECENT_FILE),
            entries: None,
        }
    }

    /// The projects, most recent first, each checked for availability. A
    /// list that cannot be read is reported as empty.
    pub(crate) fn list(&mut self) -> Vec<RecentProject> {
        let entries = self.entries().cloned();
        let entries = entries.unwrap_or_else(|error| {
            log::warn!("could not read {}: {error}", self.file.display());
            Vec::new()
        });
        entries
            .into_iter()
            .map(|entry| RecentProject {
                available: Path::new(&entry.path).join(PROJECT_FILE).is_file(),
                path: entry.path,
                name: entry.name,
                opened_at: entry.opened_at,
                format: entry.format,
                duration: entry.duration,
            })
            .collect()
    }

    /// Puts `entry` first, replacing the entry with the same path.
    pub(crate) fn record(&mut self, entry: Entry) -> io::Result<()> {
        let entries = self.entries()?;
        entries.retain(|e| e.path != entry.path);
        entries.insert(0, entry);
        entries.truncate(MAX_RECENT_PROJECTS);
        self.save()
    }

    /// Updates the name, format and duration of the entry with `entry`'s
    /// path, keeping its place and time; does nothing when it is not listed.
    pub(crate) fn refresh(&mut self, entry: Entry) -> io::Result<()> {
        let entries = self.entries()?;
        let Some(existing) = entries.iter_mut().find(|e| e.path == entry.path) else {
            return Ok(());
        };
        let updated = Entry {
            opened_at: existing.opened_at,
            ..entry
        };
        if *existing == updated {
            return Ok(());
        }
        *existing = updated;
        self.save()
    }

    /// Removes the entries whose path is one of `paths`; does nothing when
    /// none is listed.
    pub(crate) fn forget(&mut self, paths: &[&str]) -> io::Result<()> {
        let entries = self.entries()?;
        let before = entries.len();
        entries.retain(|e| !paths.contains(&e.path.as_str()));
        if entries.len() == before {
            return Ok(());
        }
        self.save()
    }

    /// The list's file.
    pub(crate) fn file(&self) -> &Path {
        &self.file
    }

    fn entries(&mut self) -> io::Result<&mut Vec<Entry>> {
        if self.entries.is_none() {
            self.entries = Some(load(&self.file)?);
        }
        Ok(self.entries.get_or_insert_default())
    }

    fn save(&self) -> io::Result<()> {
        let file = ListFile {
            version: FORMAT,
            projects: self.entries.clone().unwrap_or_default(),
        };
        let mut json = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
        json.push('\n');
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        fsx::write_atomic(&self.file, json.as_bytes())
    }
}

/// Reads the list. A missing file is an empty list; a file that is not a
/// usable list is moved aside and also yields an empty list. Only a file
/// that cannot be read at all (locked, access denied) is an error, so a
/// passing problem never erases the list.
fn load(file: &Path) -> io::Result<Vec<Entry>> {
    let bytes = match fsx::read_prefix(file, MAX_FILE_BYTES + 1) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    match parse(&bytes) {
        Ok(entries) => Ok(entries),
        Err(reason) => {
            log::warn!("the recent-projects list is unusable ({reason}); moving it aside");
            if let Err(error) = fs::rename(file, file.with_file_name(CORRUPT_RECENT_FILE)) {
                log::warn!("could not move {} aside: {error}", file.display());
            }
            Ok(Vec::new())
        }
    }
}

/// Parses and cleans a list file: entries with an empty path or a repeated
/// path are dropped, timestamps are clamped, and at most
/// [`MAX_RECENT_PROJECTS`] are kept.
fn parse(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("the file is too large".into());
    }
    let probe: VersionProbe = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if probe.version != FORMAT {
        return Err(format!("unknown version {}", probe.version));
    }
    let file: ListFile = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut entries: Vec<Entry> = Vec::new();
    for mut entry in file.projects {
        if entries.len() == MAX_RECENT_PROJECTS {
            break;
        }
        if entry.path.is_empty() || entries.iter().any(|e| e.path == entry.path) {
            continue;
        }
        entry.opened_at = entry.opened_at.min(MAX_SAFE_INTEGER);
        entries.push(entry);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use kinora_project::ProjectSettings;

    use super::*;

    fn entry(path: &str, opened_at: u64) -> Entry {
        Entry {
            path: path.to_owned(),
            name: format!("Project {opened_at}"),
            opened_at,
            format: Some(ProjectSettings::default().default_format),
            duration: Some(Time::ZERO),
        }
    }

    #[test]
    fn most_recent_first_without_duplicates_and_bounded() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut list = RecentProjects::new(dir.path());
        for i in 0..25 {
            list.record(entry(&format!("/p/{i}"), i)).expect("records");
        }
        list.record(entry("/p/10", 99)).expect("records");
        let paths: Vec<String> = list.list().into_iter().map(|p| p.path).collect();
        assert_eq!(paths.len(), MAX_RECENT_PROJECTS);
        assert_eq!(paths[0], "/p/10");
        assert_eq!(paths[1], "/p/24");
        assert_eq!(paths.iter().filter(|p| *p == "/p/10").count(), 1);

        // A second instance reads the same list back.
        let mut reread = RecentProjects::new(dir.path());
        assert_eq!(reread.list(), list.list());
    }

    #[test]
    fn missing_folders_are_unavailable() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("Here.kinora");
        fs::create_dir(&project).expect("folder");
        fs::write(project.join(PROJECT_FILE), "{}").expect("file");
        let here = project.to_string_lossy().into_owned();
        let gone = dir.path().join("Gone.kinora").to_string_lossy().into_owned();

        let mut list = RecentProjects::new(dir.path());
        list.record(entry(&gone, 1)).expect("records");
        list.record(entry(&here, 2)).expect("records");
        let listed = list.list();
        assert_eq!(listed.len(), 2);
        assert!(listed[0].available);
        assert!(!listed[1].available);
        assert_eq!(listed[1].format, entry(&gone, 1).format, "kept for the UI");
    }

    #[test]
    fn forget_and_refresh() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut list = RecentProjects::new(dir.path());
        list.record(entry("/a", 1)).expect("records");
        list.record(entry("/b", 2)).expect("records");
        list.forget(&["/a"]).expect("forgets");
        list.forget(&["/never-listed"]).expect("absent is fine");

        let mut renamed = entry("/b", 50);
        renamed.name = "Renamed".into();
        list.refresh(renamed).expect("refreshes");
        list.refresh(entry("/c", 3)).expect("absent is ignored");
        let listed = RecentProjects::new(dir.path()).list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Renamed");
        assert_eq!(listed[0].opened_at, 2, "refresh keeps the time");
    }

    #[test]
    fn a_corrupt_file_is_moved_aside() {
        for garbage in [
            "not json".to_owned(),
            r#"{"version":1,"projects":[{"path":"/x"}]}"#.to_owned(),
            r#"{"version":2,"projects":[]}"#.to_owned(),
            "[]".to_owned(),
            " ".repeat(MAX_FILE_BYTES + 1),
        ] {
            let dir = tempfile::tempdir().expect("temp dir");
            fs::write(dir.path().join(RECENT_FILE), garbage).expect("writes");
            let mut list = RecentProjects::new(dir.path());
            assert!(list.list().is_empty());
            assert!(dir.path().join(CORRUPT_RECENT_FILE).is_file());
            assert!(!dir.path().join(RECENT_FILE).exists());

            list.record(entry("/fresh", 1)).expect("starts over");
            assert_eq!(RecentProjects::new(dir.path()).list().len(), 1);
        }
    }

    #[test]
    fn duplicates_and_huge_timestamps_are_cleaned() {
        let json = format!(
            r#"{{"version":1,"projects":[
                {{"path":"/a","name":"A","openedAt":{},"format":null,"duration":null}},
                {{"path":"/a","name":"A again","openedAt":1,"format":null,"duration":null}},
                {{"path":"","name":"Empty","openedAt":1,"format":null,"duration":null}}
            ]}}"#,
            u64::MAX
        );
        let entries = parse(json.as_bytes()).expect("parses");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "A");
        assert_eq!(entries[0].opened_at, MAX_SAFE_INTEGER);
    }
}
