use serde::Serialize;
use tauri::State;

use crate::error::TwigError;
use crate::git::{reader, writer};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
}

/// Get the working directory status (staged + unstaged file lists).
#[tauri::command]
pub async fn get_working_status(
    state: State<'_, AppState>,
    path: String,
) -> Result<reader::WorkingStatus, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_working_status(&open.repository)
}

/// Get the staged diff, optionally for a single file.
#[tauri::command]
pub async fn get_staged_diff(
    state: State<'_, AppState>,
    path: String,
    file_path: Option<String>,
) -> Result<Vec<reader::DiffFile>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_staged_diff(&open.repository, file_path.as_deref())
}

/// Get the unstaged diff, optionally for a single file.
#[tauri::command]
pub async fn get_unstaged_diff(
    state: State<'_, AppState>,
    path: String,
    file_path: Option<String>,
) -> Result<Vec<reader::DiffFile>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_unstaged_diff(&open.repository, file_path.as_deref())
}

/// Stage files by path.
#[tauri::command]
pub async fn stage_files(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    let output = writer::stage_files(&repo_path, &file_refs).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Unstage files by path.
#[tauri::command]
pub async fn unstage_files(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    let output = writer::unstage_files(&repo_path, &file_refs).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Create a commit with the given message.
#[tauri::command]
pub async fn create_commit(
    state: State<'_, AppState>,
    path: String,
    message: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::commit(&repo_path, &message).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Pull from remote.
#[tauri::command]
pub async fn pull(
    state: State<'_, AppState>,
    path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let remote_name = remote.as_deref().unwrap_or("origin");
    let branch_name = branch.as_deref().unwrap_or("HEAD");
    let output = writer::pull(&repo_path, remote_name, branch_name).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            format!("Pulled successfully\n{}", output.stdout)
        } else {
            output.stderr
        },
    })
}
