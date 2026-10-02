//! Branch-list commands: upstream tracking, fast-forward pull of any branch,
//! push of a specific branch, rebase, compare, create-at, tracking checkout.
//!
//! NOTE for merging: `rebase_branch` / `rebase_branch_abort` are a minimal
//! rebase backend for the branch list. If a dedicated rebase module exists,
//! these can be replaced by it.
use tauri::State;

use crate::error::TwigError;
use crate::git::branch_ops::{self, BranchComparison};
use crate::git::writer::GitOutput;
use crate::state::AppState;

use super::staging::CommandResult;

fn result(out: GitOutput, ok_message: String) -> CommandResult {
    if out.success {
        CommandResult { success: true, message: ok_message }
    } else {
        let err = out.stderr.trim();
        CommandResult {
            success: false,
            message: if err.is_empty() { out.stdout.trim().to_string() } else { err.to_string() },
        }
    }
}

#[tauri::command]
pub async fn set_branch_upstream(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    upstream: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::set_upstream(&repo_path, &branch_name, &upstream).await?;
    Ok(result(out, format!("'{branch_name}' now tracks '{upstream}'")))
}

#[tauri::command]
pub async fn unset_branch_upstream(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::unset_upstream(&repo_path, &branch_name).await?;
    Ok(result(out, format!("Removed upstream of '{branch_name}'")))
}

/// Fast-forward a branch that is not checked out to its upstream.
#[tauri::command]
pub async fn fast_forward_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::fast_forward_branch(&repo_path, &branch_name).await?;
    Ok(result(out, format!("Updated '{branch_name}'")))
}

/// Push a local branch to its upstream, or to `remote` with `--set-upstream`.
#[tauri::command]
pub async fn push_local_branch(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    remote: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::push_local_branch(&repo_path, &branch_name, remote.as_deref()).await?;
    Ok(result(out, format!("Pushed '{branch_name}'")))
}

/// Rebase `branch_name` (current branch when `None`) onto `onto`.
#[tauri::command]
pub async fn rebase_branch(
    state: State<'_, AppState>,
    path: String,
    onto: String,
    branch_name: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::rebase_branch(&repo_path, &onto, branch_name.as_deref()).await?;
    let what = branch_name.as_deref().unwrap_or("current branch");
    Ok(result(out, format!("Rebased {what} onto '{onto}'")))
}

#[tauri::command]
pub async fn rebase_branch_abort(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::rebase_abort(&repo_path).await?;
    Ok(result(out, "Rebase aborted".to_string()))
}

/// Create a branch at a start point without checking it out.
#[tauri::command]
pub async fn create_branch_at(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    start_point: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::create_branch_at(&repo_path, branch_name.trim(), &start_point).await?;
    Ok(result(out, format!("Created '{}'", branch_name.trim())))
}

/// Checkout `<remote>/<branch>` as a local tracking branch.
#[tauri::command]
pub async fn checkout_remote_tracking(
    state: State<'_, AppState>,
    path: String,
    remote: String,
    branch_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = branch_ops::checkout_remote_tracking(&repo_path, &remote, &branch_name).await?;
    Ok(result(out, format!("Checked out '{branch_name}'")))
}

/// Commits `other` has that `base` lacks (ahead) and vice versa (behind).
#[tauri::command]
pub async fn compare_branches(
    state: State<'_, AppState>,
    path: String,
    base: String,
    other: String,
) -> Result<BranchComparison, TwigError> {
    state
        .read_repo(&path, move |repo| branch_ops::compare_refs(repo, &base, &other, 200))
        .await
}
