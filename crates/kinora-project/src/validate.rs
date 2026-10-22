//! Semantic checks: the rules of a well-formed document that the types and
//! the JSON Schema cannot express.

use std::collections::HashSet;
use std::fmt;

use kinora_core::{MAX_SAFE_INTEGER, Time};
use uuid::Uuid;

use crate::media::{MediaAsset, MediaKind, MediaOrigin};
use crate::model::{
    AudioMix, Clip, ClipContent, Crop, EffectRef, MediaContent, Project, Speed, Timeline, Track,
    TrackKind, VideoFormat, Visual,
};
use crate::text::{CaptionContent, TextStyle};
use crate::value::{Animated, Color, Easing, ParamValue, Vec2};

/// Largest canvas edge accepted, in pixels.
const MAX_CANVAS_EDGE: u32 = 16_384;

/// A rule a document breaks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// JSON Pointer (RFC 6901) to the offending value inside `project.json`,
    /// e.g. `/timelines/0/tracks/1/clips/3/duration`.
    pub path: String,
    /// What is wrong, in English, for logs and developer tools. The UI shows
    /// its own localised text.
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Checks every semantic rule and returns all violations; an empty list means
/// the document is well-formed.
pub fn validate(project: &Project) -> Vec<Issue> {
    let mut checker = Checker {
        project,
        issues: Vec::new(),
        ids: HashSet::new(),
    };
    checker.project();
    checker.issues
}

struct Checker<'a> {
    project: &'a Project,
    issues: Vec<Issue>,
    /// Every identifier seen. Where it was first seen is looked up only
    /// for a duplicate ([`first_use`]), so a valid document allocates no
    /// path per entity.
    ids: HashSet<Uuid>,
}

impl Checker<'_> {
    fn issue(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(Issue {
            path: path.into(),
            message: message.into(),
        });
    }

    fn check(&mut self, ok: bool, path: impl FnOnce() -> String, message: &str) {
        if !ok {
            self.issue(path(), message);
        }
    }

    /// Identifiers are unique across the whole document, whatever their kind.
    fn id(&mut self, id: Uuid, path: impl FnOnce() -> String) {
        if !self.ids.insert(id) {
            let first = first_use(self.project, id).unwrap_or_default();
            self.issue(
                path(),
                format!("identifier {id} is already used at {first}"),
            );
        }
    }

    fn project(&mut self) {
        let project = self.project;
        self.id(project.id.as_uuid(), || "/id".into());
        self.format("/settings/defaultFormat", project.settings.default_format);
        let rate = project.settings.audio.sample_rate;
        self.check(
            (8_000..=192_000).contains(&rate),
            || "/settings/audio/sampleRate".into(),
            "sample rate must be within 8000..=192000",
        );
        for (i, asset) in project.media.iter().enumerate() {
            self.media(&format!("/media/{i}"), asset);
        }
        self.check(
            !project.timelines.is_empty(),
            || "/timelines".into(),
            "a project needs at least one timeline",
        );
        for (i, timeline) in project.timelines.iter().enumerate() {
            self.timeline(&format!("/timelines/{i}"), timeline);
        }
    }

    fn format(&mut self, path: &str, format: VideoFormat) {
        for (name, edge) in [("width", format.width), ("height", format.height)] {
            self.check(
                (2..=MAX_CANVAS_EDGE).contains(&edge) && edge % 2 == 0,
                || format!("{path}/{name}"),
                "canvas edges must be even and within 2..=16384",
            );
        }
        self.check(
            format.frame_rate.is_grid_aligned(),
            || format!("{path}/frameRate"),
            "a frame must last a whole number of flicks (1/705600000 s)",
        );
    }

    /// Times on the timeline axis lie on the edit grid (ADR-0004 amendment 2).
    fn grid(&mut self, path: impl FnOnce() -> String, time: Time) {
        self.check(
            time.is_on_grid(),
            path,
            "time must be a whole number of flicks (1/705600000 s) within ±(2^53 - 1)",
        );
    }

    fn media(&mut self, path: &str, asset: &MediaAsset) {
        self.id(asset.id.as_uuid(), || format!("{path}/id"));

        let digest = &asset.hash.digest;
        self.check(
            digest.len() == 32
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            || format!("{path}/hash/digest"),
            "digest must be 32 lowercase hexadecimal characters",
        );
        self.check(
            i64::try_from(asset.hash.file_size).is_ok_and(|size| size <= MAX_SAFE_INTEGER),
            || format!("{path}/hash/fileSize"),
            "file size must not exceed 2^53 - 1",
        );
        if let Some(relative) = &asset.location.relative_path {
            self.check(
                is_portable_relative_path(relative),
                || format!("{path}/location/relativePath"),
                "relative path must be relative and use `/` separators",
            );
        }

        let info = &asset.info;
        let duration_ok = match (asset.kind, info.duration) {
            (MediaKind::Image, duration) => duration.is_none(),
            (_, Some(duration)) => duration.is_positive(),
            (_, None) => false,
        };
        self.check(
            duration_ok,
            || format!("{path}/info/duration"),
            "images have no duration; video and audio need a positive one",
        );
        let needs_video = matches!(asset.kind, MediaKind::Video | MediaKind::Image);
        self.check(
            info.video.is_some() == needs_video,
            || format!("{path}/info/video"),
            "video and images need video stream info; audio files have none",
        );
        if let Some(video) = &info.video {
            self.check(
                video.width > 0 && video.height > 0,
                || format!("{path}/info/video"),
                "stream dimensions must be positive",
            );
        }
        let audio_ok = match asset.kind {
            MediaKind::Audio => info.audio.is_some(),
            MediaKind::Image => info.audio.is_none(),
            MediaKind::Video => true,
        };
        self.check(
            audio_ok,
            || format!("{path}/info/audio"),
            "audio files need audio stream info; images have none",
        );
        if let Some(audio) = &info.audio {
            self.check(
                audio.sample_rate > 0 && audio.channels > 0,
                || format!("{path}/info/audio"),
                "sample rate and channel count must be positive",
            );
        }

        if let MediaOrigin::Generated { generator, source } = &asset.origin {
            self.check(
                !generator.trim().is_empty(),
                || format!("{path}/origin/generator"),
                "generator must not be empty",
            );
            if let Some(source) = source {
                let project = self.project;
                let exists = *source != asset.id && project.media(*source).is_some();
                self.check(
                    exists,
                    || format!("{path}/origin/source"),
                    "source must be another asset in the media pool",
                );
            }
        }
    }

    fn timeline(&mut self, path: &str, timeline: &Timeline) {
        self.id(timeline.id.as_uuid(), || format!("{path}/id"));
        self.format(&format!("{path}/format"), timeline.format);
        for (i, track) in timeline.tracks.iter().enumerate() {
            self.track(&format!("{path}/tracks/{i}"), track);
        }
        let mut previous = Time::ZERO;
        for (i, marker) in timeline.markers.iter().enumerate() {
            let path = format!("{path}/markers/{i}");
            self.id(marker.id.as_uuid(), || format!("{path}/id"));
            self.check(
                marker.start >= previous,
                || format!("{path}/start"),
                "markers must be ordered by start, from 0",
            );
            self.grid(|| format!("{path}/start"), marker.start);
            self.grid(|| format!("{path}/duration"), marker.duration);
            self.check(
                !marker.duration.is_negative(),
                || format!("{path}/duration"),
                "duration must not be negative",
            );
            previous = marker.start;
        }
    }

    fn track(&mut self, path: &str, track: &Track) {
        self.id(track.id.as_uuid(), || format!("{path}/id"));
        let mut previous_end = Time::ZERO;
        for (i, clip) in track.clips.iter().enumerate() {
            let clip_path = format!("{path}/clips/{i}");
            self.clip(&clip_path, track.kind, clip);
            self.check(
                clip.start >= previous_end,
                || format!("{clip_path}/start"),
                "clips must be ordered by start and must not overlap",
            );
            if let Some(end) = clip.end() {
                previous_end = end;
            }
        }
        // A clip ends at one cut and starts at another, so it may appear once
        // as `from` and once as `to`.
        let (mut outgoing, mut incoming) = (HashSet::new(), HashSet::new());
        for (i, transition) in track.transitions.iter().enumerate() {
            let path = format!("{path}/transitions/{i}");
            let first_out = outgoing.insert(transition.from);
            let first_in = incoming.insert(transition.to);
            self.check(
                first_out && first_in,
                || path.clone(),
                "a cut has at most one transition",
            );
            self.id(transition.id.as_uuid(), || format!("{path}/id"));
            self.effect_ref(&format!("{path}/effect"), &transition.effect);
            let find = |id| track.clips.iter().find(|c| c.id == id);
            let (Some(from), Some(to)) = (find(transition.from), find(transition.to)) else {
                self.issue(path, "`from` and `to` must be clips of this track");
                continue;
            };
            self.check(
                from.end() == Some(to.start),
                || path.clone(),
                "`from` must end exactly where `to` starts",
            );
            let d = transition.duration;
            self.grid(|| format!("{path}/duration"), d);
            self.check(
                d.is_positive() && d <= from.duration && d <= to.duration,
                || format!("{path}/duration"),
                "duration must be positive and fit in both clips",
            );
        }
    }

    fn clip(&mut self, path: &str, track: TrackKind, clip: &Clip) {
        self.id(clip.id.as_uuid(), || format!("{path}/id"));
        self.check(
            !clip.start.is_negative(),
            || format!("{path}/start"),
            "start must not be negative",
        );
        self.check(
            clip.duration.is_positive(),
            || format!("{path}/duration"),
            "duration must be positive",
        );
        self.grid(|| format!("{path}/start"), clip.start);
        self.grid(|| format!("{path}/duration"), clip.duration);
        self.check(
            clip.end().is_some_and(Time::is_on_grid),
            || format!("{path}/duration"),
            "start + duration is out of range",
        );

        let media_kind = match &clip.content {
            ClipContent::Media(content) => self.media_content(path, clip.duration, content),
            _ => None,
        };
        let fits = match (&clip.content, track) {
            (ClipContent::Media(_), TrackKind::Video) => {
                media_kind.is_none_or(|kind| kind != MediaKind::Audio)
            }
            (ClipContent::Media(_), TrackKind::Audio) => {
                media_kind.is_none_or(|kind| kind != MediaKind::Image)
            }
            (ClipContent::Solid(_) | ClipContent::Adjustment, TrackKind::Video)
            | (ClipContent::Text(_), TrackKind::Text)
            | (ClipContent::Caption(_), TrackKind::Caption) => true,
            _ => false,
        };
        self.check(
            fits,
            || format!("{path}/content"),
            "this content cannot be placed on this kind of track",
        );

        let visual_expected = track != TrackKind::Audio;
        self.check(
            clip.visual.is_some() == visual_expected,
            || format!("{path}/visual"),
            "visual is required on video, text and caption tracks and absent on audio tracks",
        );
        let audio_ok = match track {
            TrackKind::Audio => clip.audio.is_some(),
            TrackKind::Video => clip.audio.is_none() || media_kind == Some(MediaKind::Video),
            TrackKind::Text | TrackKind::Caption => clip.audio.is_none(),
        };
        self.check(
            audio_ok,
            || format!("{path}/audio"),
            "audio is required on audio tracks, allowed for video media and absent otherwise",
        );

        if let Some(visual) = &clip.visual {
            self.visual(&format!("{path}/visual"), visual);
        }
        if let Some(audio) = &clip.audio {
            self.audio(&format!("{path}/audio"), clip.duration, audio);
        }
        match &clip.content {
            ClipContent::Text(text) => {
                self.text_style(&format!("{path}/content/style"), &text.style)
            }
            ClipContent::Caption(caption) => self.caption(path, caption),
            ClipContent::Solid(solid) => self.color(&format!("{path}/content/color"), solid.color),
            ClipContent::Media(_) | ClipContent::Adjustment => {}
        }
    }

    /// Checks a media clip's source range; returns the asset's kind when the
    /// asset exists.
    fn media_content(
        &mut self,
        path: &str,
        duration: Time,
        content: &MediaContent,
    ) -> Option<MediaKind> {
        let Speed::Constant { rate } = content.speed;
        self.check(
            rate.is_positive(),
            || format!("{path}/content/speed/rate"),
            "speed must be positive",
        );
        self.check(
            !content.source_in.is_negative(),
            || format!("{path}/content/sourceIn"),
            "source in-point must not be negative",
        );
        self.grid(|| format!("{path}/content/sourceIn"), content.source_in);
        let project = self.project;
        let Some(asset) = project.media(content.media) else {
            self.issue(
                format!("{path}/content/media"),
                "media must be an asset of the media pool",
            );
            return None;
        };
        if let Some(media_duration) = asset.info.duration {
            let source_end = content
                .speed
                .source_duration(duration)
                .and_then(|d| content.source_in.checked_add(d));
            self.check(
                source_end.is_some_and(|end| end <= media_duration),
                || format!("{path}/content"),
                "the source range runs past the end of the media",
            );
        }
        Some(asset.kind)
    }

    fn visual(&mut self, path: &str, visual: &Visual) {
        let transform = &visual.transform;
        self.animated(
            &format!("{path}/transform/position"),
            &transform.position,
            |c, p, v| {
                c.vec2(p, *v);
            },
        );
        self.animated(
            &format!("{path}/transform/scale"),
            &transform.scale,
            |c, p, v| {
                c.vec2(p, *v);
            },
        );
        self.animated(
            &format!("{path}/transform/rotation"),
            &transform.rotation,
            |c, p, v| {
                c.finite(p, *v);
            },
        );
        self.vec2(&format!("{path}/transform/anchor"), transform.anchor);
        self.crop(&format!("{path}/crop"), visual.crop);
        self.animated(&format!("{path}/opacity"), &visual.opacity, |c, p, v| {
            c.check(
                (0.0..=1.0).contains(v),
                || p.to_owned(),
                "opacity must be within 0..=1",
            );
        });
        for (i, effect) in visual.effects.iter().enumerate() {
            let path = format!("{path}/effects/{i}");
            self.id(effect.id.as_uuid(), || format!("{path}/id"));
            self.effect_ref(&format!("{path}/effect"), &effect.effect);
            for (name, value) in &effect.params {
                let param_path = format!("{path}/params/{}", escape_pointer(name));
                self.check(
                    !name.is_empty(),
                    || param_path.clone(),
                    "parameter names must not be empty",
                );
                let first_type = value.values().next().map(ParamValue::type_name);
                self.animated(&param_path, value, |c, p, v| {
                    c.check(
                        Some(v.type_name()) == first_type,
                        || p.to_owned(),
                        "every keyframe of a parameter must have the same type",
                    );
                    c.param(p, v);
                });
            }
        }
    }

    fn audio(&mut self, path: &str, duration: Time, audio: &AudioMix) {
        self.animated(&format!("{path}/gainDb"), &audio.gain_db, |c, p, v| {
            c.finite(p, *v);
        });
        self.grid(|| format!("{path}/fadeIn"), audio.fade_in);
        self.grid(|| format!("{path}/fadeOut"), audio.fade_out);
        let fades = audio.fade_in.checked_add(audio.fade_out);
        self.check(
            !audio.fade_in.is_negative()
                && !audio.fade_out.is_negative()
                && fades.is_some_and(|f| f <= duration),
            || path.to_owned(),
            "fades must not be negative and must fit in the clip together",
        );
    }

    /// Word times may lie outside the clip after a trim or split: they keep
    /// their place in the speech, and only the words inside the clip show.
    fn caption(&mut self, path: &str, caption: &CaptionContent) {
        self.text_style(&format!("{path}/content/style"), &caption.style);
        let mut previous_end: Option<Time> = None;
        for (i, word) in caption.words.iter().enumerate() {
            let word_path = || format!("{path}/content/words/{i}");
            self.check(
                previous_end.is_none_or(|end| word.start >= end) && word.start <= word.end,
                word_path,
                "words must be ordered and must not overlap",
            );
            self.grid(|| format!("{path}/content/words/{i}/start"), word.start);
            self.grid(|| format!("{path}/content/words/{i}/end"), word.end);
            previous_end = Some(word.end);
        }
    }

    fn text_style(&mut self, path: &str, style: &TextStyle) {
        self.check(
            !style.font.family.trim().is_empty(),
            || format!("{path}/font/family"),
            "font family must not be empty",
        );
        self.check(
            (100..=900).contains(&style.font.weight),
            || format!("{path}/font/weight"),
            "font weight must be within 100..=900",
        );
        self.check(
            style.size > 0.0,
            || format!("{path}/size"),
            "size must be positive",
        );
        self.check(
            style.line_height > 0.0,
            || format!("{path}/lineHeight"),
            "line height must be positive",
        );
        self.finite(&format!("{path}/letterSpacing"), style.letter_spacing);
        self.color(&format!("{path}/color"), style.color);
        if let Some(stroke) = &style.stroke {
            self.color(&format!("{path}/stroke/color"), stroke.color);
            self.check(
                stroke.width > 0.0,
                || format!("{path}/stroke/width"),
                "width must be positive",
            );
        }
        if let Some(shadow) = &style.shadow {
            self.color(&format!("{path}/shadow/color"), shadow.color);
            self.vec2(&format!("{path}/shadow/offset"), shadow.offset);
            self.check(
                shadow.blur >= 0.0,
                || format!("{path}/shadow/blur"),
                "blur must not be negative",
            );
        }
        if let Some(background) = &style.background {
            self.color(&format!("{path}/background/color"), background.color);
            self.check(
                background.padding >= 0.0 && background.corner_radius >= 0.0,
                || format!("{path}/background"),
                "padding and corner radius must not be negative",
            );
        }
    }

    /// Keyframes are non-empty, strictly increasing and on the grid; each
    /// value passes `value`. They may lie outside the clip: a trim or a split
    /// keeps them where they are relative to the content, so the animation
    /// continues across the cut and comes back when the clip is extended.
    fn animated<T>(
        &mut self,
        path: &str,
        animated: &Animated<T>,
        mut value: impl FnMut(&mut Self, &str, &T),
    ) {
        match animated {
            Animated::Static { value: v } => value(self, &format!("{path}/value"), v),
            Animated::Keyframes { keyframes } => {
                self.check(
                    !keyframes.is_empty(),
                    || format!("{path}/keyframes"),
                    "at least one keyframe is required",
                );
                let mut previous: Option<Time> = None;
                for (i, keyframe) in keyframes.iter().enumerate() {
                    let path = format!("{path}/keyframes/{i}");
                    self.check(
                        previous.is_none_or(|p| keyframe.at > p),
                        || format!("{path}/at"),
                        "keyframes must be strictly increasing",
                    );
                    self.grid(|| format!("{path}/at"), keyframe.at);
                    if let Easing::CubicBezier { x1, y1, x2, y2 } = keyframe.easing {
                        self.check(
                            (0.0..=1.0).contains(&x1)
                                && (0.0..=1.0).contains(&x2)
                                && y1.is_finite()
                                && y2.is_finite(),
                            || format!("{path}/easing"),
                            "Bézier x coordinates must be within 0..=1",
                        );
                    }
                    value(self, &format!("{path}/value"), &keyframe.value);
                    previous = Some(keyframe.at);
                }
            }
        }
    }

    fn param(&mut self, path: &str, value: &ParamValue) {
        match value {
            ParamValue::Float(v) => self.finite(path, *v),
            ParamValue::Vec2(v) => self.vec2(path, *v),
            ParamValue::Color(c) => self.color(path, *c),
            ParamValue::Choice(choice) => {
                self.check(
                    !choice.is_empty(),
                    || path.to_owned(),
                    "choice must not be empty",
                );
            }
            ParamValue::Int(_) | ParamValue::Bool(_) => {}
        }
    }

    fn effect_ref(&mut self, path: &str, effect: &EffectRef) {
        self.check(
            is_effect_id(&effect.id),
            || format!("{path}/id"),
            "effect id must be dotted, e.g. `kinora.gaussianBlur`",
        );
    }

    fn crop(&mut self, path: &str, crop: Crop) {
        let edge = |v: f64| (0.0..1.0).contains(&v);
        self.check(
            edge(crop.left)
                && edge(crop.right)
                && edge(crop.top)
                && edge(crop.bottom)
                && crop.left + crop.right < 1.0
                && crop.top + crop.bottom < 1.0,
            || path.to_owned(),
            "crop edges must be within 0..1 and leave part of the picture",
        );
    }

    fn color(&mut self, path: &str, color: Color) {
        self.check(
            color.is_valid(),
            || path.to_owned(),
            "colour components must be within 0..=1",
        );
    }

    fn vec2(&mut self, path: &str, v: Vec2) {
        self.check(
            v.x.is_finite() && v.y.is_finite(),
            || path.to_owned(),
            "coordinates must be finite",
        );
    }

    fn finite(&mut self, path: &str, v: f64) {
        self.check(v.is_finite(), || path.to_owned(), "number must be finite");
    }
}

/// Where `id` is used first, in the order the checker visits entities.
fn first_use(project: &Project, id: Uuid) -> Option<String> {
    if project.id.as_uuid() == id {
        return Some("/id".into());
    }
    if let Some(i) = project.media.iter().position(|m| m.id.as_uuid() == id) {
        return Some(format!("/media/{i}/id"));
    }
    for (t, timeline) in project.timelines.iter().enumerate() {
        let path = format!("/timelines/{t}");
        if timeline.id.as_uuid() == id {
            return Some(format!("{path}/id"));
        }
        for (k, track) in timeline.tracks.iter().enumerate() {
            let path = format!("{path}/tracks/{k}");
            if track.id.as_uuid() == id {
                return Some(format!("{path}/id"));
            }
            for (c, clip) in track.clips.iter().enumerate() {
                if clip.id.as_uuid() == id {
                    return Some(format!("{path}/clips/{c}/id"));
                }
                let effects = clip.visual.iter().flat_map(|v| &v.effects);
                if let Some(e) = effects.into_iter().position(|e| e.id.as_uuid() == id) {
                    return Some(format!("{path}/clips/{c}/visual/effects/{e}/id"));
                }
            }
            if let Some(x) = track.transitions.iter().position(|x| x.id.as_uuid() == id) {
                return Some(format!("{path}/transitions/{x}/id"));
            }
        }
        if let Some(m) = timeline.markers.iter().position(|m| m.id.as_uuid() == id) {
            return Some(format!("{path}/markers/{m}/id"));
        }
    }
    None
}

/// `a.b.c`: lowercase first segment, then segments of ASCII letters, digits
/// and `-`, each starting with a letter.
fn is_effect_id(id: &str) -> bool {
    let mut segments = id.split('.');
    let first_ok = segments.next().is_some_and(|s| {
        s.starts_with(|c: char| c.is_ascii_lowercase())
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    });
    let mut rest = segments.peekable();
    first_ok
        && rest.peek().is_some()
        && rest.all(|s| {
            s.starts_with(|c: char| c.is_ascii_alphabetic())
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// Relative, `/`-separated, no drive letter or root.
fn is_portable_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path.split('/').all(|segment| !segment.is_empty())
}

/// Escapes a map key for use in a JSON Pointer (RFC 6901).
fn escape_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_ids() {
        for ok in [
            "kinora.blur",
            "kinora.gaussianBlur",
            "acme.fx.glow-2",
            "k9.a",
        ] {
            assert!(is_effect_id(ok), "{ok}");
        }
        for bad in [
            "",
            "kinora",
            "Kinora.blur",
            "kinora.",
            ".blur",
            "kinora.2d",
            "kinora.a b",
            "kinora..x",
        ] {
            assert!(!is_effect_id(bad), "{bad}");
        }
    }

    #[test]
    fn relative_paths() {
        for ok in ["media/a.mp4", "../Footage/a.mp4", "a.mp4"] {
            assert!(is_portable_relative_path(ok), "{ok}");
        }
        for bad in [
            "",
            "/abs/a.mp4",
            "C:/a.mp4",
            "media\\a.mp4",
            "media//a.mp4",
            "media/",
        ] {
            assert!(!is_portable_relative_path(bad), "{bad}");
        }
    }

    #[test]
    fn duplicates_name_the_first_use() {
        use crate::model::ProjectSettings;
        let mut project = Project::new("Dup", ProjectSettings::default());
        let mut copy = project.timelines[0].clone();
        copy.id = crate::id::TimelineId::from_uuid(project.id.as_uuid());
        project.timelines.push(copy);
        let issues = validate(&project);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].path, "/timelines/1/id");
        assert!(
            issues[0].message.ends_with("is already used at /id"),
            "{}",
            issues[0].message
        );
    }

    #[test]
    fn pointer_escaping() {
        assert_eq!(escape_pointer("a/b~c"), "a~1b~0c");
    }
}
