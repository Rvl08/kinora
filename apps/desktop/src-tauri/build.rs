//! Runs tauri-build: the Windows resources, the embedded configuration, and
//! the permissions of Kinora's own commands.
//!
//! The command list comes from the contract (`kinora_ipc::COMMAND_NAMES`),
//! so tauri-build generates an `allow-<command>` permission for each, and a
//! command works only in a window whose capability grants it. tauri-build
//! checks that every granted permission exists, not that every command is
//! granted, so the check below fails the build when a command is missing
//! from the main window's capability.

use std::fs;

const CAPABILITY: &str = "capabilities/main.json";

fn main() {
    check_capability();
    let manifest = tauri_build::AppManifest::new().commands(kinora_ipc::COMMAND_NAMES);
    let attributes = tauri_build::Attributes::new().app_manifest(manifest);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build failed: {error:#}");
    }
}

fn check_capability() {
    println!("cargo:rerun-if-changed={CAPABILITY}");
    let capability = match fs::read_to_string(CAPABILITY) {
        Ok(text) => text,
        Err(error) => panic!("cannot read {CAPABILITY}: {error}"),
    };
    let missing: Vec<String> = kinora_ipc::COMMAND_NAMES
        .iter()
        .map(|command| format!("\"allow-{}\"", command.replace('_', "-")))
        .filter(|permission| !capability.contains(permission.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "{CAPABILITY} does not grant {}",
        missing.join(", ")
    );
}
