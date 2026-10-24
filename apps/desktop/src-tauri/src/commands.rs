//! The IPC commands of `kinora-ipc`, each a thin wrapper over the matching
//! [`Shell`] method.
//!
//! Every command takes the contract's `args` (or nothing) and resolves to
//! the contract's output or rejects with an [`IpcError`] value, never a
//! string. The shell runs on a blocking thread, behind a mutex: file I/O and
//! validation never block the window or the async runtime, and commands run
//! one at a time, in the order they arrive.

use std::sync::{Arc, Mutex, PoisonError};

use kinora_ipc::command::{
    ApplyCommand, CancelJob, CloseProject, CreateProject, ForgetRecentProject, GetProject,
    ListRecentProjects, OpenProject, Redo, SaveProject, Undo,
};
use kinora_ipc::{IpcCommand, IpcError};
use kinora_shell::{Shell, internal_error};
use tauri::State;

/// The app's single shell, shared by every command.
pub struct AppState {
    shell: Arc<Mutex<Shell>>,
}

impl AppState {
    /// Wraps the shell the app starts with.
    pub fn new(shell: Shell) -> Self {
        Self {
            shell: Arc::new(Mutex::new(shell)),
        }
    }

    /// Runs `operation` on the shell, on a blocking thread.
    async fn run<T, F>(&self, operation: F) -> Result<T, IpcError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Shell) -> Result<T, IpcError> + Send + 'static,
    {
        let shell = Arc::clone(&self.shell);
        let task = tauri::async_runtime::spawn_blocking(move || {
            // A panic inside the shell cannot leave a half-written file (all
            // writes are atomic or append-only), so the state stays usable.
            let mut shell = shell.lock().unwrap_or_else(PoisonError::into_inner);
            operation(&mut shell)
        });
        match task.await {
            Ok(result) => result,
            Err(error) => Err(internal_error("a command task failed", &error)),
        }
    }
}

/// The arguments of command `C`.
type Args<C> = <C as IpcCommand>::Args;

/// The result of command `C`.
type Output<C> = Result<<C as IpcCommand>::Output, IpcError>;

/// See [`CreateProject`].
#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    args: Args<CreateProject>,
) -> Output<CreateProject> {
    state.run(move |shell| shell.create_project(args)).await
}

/// See [`OpenProject`].
#[tauri::command]
pub async fn open_project(
    state: State<'_, AppState>,
    args: Args<OpenProject>,
) -> Output<OpenProject> {
    state.run(move |shell| shell.open_project(args)).await
}

/// See [`SaveProject`].
#[tauri::command]
pub async fn save_project(
    state: State<'_, AppState>,
    args: Args<SaveProject>,
) -> Output<SaveProject> {
    state.run(move |shell| shell.save_project(args)).await
}

/// See [`CloseProject`].
#[tauri::command]
pub async fn close_project(
    state: State<'_, AppState>,
    args: Args<CloseProject>,
) -> Output<CloseProject> {
    state.run(move |shell| shell.close_project(args)).await
}

/// See [`GetProject`].
#[tauri::command]
pub async fn get_project(state: State<'_, AppState>) -> Output<GetProject> {
    state.run(|shell| shell.get_project()).await
}

/// See [`ApplyCommand`].
#[tauri::command]
pub async fn apply_command(
    state: State<'_, AppState>,
    args: Args<ApplyCommand>,
) -> Output<ApplyCommand> {
    state.run(move |shell| shell.apply_command(args)).await
}

/// See [`Undo`].
#[tauri::command]
pub async fn undo(state: State<'_, AppState>, args: Args<Undo>) -> Output<Undo> {
    state.run(move |shell| shell.undo(args)).await
}

/// See [`Redo`].
#[tauri::command]
pub async fn redo(state: State<'_, AppState>, args: Args<Redo>) -> Output<Redo> {
    state.run(move |shell| shell.redo(args)).await
}

/// See [`CancelJob`].
#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>, args: Args<CancelJob>) -> Output<CancelJob> {
    state.run(move |shell| shell.cancel_job(args)).await
}

/// See [`ListRecentProjects`].
#[tauri::command]
pub async fn list_recent_projects(state: State<'_, AppState>) -> Output<ListRecentProjects> {
    state.run(|shell| shell.list_recent_projects()).await
}

/// See [`ForgetRecentProject`].
#[tauri::command]
pub async fn forget_recent_project(
    state: State<'_, AppState>,
    args: Args<ForgetRecentProject>,
) -> Output<ForgetRecentProject> {
    state
        .run(move |shell| shell.forget_recent_project(args))
        .await
}
