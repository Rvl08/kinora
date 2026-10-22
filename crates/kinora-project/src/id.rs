//! Stable identifiers for every entity in the document.
//!
//! Each kind of entity has its own newtype over a UUID, so a `ClipId` can never
//! be passed where a `TrackId` is expected. New identifiers are UUIDv7: unique
//! without coordination (the UI, the Copilot and scripts can mint them inside
//! an [`EditCommand`](crate::EditCommand), which keeps journal replay
//! deterministic) and time-ordered, which keeps diffs and database indexes
//! tidy. Any UUID version is accepted when reading.
//!
//! Serialised as the hyphenated lowercase string,
//! e.g. `"0192f1c4-6a4e-7cc0-8b9e-3f5d2a1b4c6d"`.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

macro_rules! define_id {
    ($( $(#[$doc:meta])* $name:ident; )*) => {$(
        $(#[$doc])*
        #[derive(
            Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            serde::Serialize, serde::Deserialize, schemars::JsonSchema,
        )]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
        pub struct $name(Uuid);

        impl $name {
            /// A new, unique, time-ordered identifier (UUIDv7).
            pub fn generate() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an existing UUID.
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// The underlying UUID.
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0.hyphenated())
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }
    )*};
}

define_id! {
    /// Identifies a project. Survives "save as" so caches can be shared.
    ProjectId;
    /// Identifies an asset in the media pool.
    MediaId;
    /// Identifies a timeline (a sequence) within the project.
    TimelineId;
    /// Identifies a track within a timeline.
    TrackId;
    /// Identifies a clip on a track.
    ClipId;
    /// Identifies one applied effect on a clip.
    EffectInstanceId;
    /// Identifies a transition between two clips.
    TransitionId;
    /// Identifies a marker on a timeline.
    MarkerId;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_unique_v7() {
        let a = ClipId::generate();
        let b = ClipId::generate();
        assert_eq!(a.as_uuid().get_version_num(), 7);
        assert_ne!(a, b);
    }

    #[test]
    fn serialises_as_a_plain_string() {
        let id: TrackId = "0192f1c4-6a4e-7cc0-8b9e-3f5d2a1b4c6d"
            .parse()
            .expect("valid uuid");
        let json = serde_json::to_string(&id).expect("serialise");
        assert_eq!(json, r#""0192f1c4-6a4e-7cc0-8b9e-3f5d2a1b4c6d""#);
        assert_eq!(serde_json::from_str::<TrackId>(&json).expect("parse"), id);
        assert_eq!(
            format!("{id:?}"),
            "TrackId(0192f1c4-6a4e-7cc0-8b9e-3f5d2a1b4c6d)"
        );
        assert!(serde_json::from_str::<TrackId>(r#""not-a-uuid""#).is_err());
    }
}
