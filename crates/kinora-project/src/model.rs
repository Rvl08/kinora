//! The document tree: project, settings, timelines, tracks and clips.

use kinora_core::{FrameRate, Rational, Rounding, Time};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{
    ClipId, EffectInstanceId, MarkerId, MediaId, ProjectId, TimelineId, TrackId, TransitionId,
};
use crate::media::MediaAsset;
use crate::text::{CaptionContent, TextContent};
use crate::value::{Animated, Color, ParamValue, Params, Vec2};
use crate::version::SchemaVersion;

/// The whole `project.json` document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(title = "Kinora project")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Project {
    /// Document format version; see `kinora_project::version`.
    pub schema_version: SchemaVersion,
    /// Stable project identifier.
    pub id: ProjectId,
    /// Display name.
    pub name: String,
    /// Project-wide defaults.
    pub settings: ProjectSettings,
    /// The media pool.
    pub media: Vec<MediaAsset>,
    /// Timelines (sequences), e.g. a 16:9 cut and a 9:16 cut of the same
    /// material. At least one.
    pub timelines: Vec<Timeline>,
}

impl Project {
    /// An empty project with one empty timeline in the default format.
    pub fn new(name: impl Into<String>, settings: ProjectSettings) -> Self {
        let name = name.into();
        Self {
            schema_version: SchemaVersion::CURRENT,
            id: ProjectId::generate(),
            timelines: vec![Timeline::new(name.clone(), settings.default_format)],
            name,
            settings,
            media: Vec::new(),
        }
    }

    /// The media asset with this id.
    pub fn media(&self, id: MediaId) -> Option<&MediaAsset> {
        self.media.iter().find(|m| m.id == id)
    }
}

/// Project-wide settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ProjectSettings {
    /// Format given to new timelines.
    pub default_format: VideoFormat,
    /// Colour space the compositor works and exports in.
    pub color_space: ColorSpace,
    /// Mixer output format.
    pub audio: AudioFormat,
}

impl Default for ProjectSettings {
    /// 1080p30, Rec. 709, 48 kHz stereo.
    fn default() -> Self {
        Self {
            default_format: VideoFormat {
                width: 1920,
                height: 1080,
                frame_rate: FrameRate::FPS_30,
            },
            color_space: ColorSpace::Rec709,
            audio: AudioFormat {
                sample_rate: 48_000,
                channels: ChannelLayout::Stereo,
            },
        }
    }
}

/// Canvas size and frame rate of a timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct VideoFormat {
    /// Width in pixels; even, `2..=16384`.
    pub width: u32,
    /// Height in pixels; even, `2..=16384`.
    pub height: u32,
    /// Frames per second.
    pub frame_rate: FrameRate,
}

/// Colour spaces. Phase 1 renders Rec. 709 only; the HDR variants exist so
/// the format does not need a new version when HDR arrives (P5+).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum ColorSpace {
    /// ITU-R BT.709 primaries and transfer (SDR video, close to sRGB).
    Rec709,
    /// ITU-R BT.2100 with the PQ transfer (HDR10).
    Rec2100Pq,
    /// ITU-R BT.2100 with the HLG transfer.
    Rec2100Hlg,
}

/// Mixer output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct AudioFormat {
    /// Samples per second, e.g. 48000.
    pub sample_rate: u32,
    /// Channel layout.
    pub channels: ChannelLayout,
}

/// Audio channel layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum ChannelLayout {
    /// One channel.
    Mono,
    /// Two channels.
    Stereo,
}

/// A timeline (sequence): a canvas and a stack of tracks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Timeline {
    /// Stable identifier.
    pub id: TimelineId,
    /// Display name.
    pub name: String,
    /// Canvas size and frame rate.
    pub format: VideoFormat,
    /// Tracks in compositing order: index 0 is drawn first (the bottom
    /// layer). Audio tracks are mixed regardless of order.
    pub tracks: Vec<Track>,
    /// Markers and chapters, ordered by `start`.
    pub markers: Vec<Marker>,
}

impl Timeline {
    /// An empty timeline.
    pub fn new(name: impl Into<String>, format: VideoFormat) -> Self {
        Self {
            id: TimelineId::generate(),
            name: name.into(),
            format,
            tracks: Vec::new(),
            markers: Vec::new(),
        }
    }
}

/// A row of clips of one kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Track {
    /// Stable identifier.
    pub id: TrackId,
    /// What the track holds.
    pub kind: TrackKind,
    /// Display name.
    pub name: String,
    /// Silences the track's audio.
    pub muted: bool,
    /// Hides the track's pictures.
    pub hidden: bool,
    /// Protects the track from edits.
    pub locked: bool,
    /// Clips ordered by `start`, not overlapping.
    pub clips: Vec<Clip>,
    /// Transitions between adjacent clips of this track.
    pub transitions: Vec<Transition>,
}

/// Track kinds; each accepts certain clip contents (see [`ClipContent`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum TrackKind {
    /// Video and image media, solids and adjustment layers.
    Video,
    /// Audio, from audio files or the sound of video files.
    Audio,
    /// Titles and other text.
    Text,
    /// Captions (subtitles).
    Caption,
}

/// An item on a track.
///
/// Timing: the clip occupies `[start, start + duration)` on the timeline. For
/// media, the source range is derived, never stored twice:
/// `sourceIn .. sourceIn + duration × speed` (rounded to the flick, see
/// [`Speed::source_duration`]). Every time here is on the edit grid
/// (ADR-0004 amendment 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Clip {
    /// Stable identifier.
    pub id: ClipId,
    /// Display name override; `null` shows the content's name.
    pub name: Option<String>,
    /// Position on the timeline, `>= 0`.
    pub start: Time,
    /// Length on the timeline, `> 0`.
    pub duration: Time,
    /// A disabled clip is kept but neither drawn nor heard.
    pub enabled: bool,
    /// What the clip shows or plays.
    pub content: ClipContent,
    /// Picture properties; required on video, text and caption tracks,
    /// `null` on audio tracks.
    pub visual: Option<Visual>,
    /// Sound properties; required on audio tracks, optional for video media
    /// with sound, `null` otherwise.
    pub audio: Option<AudioMix>,
}

impl Clip {
    /// The first instant after the clip: `start + duration`.
    pub fn end(&self) -> Option<Time> {
        self.start.checked_add(self.duration)
    }
}

/// What a clip shows or plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum ClipContent {
    /// A range of a media file (video and audio tracks).
    Media(MediaContent),
    /// A title (text tracks).
    Text(TextContent),
    /// A caption cue (caption tracks).
    Caption(CaptionContent),
    /// A solid colour (video tracks).
    Solid(SolidContent),
    /// Applies the clip's effects to everything below it (video tracks).
    Adjustment,
}

/// A range of a media file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct MediaContent {
    /// The asset in the media pool.
    pub media: MediaId,
    /// Where playback starts in the source, `>= 0`. With `reverse`, the
    /// clip plays the same source range backwards.
    pub source_in: Time,
    /// Playback speed.
    pub speed: Speed,
    /// Plays the source range backwards.
    pub reverse: bool,
    /// Keeps the pitch of sped-up or slowed-down sound.
    pub preserve_pitch: bool,
}

/// Playback speed of a media clip.
///
/// A tagged union so speed curves (P2) can be added as a new variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum Speed {
    /// A fixed multiplier, `> 0`: `2` is twice as fast, `1/2` is slow motion.
    /// Exact, so source and timeline durations convert without drift.
    Constant {
        /// Source seconds per timeline second.
        rate: Rational,
    },
}

impl Speed {
    /// Normal speed.
    pub const NORMAL: Self = Self::Constant {
        rate: Rational::ONE,
    };

    /// How many source seconds play during `duration` timeline seconds,
    /// rounded to the nearest flick so source times stay on the edit grid
    /// (at most half a flick, 0.7 ns, from the exact value). `None` when
    /// `duration` is off the grid or the result is out of range.
    ///
    /// This is the one mapping from timeline to source time; the validator,
    /// the editing core and the renderer all use it.
    pub fn source_duration(self, duration: Time) -> Option<Time> {
        match self {
            Self::Constant { rate } => duration.scale_to_grid(rate, Rounding::Nearest),
        }
    }

    /// The longest timeline duration whose source range
    /// ([`Self::source_duration`]) does not exceed `source`: the exact
    /// quotient rounded down to the flick. `None` when `source` is off the
    /// grid or the result is out of range.
    pub fn timeline_duration(self, source: Time) -> Option<Time> {
        match self {
            Self::Constant { rate } => source.div_to_grid(rate, Rounding::Down),
        }
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// A solid colour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct SolidContent {
    /// The colour.
    pub color: Color,
}

/// Picture properties of a clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Visual {
    /// Placement on the canvas.
    pub transform: Transform,
    /// Cropping of the source, applied before the transform.
    pub crop: Crop,
    /// Opacity, `0..=1`.
    pub opacity: Animated<f64>,
    /// How the clip combines with the layers below.
    pub blend_mode: BlendMode,
    /// Effects, applied in order.
    pub effects: Vec<EffectInstance>,
}

impl Default for Visual {
    /// Centred, fitted, opaque, no effects.
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            crop: Crop::default(),
            opacity: Animated::constant(1.0),
            blend_mode: BlendMode::Normal,
            effects: Vec::new(),
        }
    }
}

/// Placement of a clip on the canvas.
///
/// Coordinates are fractions of the canvas, origin at its centre, `+y` down:
/// `(0.5, 0)` puts the anchor on the right edge. Resolution-independent, so a
/// timeline can be resized without moving anything.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Transform {
    /// Where the anchor point sits on the canvas.
    pub position: Animated<Vec2>,
    /// Size relative to fitting the canvas; `(1, 1)` fits.
    pub scale: Animated<Vec2>,
    /// Clockwise rotation in degrees.
    pub rotation: Animated<f64>,
    /// Pivot for scale and rotation, as fractions of the clip from its
    /// centre.
    pub anchor: Vec2,
    /// Mirrors left to right.
    pub flip_horizontal: bool,
    /// Mirrors top to bottom.
    pub flip_vertical: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Animated::constant(Vec2::ZERO),
            scale: Animated::constant(Vec2::ONE),
            rotation: Animated::constant(0.0),
            anchor: Vec2::ZERO,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

/// How much of each edge of the source to cut away, as fractions of its
/// size; each `0..1`, and opposite edges sum to less than `1`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Crop {
    /// Cut from the left edge.
    pub left: f64,
    /// Cut from the top edge.
    pub top: f64,
    /// Cut from the right edge.
    pub right: f64,
    /// Cut from the bottom edge.
    pub bottom: f64,
}

/// Compositing blend modes (WGSL implementations in `effects/`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum BlendMode {
    /// Source over.
    #[default]
    Normal,
    /// Darkens by multiplying.
    Multiply,
    /// Lightens by inverse multiplying.
    Screen,
    /// Multiply or screen depending on the base.
    Overlay,
    /// Keeps the darker colour.
    Darken,
    /// Keeps the lighter colour.
    Lighten,
    /// Adds colours (linear dodge).
    Add,
}

/// Sound properties of a clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct AudioMix {
    /// Gain in decibels; `0` leaves the level unchanged.
    pub gain_db: Animated<f64>,
    /// Fade-in length from the clip start, `>= 0`.
    pub fade_in: Time,
    /// Fade-out length before the clip end, `>= 0`.
    pub fade_out: Time,
}

impl Default for AudioMix {
    /// Unity gain, no fades.
    fn default() -> Self {
        Self {
            gain_db: Animated::constant(0.0),
            fade_in: Time::ZERO,
            fade_out: Time::ZERO,
        }
    }
}

/// An effect applied to a clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct EffectInstance {
    /// Stable identifier of this application of the effect.
    pub id: EffectInstanceId,
    /// Which effect.
    pub effect: EffectRef,
    /// A disabled effect is kept but skipped.
    pub enabled: bool,
    /// Parameter values by name; parameters left out take the manifest's
    /// default.
    pub params: Params<Animated<ParamValue>>,
}

/// A reference to an effect package (`effects/<id>/effect.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct EffectRef {
    /// Dotted identifier, e.g. `"kinora.gaussianBlur"`.
    pub id: String,
    /// Manifest version the parameters were written for; the effect migrates
    /// older parameters when it loads them.
    pub version: u32,
}

/// A transition across the cut between two adjacent clips on one track,
/// centred on the cut.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Transition {
    /// Stable identifier.
    pub id: TransitionId,
    /// The clip that ends at the cut.
    pub from: ClipId,
    /// The clip that starts at the cut.
    pub to: ClipId,
    /// Length, `> 0` and no longer than either clip.
    pub duration: Time,
    /// Which transition effect.
    pub effect: EffectRef,
    /// Parameter values by name.
    pub params: Params<ParamValue>,
}

/// A named point or range on a timeline (also exported as chapters).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Marker {
    /// Stable identifier.
    pub id: MarkerId,
    /// Position, `>= 0`.
    pub start: Time,
    /// Length; `0` for a point marker.
    pub duration: Time,
    /// Label shown on the ruler.
    pub label: String,
    /// Colour tag.
    pub color: MarkerColor,
}

/// Marker colour tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum MarkerColor {
    /// Red.
    Red,
    /// Orange.
    Orange,
    /// Yellow.
    Yellow,
    /// Green.
    Green,
    /// Blue.
    #[default]
    Blue,
    /// Purple.
    Purple,
}
