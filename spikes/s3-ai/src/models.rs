//! Verified model downloads (the prototype of Kinora's Model Manager).
//!
//! Rules: a model is fetched only from the URL pinned in models/manifest.toml;
//! partial downloads resume with an HTTP Range request; the finished file must
//! match the pinned size and SHA-256 (or, for small non-LFS files that have no
//! SHA-256 pin yet, the host's git blob SHA-1, after which the SHA-256 is
//! printed for pinning); only then is `<file>.part` renamed into place.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Deserialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};

// family and license are not read by the spike, but parsing them makes every
// manifest entry state both.
#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct Model {
    pub id: String,
    pub family: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub git_blob_sha1: Option<String>,
    pub license: String,
    pub upstream: String,
}

#[derive(Deserialize)]
struct Manifest {
    model: Vec<Model>,
}

pub struct Store {
    pub dir: PathBuf,
    pub models: Vec<Model>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReport {
    pub id: String,
    pub bytes: u64,
    pub action: String,
    pub seconds: f64,
    pub mb_per_s: f64,
    pub resumed_from: u64,
    pub sha256: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// Git's object id for a blob: SHA-1 of "blob <len>\0" followed by the content.
fn git_blob_sha1(path: &Path) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let mut h = Sha1::new();
    h.update(format!("blob {}\0", data.len()).as_bytes());
    h.update(&data);
    Ok(hex(&h.finalize()))
}

impl Store {
    pub fn open(manifest: &Path, dir: &Path) -> Result<Self, String> {
        let text =
            fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let m: Manifest = toml::from_str(&text).map_err(|e| format!("manifest: {e}"))?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        Ok(Self {
            dir: dir.to_owned(),
            models: m.model,
        })
    }

    pub fn get(&self, id: &str) -> Result<&Model, String> {
        self.models
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| format!("model {id} not in manifest"))
    }

    pub fn path(&self, id: &str) -> Result<PathBuf, String> {
        Ok(self.dir.join(&self.get(id)?.file))
    }

    /// Ensures `id` is present and verified; downloads (resuming) if needed.
    /// `stop_after` cuts a download short (resume test only).
    pub fn fetch(&self, id: &str, stop_after: Option<u64>) -> Result<FetchReport, String> {
        let m = self.get(id)?.clone();
        let final_path = self.dir.join(&m.file);
        let part = self.dir.join(format!("{}.part", m.file));
        let t0 = Instant::now();
        if final_path.exists() && stop_after.is_none() {
            let sha = sha256_file(&final_path).map_err(|e| e.to_string())?;
            if self.verify(&m, &final_path, &sha)? {
                return Ok(FetchReport {
                    id: m.id,
                    bytes: m.bytes,
                    action: "cached".into(),
                    seconds: t0.elapsed().as_secs_f64(),
                    mb_per_s: 0.0,
                    resumed_from: 0,
                    sha256: sha,
                });
            }
            fs::remove_file(&final_path).map_err(|e| e.to_string())?;
        }
        if m.sha256.is_empty() && m.git_blob_sha1.is_none() {
            return Err(format!("{}: no pinned digest; refusing to download", m.id));
        }
        let have = fs::metadata(&part).map(|x| x.len()).unwrap_or(0);
        let have = if have > m.bytes { 0 } else { have };
        let mut req = ureq::get(&m.upstream);
        if have > 0 {
            req = req.header("Range", &format!("bytes={have}-"));
        }
        let resp = req.call().map_err(|e| format!("{}: {e}", m.id))?;
        let resumed = resp.status() == 206;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(!resumed)
            .open(&part)
            .map_err(|e| e.to_string())?;
        let start = if resumed { have } else { 0 };
        file.seek(SeekFrom::Start(start))
            .map_err(|e| e.to_string())?;
        let mut body = resp.into_body();
        let mut reader = body.as_reader();
        let mut buf = vec![0u8; 1 << 20];
        let mut written = start;
        let mut fetched = 0u64;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("{}: read: {e}", m.id))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            written += n as u64;
            fetched += n as u64;
            if stop_after.is_some_and(|s| written >= s) {
                file.flush().ok();
                return Ok(FetchReport {
                    id: m.id,
                    bytes: written,
                    action: "interrupted (test)".into(),
                    seconds: t0.elapsed().as_secs_f64(),
                    mb_per_s: 0.0,
                    resumed_from: start,
                    sha256: String::new(),
                });
            }
        }
        file.flush().map_err(|e| e.to_string())?;
        drop(file);
        if written != m.bytes {
            return Err(format!("{}: size {written} != pinned {}", m.id, m.bytes));
        }
        let sha = sha256_file(&part).map_err(|e| e.to_string())?;
        if !self.verify(&m, &part, &sha)? {
            fs::remove_file(&part).ok();
            return Err(format!("{}: digest mismatch, file discarded", m.id));
        }
        fs::rename(&part, &final_path).map_err(|e| e.to_string())?;
        let secs = t0.elapsed().as_secs_f64();
        Ok(FetchReport {
            id: m.id,
            bytes: m.bytes,
            action: if resumed {
                "resumed".into()
            } else {
                "downloaded".into()
            },
            seconds: secs,
            mb_per_s: fetched as f64 / 1_048_576.0 / secs,
            resumed_from: start,
            sha256: sha,
        })
    }

    fn verify(&self, m: &Model, path: &Path, sha256: &str) -> Result<bool, String> {
        if fs::metadata(path).map(|x| x.len()).unwrap_or(0) != m.bytes {
            return Ok(false);
        }
        if !m.sha256.is_empty() {
            return Ok(m.sha256 == sha256);
        }
        // No SHA-256 pin yet: the host's git blob id is the authority.
        let want = m.git_blob_sha1.as_deref().unwrap_or_default();
        let got = git_blob_sha1(path).map_err(|e| e.to_string())?;
        if got == want {
            eprintln!("PIN {}: git blob verified; sha256 = \"{sha256}\"", m.id);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
