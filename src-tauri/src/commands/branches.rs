use serde::Serialize;
use tauri::State;

use crate::error::TwigError;
use crate::git::{reader, writer};
use crate::state::AppState;

/// List all branches (local + remote).
#[tauri::command]
pub async fn get_branches(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<reader::BranchInfo>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_branches(&open.repository)
}

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
}

/// Checkout a branch by name.
#[tauri::command]
pub async fn checkout_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::checkout_branch(&repo_path, &branch_name).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Create a new branch, optionally from a start point.
#[tauri::command]
pub async fn create_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    start_point: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output =
        writer::create_branch(&repo_path, &branch_name, start_point.as_deref()).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Rename a branch.
#[tauri::command]
pub async fn rename_branch(
    state: State<'_, AppState>,
    path: String,
    old_name: String,
    new_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::rename_branch(&repo_path, &old_name, &new_name).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Delete a local branch.
#[tauri::command]
pub async fn delete_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    force: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::delete_branch(&repo_path, &branch_name, force).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Push a branch to a remote.
#[tauri::command]
pub async fn push_branch(
    state: State<'_, AppState>,
    path: String,
    remote: Option<String>,
    branch_name: String,
    set_upstream: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let remote_name = remote.as_deref().unwrap_or("origin");
    let output = writer::push_branch(&repo_path, remote_name, &branch_name, set_upstream).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Fetch from all remotes.
#[tauri::command]
pub async fn fetch_all(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::fetch_all(&repo_path).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            format!("Fetched successfully\n{}", output.stdout)
        } else {
            output.stderr
        },
    })
}
