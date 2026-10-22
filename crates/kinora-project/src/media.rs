//! The media pool: files the project references.
//!
//! Media is referenced, not embedded (ADR-0004 §2): each asset records where
//! the file was (absolute path, plus a path relative to the project folder for
//! moved or shared projects) and a fast content hash, so a missing file can be
//! found again (relink) and a replaced file is detected. Derived data
//! (proxies, thumbnails, waveforms, transcripts) lives in `cache/` and is not
//! part of this document.

use kinora_core::{FrameRate, Time};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::MediaId;
use crate::model::ColorSpace;

/// One file in the media pool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct MediaAsset {
    /// Stable identifier; clips refer to the asset by it.
    pub id: MediaId,
    /// Display name, initially the file name.
    pub name: String,
    /// What the file contains.
    pub kind: MediaKind,
    /// Where the file is.
    pub location: MediaLocation,
    /// Fingerprint of the file's content, for relinking and change detection.
    pub hash: ContentHash,
    /// Stream properties found when the file was imported.
    pub info: MediaInfo,
    /// Imported by the user or produced by Kinora.
    pub origin: MediaOrigin,
}

/// The kind of a media file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum MediaKind {
    /// Moving pictures, possibly with sound.
    Video,
    /// Sound only.
    Audio,
    /// A still picture.
    Image,
}

/// Where a media file is on disk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct MediaLocation {
    /// Absolute path when last seen, e.g. `D:\Footage\beach.mp4`.
    pub absolute_path: String,
    /// Path relative to the `.kinora` project folder with `/` separators,
    /// e.g. `../Footage/beach.mp4` or `media/beach.mp4` after "Collect media".
    /// Tried first, so a project folder moved together with its media still
    /// resolves. `null` when the file is on another drive.
    pub relative_path: Option<String>,
}

/// A fast fingerprint of a file's content.
///
/// Not a cryptographic hash: it detects that a file was replaced or matches a
/// candidate during relink, quickly, even for files of many gigabytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ContentHash {
    /// How the digest was computed.
    pub algorithm: HashAlgorithm,
    /// The digest as lowercase hexadecimal (32 characters for 128 bits).
    pub digest: String,
    /// File size in bytes, part of the fingerprint. At most 2^53 − 1.
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    #[schemars(range(max = 9_007_199_254_740_991_u64))]
    pub file_size: u64,
}

/// Fingerprint algorithms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum HashAlgorithm {
    /// XXH3-128 over the file size and three fixed-size samples (start,
    /// middle, end) of the file (ADR-0004 §2). `kinora-media` defines the
    /// exact sampling; changing it requires a new variant.
    Xxh3Sampled,
}

/// Stream properties of a media file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct MediaInfo {
    /// Length of the file; `null` for still images, which last as long as
    /// their clip.
    pub duration: Option<Time>,
    /// The primary video stream, if any.
    pub video: Option<VideoStreamInfo>,
    /// The primary audio stream, if any.
    pub audio: Option<AudioStreamInfo>,
}

/// Properties of a video (or image) stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct VideoStreamInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Nominal frame rate; `null` for variable frame rate or still images.
    pub frame_rate: Option<FrameRate>,
    /// Colour space as signalled by the file; `null` when unspecified.
    pub color_space: Option<ColorSpace>,
    /// `true` when the stream carries an alpha channel.
    pub has_alpha: bool,
}

/// Properties of an audio stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct AudioStreamInfo {
    /// Samples per second.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u16,
}

/// Where an asset came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum MediaOrigin {
    /// Imported by the user.
    Imported,
    /// Produced by Kinora: text-to-speech, a background cutout, an upscale, a
    /// voiceover recording. Can be regenerated from its source.
    Generated {
        /// Generator identifier with version, e.g. `"kinora.ai.cutout@1"`.
        generator: String,
        /// The asset it was derived from, if any.
        source: Option<MediaId>,
    },
}
