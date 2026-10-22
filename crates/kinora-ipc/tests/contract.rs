//! The checked-in bindings match the contract, and the contract follows the
//! naming rules of Tauri 2.

use std::fs;
use std::path::PathBuf;

use kinora_ipc::bindings::{BINDINGS_PATH, typescript};
use kinora_ipc::{COMMAND_NAMES, EVENT_NAMES};

#[test]
fn bindings_are_up_to_date() {
    let expected = typescript().expect("bindings render");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(BINDINGS_PATH);
    let actual = fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    if actual != expected {
        let line = actual
            .lines()
            .zip(expected.lines())
            .position(|(a, e)| a != e)
            .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
        panic!(
            "{BINDINGS_PATH} is stale (first difference at line {}).\n\
             Run `cargo run -p kinora-ipc --example generate` and commit the result.",
            line + 1
        );
    }
}

#[test]
fn command_names_are_rust_function_names() {
    for name in COMMAND_NAMES {
        assert!(
            name.starts_with(|c: char| c.is_ascii_lowercase())
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "{name}"
        );
    }
}

#[test]
fn event_names_follow_tauri_rules() {
    for name in EVENT_NAMES {
        assert!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_')),
            "{name}"
        );
    }
}

#[test]
fn names_are_unique() {
    for names in [COMMAND_NAMES, EVENT_NAMES] {
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }
}
