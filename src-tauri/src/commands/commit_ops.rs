//! Commands for the commit context menu and the undo (reflog) history.
use tauri::State;

use crate::error::TwigError;
use crate::git::commit_ops::{self, CommitOpResult};
use crate::git::reflog::{self, ReflogEntry};
use crate::state::AppState;

/// Check out a commit with a detached HEAD.
#[tauri::command]
pub async fn checkout_commit(
    state: State<'_, AppState>,
    path: String,
    oid: String,
) -> Result<CommitOpResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    commit_ops::checkout_detached(&repo_path, &oid).await
}

/// Cherry-pick a commit onto HEAD. Conflicts leave the repo mid-cherry-pick.
#[tauri::command]
pub async fn cherry_pick_commit(
    state: State<'_, AppState>,
    path: String,
    oid: String,
) -> Result<CommitOpResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    commit_ops::cherry_pick(&repo_path, &oid).await
}

/// Revert a commit. Conflicts leave the repo mid-revert.
#[tauri::command]
pub async fn revert_commit(
    state: State<'_, AppState>,
    path: String,
    oid: String,
) -> Result<CommitOpResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    commit_ops::revert(&repo_path, &oid).await
}

/// Reset HEAD to a commit. `mode`: "soft" | "mixed" | "hard" | "keep".
#[tauri::command]
pub async fn reset_to_commit(
    state: State<'_, AppState>,
    path: String,
    oid: String,
    mode: String,
) -> Result<CommitOpResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    commit_ops::reset(&repo_path, &oid, &mode).await
}

/// Recent HEAD movements, newest first.
#[tauri::command]
pub async fn get_head_reflog(
    state: State<'_, AppState>,
    path: String,
    limit: Option<usize>,
) -> Result<Vec<ReflogEntry>, TwigError> {
    let limit = limit.unwrap_or(200).clamp(1, 5000);
    state
        .read_repo(&path, move |repo| reflog::read_head_reflog(repo, limit))
        .await
}

/// Restore HEAD to a previous commit (reflog entry or undo target).
#[tauri::command]
pub async fn restore_head(
    state: State<'_, AppState>,
    path: String,
    oid: String,
    branch: Option<String>,
    auto_stash: bool,
) -> Result<CommitOpResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    commit_ops::restore_head(&repo_path, &oid, branch.as_deref(), auto_stash).await
}
