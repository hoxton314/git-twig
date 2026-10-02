//! Stash extras: SHA-addressed apply/pop/drop, show, rename, branch, and
//! partial / keep-index push.

use tauri::State;

use crate::error::TwigError;
use crate::git::stash_extra::{self, StashDetail, StashDiff};
use crate::state::AppState;

use super::staging::CommandResult;

#[tauri::command]
pub async fn stash_list_detailed(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<StashDetail>, TwigError> {
    let repo_path = state.repo_path(&path)?;
    stash_extra::stash_list_detailed(&repo_path).await
}

#[tauri::command]
pub async fn stash_show(
    state: State<'_, AppState>,
    path: String,
    oid: String,
) -> Result<StashDiff, TwigError> {
    state
        .read_repo(&path, move |repo| stash_extra::read_stash_diff(repo, &oid))
        .await
}

/// `action` is "apply", "pop" or "drop".
#[tauri::command]
pub async fn stash_act(
    state: State<'_, AppState>,
    path: String,
    oid: String,
    action: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(stash_extra::stash_act(&repo_path, &oid, &action).await?.into())
}

#[tauri::command]
pub async fn stash_rename(
    state: State<'_, AppState>,
    path: String,
    oid: String,
    message: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(stash_extra::stash_rename(&repo_path, &oid, &message).await?.into())
}

#[tauri::command]
pub async fn stash_branch(
    state: State<'_, AppState>,
    path: String,
    oid: String,
    branch: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(stash_extra::stash_branch(&repo_path, &oid, &branch).await?.into())
}

#[tauri::command]
pub async fn stash_push_ext(
    state: State<'_, AppState>,
    path: String,
    message: Option<String>,
    files: Vec<String>,
    keep_index: bool,
    include_untracked: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    Ok(stash_extra::stash_push_ext(
        &repo_path,
        message.as_deref(),
        &refs,
        keep_index,
        include_untracked,
    )
    .await?
    .into())
}
