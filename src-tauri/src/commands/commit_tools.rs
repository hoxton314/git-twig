//! Commit helpers for the staging panel (amend, sign-off, co-authors,
//! commit template).
use serde::Serialize;
use tauri::State;

use crate::commands::staging::CommandResult;
use crate::error::TwigError;
use crate::git::commit_tools::{self, AuthorInfo, CommitOptions, HeadCommitInfo};
use crate::state::AppState;

/// Result of a commit from the staging panel, with what its hooks printed.
#[derive(Debug, Serialize)]
pub struct CommitResult {
    #[serde(flatten)]
    pub result: CommandResult,
    /// Hooks that ran (pre-commit, commit-msg, …).
    pub hooks: Vec<String>,
    /// Their combined output (git sends hook stdout to stderr), when any.
    pub hook_output: Option<String>,
}

/// Create a commit, optionally amending HEAD, adding `Signed-off-by` and/or
/// skipping the verifying hooks.
#[tauri::command]
pub async fn create_commit_with_options(
    state: State<'_, AppState>,
    path: String,
    message: String,
    amend: bool,
    signoff: bool,
    no_verify: Option<bool>,
) -> Result<CommitResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let no_verify = no_verify.unwrap_or(false);
    let hooks = commit_tools::active_commit_hooks(&repo_path, no_verify).await.unwrap_or_default();
    let output = commit_tools::commit_with_options(
        &repo_path,
        &message,
        CommitOptions { amend, signoff, no_verify },
    )
    .await?;
    // Without hooks, stderr only holds git's own warnings; on failure it is
    // already the message.
    let hook_output = (!hooks.is_empty())
        .then(|| commit_tools::clean_hook_output(&output.stderr))
        .filter(|s| !s.is_empty());
    Ok(CommitResult { result: crate::commands::signing::commit_result(output), hooks, hook_output })
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
