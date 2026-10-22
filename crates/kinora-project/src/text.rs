//! Text and caption clip content.
//!
//! Sizes and distances are relative, so a title keeps its look when the
//! timeline is resized (16:9 to 9:16, 1080p to 4K): font size is a fraction of
//! the canvas height, and stroke, shadow and padding are in ems (multiples of
//! the font size).

use kinora_core::Time;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::value::{Color, Vec2};

/// A styled block of text (titles, labels, lower thirds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct TextContent {
    /// The text; `\n` breaks lines.
    pub text: String,
    /// How it looks.
    pub style: TextStyle,
}

/// One caption cue, usually produced by transcription (auto captions) and
/// then edited.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct CaptionContent {
    /// The cue text as displayed.
    pub text: String,
    /// Word timings for karaoke-style highlighting; empty when unknown.
    /// Ordered, non-overlapping and within the clip.
    pub words: Vec<CaptionWord>,
    /// How it looks.
    pub style: TextStyle,
    /// BCP 47 language tag (`"id"`, `"en-US"`), when known.
    pub language: Option<String>,
}

/// A word inside a caption cue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct CaptionWord {
    /// The word as displayed.
    pub text: String,
    /// When the word starts, relative to the clip start.
    pub start: Time,
    /// When the word ends, relative to the clip start; `>= start`.
    pub end: Time,
}

/// Typography and decoration of text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct TextStyle {
    /// Typeface.
    pub font: FontRef,
    /// Font size as a fraction of the canvas height, `> 0`.
    pub size: f64,
    /// Fill colour.
    pub color: Color,
    /// Horizontal alignment of lines.
    pub align: TextAlign,
    /// Line height as a multiple of the font size, `> 0`.
    pub line_height: f64,
    /// Extra space between letters, in ems.
    pub letter_spacing: f64,
    /// Outline, if any.
    pub stroke: Option<Stroke>,
    /// Drop shadow, if any.
    pub shadow: Option<Shadow>,
    /// Box behind the text, if any.
    pub background: Option<TextBackground>,
}

/// A typeface by family name. Fonts are resolved at render time from the
/// bundled open-licence fonts, then the system; a missing font falls back
/// and is reported, never silently substituted in the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct FontRef {
    /// Family name, e.g. `"Inter"`.
    pub family: String,
    /// CSS weight, `100..=900`.
    pub weight: u16,
    /// Italic style.
    pub italic: bool,
}

/// Horizontal alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub enum TextAlign {
    /// Flush left.
    Left,
    /// Centred.
    #[default]
    Center,
    /// Flush right.
    Right,
}

/// An outline around glyphs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Stroke {
    /// Outline colour.
    pub color: Color,
    /// Outline width in ems, `> 0`.
    pub width: f64,
}

/// A drop shadow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct Shadow {
    /// Shadow colour.
    pub color: Color,
    /// Offset in ems.
    pub offset: Vec2,
    /// Blur radius in ems, `>= 0`.
    pub blur: f64,
}

/// A box drawn behind the text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct TextBackground {
    /// Box colour.
    pub color: Color,
    /// Padding around the text in ems, `>= 0`.
    pub padding: f64,
    /// Corner radius in ems, `>= 0`.
    pub corner_radius: f64,
}
