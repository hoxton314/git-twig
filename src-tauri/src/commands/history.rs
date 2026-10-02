//! Tauri commands for rebase, interactive rebase, and force push.
use tauri::State;

use crate::error::TwigError;
use crate::git::conflicts::read_operation_state;
use crate::git::history::{self, RebaseCommitList, RebaseTodoItem};
use crate::state::AppState;

use super::conflicts::merge_output;
use super::staging::CommandResult;

fn busy(kind: &str) -> CommandResult {
    CommandResult {
        success: false,
        message: format!(
            "A {} is already in progress. Continue or abort it first.",
            kind.replace('_', "-")
        ),
    }
}

/// Rebase the current branch onto `upstream` (branch name or commit).
#[tauri::command]
pub async fn rebase_onto(
    state: State<'_, AppState>,
    path: String,
    upstream: String,
    autostash: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let kind = state
        .read_repo(&path, |repo| Ok(read_operation_state(repo)?.kind))
        .await?;
    if kind != "none" {
        return Ok(busy(&kind));
    }
    let out = history::rebase_onto(&repo_path, &upstream, autostash).await?;
    Ok(merge_output(out))
}

/// Commits (oldest first) that an interactive rebase from `base` would edit.
/// `base = None` means from the root commit.
#[tauri::command]
pub async fn list_rebase_commits(
    state: State<'_, AppState>,
    path: String,
    base: Option<String>,
) -> Result<RebaseCommitList, TwigError> {
    state
        .read_repo(&path, move |repo| history::list_rebase_commits(repo, base.as_deref()))
        .await
}

/// Run an interactive rebase with the given todo list.
#[tauri::command]
pub async fn interactive_rebase(
    state: State<'_, AppState>,
    path: String,
    base: Option<String>,
    items: Vec<RebaseTodoItem>,
    autostash: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let b = base.clone();
    let (kind, list, gitdir) = state
        .read_repo(&path, move |repo| {
            Ok((
                read_operation_state(repo)?.kind,
                // Only the ids are compared; skip the patch-id scan.
                history::list_rebase_commits_opts(repo, b.as_deref(), false)?,
                repo.path().to_path_buf(),
            ))
        })
        .await?;
    if kind != "none" {
        return Ok(busy(&kind));
    }

    // Every commit in range must be accounted for exactly once; anything
    // missing would be silently dropped by git.
    let mut expected: Vec<&str> = list.commits.iter().map(|c| c.oid.as_str()).collect();
    let mut given: Vec<&str> = items.iter().map(|i| i.oid.as_str()).collect();
    expected.sort_unstable();
    given.sort_unstable();
    if expected != given {
        return Ok(CommandResult {
            success: false,
            message: "The commit list is out of date (HEAD moved). Reload and try again."
                .to_string(),
        });
    }

    let out =
        history::interactive_rebase(&repo_path, &gitdir, base.as_deref(), &items, autostash).await?;
    Ok(merge_output(out))
}

/// Force-push the branch with `--force-with-lease`. The remote defaults to
/// the branch's upstream remote, then `origin`.
#[tauri::command]
pub async fn force_push_with_lease(
    state: State<'_, AppState>,
    path: String,
    branch_name: String,
    remote: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let b = branch_name.clone();
    let (is_local, upstream) = state
        .read_repo(&path, move |repo| {
            let is_local = repo.find_reference(&format!("refs/heads/{b}")).is_ok();
            Ok((is_local, history::upstream_remote(repo, &b)))
        })
        .await?;
    if !is_local {
        return Ok(CommandResult {
            success: false,
            message: format!(
                "'{branch_name}' is not a local branch (HEAD may be detached)."
            ),
        });
    }
    let set_upstream = upstream.is_none();
    let remote = remote.or(upstream).unwrap_or_else(|| "origin".to_string());
    let out = history::force_push_with_lease(&repo_path, &remote, &branch_name, set_upstream).await?;
    Ok(merge_output(out))
}
