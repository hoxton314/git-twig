//! Worktree commands.

use tauri::State;

use crate::error::TwigError;
use crate::git::worktrees::{self, WorktreeInfo};
use crate::state::AppState;

use super::staging::CommandResult;

#[tauri::command]
pub async fn list_worktrees(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<WorktreeInfo>, TwigError> {
    state.read_repo(&path, worktrees::list_worktrees).await
}

#[tauri::command]
pub async fn worktree_add(
    state: State<'_, AppState>,
    path: String,
    worktree_path: String,
    commitish: Option<String>,
    new_branch: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(worktrees::worktree_add(
        &repo_path,
        &worktree_path,
        commitish.as_deref(),
        new_branch.as_deref(),
    )
    .await?
    .into())
}

#[tauri::command]
pub async fn worktree_remove(
    state: State<'_, AppState>,
    path: String,
    worktree_path: String,
    force: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(worktrees::worktree_remove(&repo_path, &worktree_path, force).await?.into())
}

#[tauri::command]
pub async fn worktree_prune(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(worktrees::worktree_prune(&repo_path).await?.into())
}
