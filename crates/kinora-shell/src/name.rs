//! Project names. A name is also the folder name (`<name>.kinora`), so it
//! must be a single file name that is valid and unambiguous on Windows,
//! where the rules are strictest; the same rules apply everywhere, so a
//! project folder copied between systems keeps working.

/// Longest name accepted, in characters (Unicode scalar values). Well below
/// the 255 UTF-16 units of a file name, leaving room for the extension and
/// for deep folders under the legacy 260-character path limit.
pub const MAX_NAME_CHARS: usize = 100;

/// Characters Windows does not allow in file names.
const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Why a name cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The name is empty.
    #[error("the name is empty")]
    Empty,
    /// The name is longer than [`MAX_NAME_CHARS`].
    #[error("the name is longer than {MAX_NAME_CHARS} characters")]
    TooLong,
    /// The name contains a character file names cannot hold: `< > : " / \ |
    /// ? *`, a control character, or an invisible text-direction override.
    #[error("the name contains {0:?}, which file names cannot hold")]
    Character(char),
    /// The name starts or ends with white space, which Windows drops or
    /// refuses.
    #[error("the name starts or ends with a space")]
    Space,
    /// The name ends with a dot, which Windows drops.
    #[error("the name ends with a dot")]
    TrailingDot,
    /// The name is a Windows device name (`CON`, `NUL`, `COM1`, …), with or
    /// without an extension.
    #[error("the name is reserved by Windows")]
    Reserved,
}

/// Checks that `name` can be a project folder name.
///
/// # Errors
/// The first rule the name breaks.
pub fn validate(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(NameError::TooLong);
    }
    if let Some(c) = name.chars().find(|&c| is_forbidden(c)) {
        return Err(NameError::Character(c));
    }
    if name.starts_with(char::is_whitespace) || name.ends_with(char::is_whitespace) {
        return Err(NameError::Space);
    }
    if name.ends_with('.') {
        return Err(NameError::TrailingDot);
    }
    if is_reserved(name) {
        return Err(NameError::Reserved);
    }
    Ok(())
}

fn is_forbidden(c: char) -> bool {
    // Bidirectional overrides and isolates can make a name display as
    // something else (`txt.exe` shown as `exe.txt`).
    FORBIDDEN.contains(&c)
        || c.is_control()
        || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Windows resolves these to devices whatever follows the first dot, and
/// ignores spaces before it: `nul.kinora` and `COM1 .x` are devices too.
fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    let stem = stem.to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    ["COM", "LPT"].iter().any(|device| {
        stem.strip_prefix(device).is_some_and(|digit| {
            let mut chars = digit.chars();
            matches!(
                (chars.next(), chars.next()),
                (Some('0'..='9' | '¹' | '²' | '³'), None)
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn ordinary_names_are_accepted() {
        for name in [
            "Holiday",
            "Liburan ke Bali 2026",
            "Ulang tahun Ibu (final)",
            "video.v2",
            ".hidden",
            "Café — été",
            "日本旅行",
            "COM10",
            "Console",
            "nul-free",
            "a",
        ] {
            assert_eq!(validate(name), Ok(()), "{name:?}");
        }
        assert_eq!(validate(&"x".repeat(MAX_NAME_CHARS)), Ok(()));
    }

    #[test]
    fn unsafe_names_are_refused() {
        let cases = [
            ("", NameError::Empty),
            ("a/b", NameError::Character('/')),
            ("a\\b", NameError::Character('\\')),
            ("..\\up", NameError::Character('\\')),
            ("what?", NameError::Character('?')),
            ("C:", NameError::Character(':')),
            ("tab\there", NameError::Character('\t')),
            ("bell\u{7}", NameError::Character('\u{7}')),
            ("evil\u{202E}txt", NameError::Character('\u{202E}')),
            (" leading", NameError::Space),
            ("trailing ", NameError::Space),
            ("ideographic\u{3000}", NameError::Space),
            ("ends.", NameError::TrailingDot),
            (".", NameError::TrailingDot),
            ("..", NameError::TrailingDot),
            ("CON", NameError::Reserved),
            ("con", NameError::Reserved),
            ("Nul.kinora", NameError::Reserved),
            ("aux .txt", NameError::Reserved),
            ("COM1", NameError::Reserved),
            ("lpt9", NameError::Reserved),
            ("COM¹", NameError::Reserved),
            ("CONIN$", NameError::Reserved),
        ];
        for (name, expected) in cases {
            assert_eq!(validate(name), Err(expected), "{name:?}");
        }
        let long = "x".repeat(MAX_NAME_CHARS + 1);
        assert_eq!(validate(&long), Err(NameError::TooLong));
    }

    proptest! {
        /// Every accepted name works as a real folder name on this system,
        /// and is the folder's whole name (no separators, no `..`).
        #[test]
        fn accepted_names_make_real_folders(
            name in "[a-zA-Z0-9 ._()$¹é日:?\u{202E}\u{3000}-]{1,12}"
        ) {
            prop_assume!(validate(&name).is_ok());
            let dir = tempfile::tempdir().expect("temp dir");
            std::fs::create_dir(dir.path().join(format!("{name}.kinora")))
                .expect("creates the folder");
            let listed: Vec<_> = std::fs::read_dir(dir.path())
                .expect("lists")
                .map(|entry| entry.expect("entry").file_name())
                .collect();
            prop_assert_eq!(listed, vec![std::ffi::OsString::from(format!("{name}.kinora"))]);
        }
    }
}
