//! Commit helpers for the staging panel (amend, sign-off, co-authors,
//! commit template).
use tauri::State;

use crate::commands::staging::CommandResult;
use crate::error::TwigError;
use crate::git::commit_tools::{self, AuthorInfo, CommitOptions, HeadCommitInfo};
use crate::state::AppState;

/// Create a commit, optionally amending HEAD and/or adding `Signed-off-by`.
#[tauri::command]
pub async fn create_commit_with_options(
    state: State<'_, AppState>,
    path: String,
    message: String,
    amend: bool,
    signoff: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let output = commit_tools::commit_with_options(
        &repo_path,
        &message,
        CommitOptions { amend, signoff },
    )
    .await?;
    Ok(crate::commands::signing::commit_result(output))
}

/// HEAD's message and whether it has already been pushed.
#[tauri::command]
pub async fn get_head_commit_info(
    state: State<'_, AppState>,
    path: String,
) -> Result<HeadCommitInfo, TwigError> {
    state.read_repo(&path, commit_tools::read_head_commit).await
}

/// Distinct recent commit authors (excluding the current user), most
/// frequent first.
#[tauri::command]
pub async fn get_recent_authors(
    state: State<'_, AppState>,
    path: String,
    max_commits: Option<u32>,
) -> Result<Vec<AuthorInfo>, TwigError> {
    let limit = max_commits.unwrap_or(2000).clamp(1, 20_000) as usize;
    state
        .read_repo(&path, move |repo| commit_tools::read_recent_authors(repo, limit))
        .await
}

/// The `commit.template` contents (comment lines removed), if configured.
#[tauri::command]
pub async fn get_commit_template(
    state: State<'_, AppState>,
    path: String,
) -> Result<Option<String>, TwigError> {
    state.read_repo(&path, commit_tools::read_commit_template).await
}
