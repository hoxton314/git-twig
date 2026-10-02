use tauri::State;

use crate::error::TwigError;
use crate::git::{reader, writer};
use crate::state::AppState;

pub use super::staging::CommandResult;

/// List all branches (local + remote).
#[tauri::command]
pub async fn get_branches(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<reader::BranchInfo>, TwigError> {
    state.read_repo(&path, reader::read_branches).await
}

/// Checkout a branch by name.
#[tauri::command]
pub async fn checkout_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output = writer::checkout_branch(&repo_path, &branch_name).await?;
    Ok(output.into())
}

/// Checkout a remote branch (e.g. "origin/feature") as a local tracking branch.
#[tauri::command]
pub async fn checkout_remote_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output = writer::checkout_remote_branch(&repo_path, &branch_name).await?;
    Ok(output.into())
}

/// Create a new branch, optionally from a start point.
#[tauri::command]
pub async fn create_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    start_point: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output =
        writer::create_branch(&repo_path, &branch_name, start_point.as_deref()).await?;
    Ok(output.into())
}

/// Rename a branch.
#[tauri::command]
pub async fn rename_branch(
    state: State<'_, AppState>,
    path: String,
    old_name: String,
    new_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output = writer::rename_branch(&repo_path, &old_name, &new_name).await?;
    Ok(output.into())
}

/// Delete a local branch.
#[tauri::command]
pub async fn delete_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    force: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output = writer::delete_branch(&repo_path, &branch_name, force).await?;
    Ok(output.into())
}

/// Delete a branch on a remote. `branch_name` excludes the remote prefix.
#[tauri::command]
pub async fn delete_remote_branch(
    state: State<'_, AppState>,
    path: String,
    remote: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output = writer::delete_remote_branch(&repo_path, &remote, &branch_name).await?;
    Ok(output.into())
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
    // Callers pass the HEAD display name, which is a short SHA when detached;
    // pushing that would create a junk branch on the remote.
    let local_ref = format!("refs/heads/{branch_name}");
    let is_local_branch = state
        .read_repo(&path, move |repo| Ok(repo.find_reference(&local_ref).is_ok()))
        .await?;
    if !is_local_branch {
        return Ok(CommandResult {
            success: false,
            message: format!(
                "'{branch_name}' is not a local branch (HEAD may be detached). \
                 Create or check out a branch before pushing."
            ),
        });
    }

    let repo_path = state.repo_path(&path)?;

    let remote_name = remote.as_deref().unwrap_or("origin");
    let output = writer::push_branch(&repo_path, remote_name, &branch_name, set_upstream).await?;
    Ok(output.into())
}

/// Merge a branch into the current HEAD.
#[tauri::command]
pub async fn merge_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output = writer::merge_branch(&repo_path, &branch_name).await?;
    Ok(output.into())
}

/// Fetch from all remotes.
#[tauri::command]
pub async fn fetch_all(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

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
