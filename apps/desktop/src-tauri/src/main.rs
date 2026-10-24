//! Kinora's desktop app: one window showing the Svelte UI, the native file
//! dialogs, and the IPC commands of `kinora-ipc`, each a thin wrapper over
//! `kinora-shell` (see `commands`).

// A release build is a GUI app without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use kinora_ipc::IpcEvent;
use kinora_ipc::event::{JobProgress, JobProgressEvent, ProjectChanged, ProjectChangedEvent};
use kinora_shell::logger::FileLogger;
use kinora_shell::{EventSink, Shell};
use log::LevelFilter;
use tauri::{AppHandle, Emitter, Manager};

/// Registers the IPC commands. Fails to compile unless they are exactly the
/// contract's commands (`kinora_ipc::COMMAND_NAMES`), in the same order.
macro_rules! ipc_handler {
    ($($command:ident),* $(,)?) => {{
        const REGISTERED: &[&str] = &[$(stringify!($command)),*];
        const _: () = assert!(
            same_names(REGISTERED, kinora_ipc::COMMAND_NAMES),
            "the registered commands differ from kinora_ipc::COMMAND_NAMES"
        );
        tauri::generate_handler![$(commands::$command),*]
    }};
}

fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = app.path();
            match paths.app_log_dir() {
                Ok(dir) => start_logging(&dir),
                Err(error) => eprintln!("Kinora has no log folder: {error}"),
            }
            let shell = Shell::new(&paths.app_data_dir()?, WindowEvents(app.handle().clone()));
            if !app.manage(commands::AppState::new(shell)) {
                log::warn!("the shell state was already registered");
            }
            log::info!("Kinora {} started", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .invoke_handler(ipc_handler![
            create_project,
            open_project,
            save_project,
            close_project,
            get_project,
            apply_command,
            undo,
            redo,
            cancel_job,
            list_recent_projects,
            forget_recent_project,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        log::error!("Kinora stopped with an error: {error}");
        eprintln!("Kinora stopped with an error: {error}");
        std::process::exit(1);
    }
}

fn start_logging(dir: &std::path::Path) {
    let level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let installed = FileLogger::open(dir, level)
        .map_err(|error| error.to_string())
        .and_then(|logger| logger.install().map_err(|error| error.to_string()));
    if let Err(error) = installed {
        eprintln!("Kinora cannot write its log: {error}");
    }
}

/// `true` when both lists hold the same names in the same order; usable in
/// constants.
const fn same_names(a: &[&str], b: &[&str]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        let (x, y) = (a[i].as_bytes(), b[i].as_bytes());
        if x.len() != y.len() {
            return false;
        }
        let mut j = 0;
        while j < x.len() {
            if x[j] != y[j] {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Forwards the shell's notifications to the window as Tauri events.
struct WindowEvents(AppHandle);

impl WindowEvents {
    fn emit<E: IpcEvent>(&self, payload: &E::Payload) {
        if let Err(error) = self.0.emit(E::NAME, payload) {
            log::warn!("could not emit {}: {error}", E::NAME);
        }
    }
}

impl EventSink for WindowEvents {
    fn project_changed(&self, event: &ProjectChanged) {
        self.emit::<ProjectChangedEvent>(event);
    }

    fn job_progress(&self, event: &JobProgress) {
        self.emit::<JobProgressEvent>(event);
    }
}
