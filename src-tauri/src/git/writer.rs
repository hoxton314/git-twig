//! CLI-based write operations — commit, push, pull, merge, rebase, stash, and all LFS ops.
//! Uses system `git` via std::process::Command for safety and LFS compatibility.
use std::path::Path;
use tokio::process::Command;

use crate::error::TwigError;

#[derive(Debug)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

async fn run_git(repo_path: &Path, args: &[&str]) -> Result<GitOutput, TwigError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git: {e}")))?;

    Ok(GitOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Reject arguments that git would otherwise interpret as command-line flags.
/// Branch names, refs and remote names never legitimately begin with `-`
/// (git itself forbids it), so a leading dash means a crafted/invalid value.
fn safe_ref(value: &str) -> Result<(), TwigError> {
    if value.is_empty() {
        return Err(TwigError::GitCli("empty git ref/name argument".to_string()));
    }
    if value.starts_with('-') {
        return Err(TwigError::GitCli(format!(
            "invalid name '{value}': must not start with '-'"
        )));
    }
    Ok(())
}

// ── Branch operations ─────────────────────────────────────────────────

pub async fn checkout_branch(repo_path: &Path, branch_name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    run_git(repo_path, &["checkout", branch_name]).await
}

/// Checkout a remote branch (e.g. "origin/feature") as a local tracking branch.
/// If a local branch with the derived name already exists, checkout that instead
/// of failing — it is almost always the branch the user wants.
pub async fn checkout_remote_branch(
    repo_path: &Path,
    remote_branch: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote_branch)?;
    let local_name = match remote_branch.split_once('/') {
        Some((_, local)) if !local.is_empty() => local,
        _ => {
            return Err(TwigError::GitCli(format!(
                "'{remote_branch}' is not a remote branch name"
            )))
        }
    };
    if local_name == "HEAD" {
        return Err(TwigError::GitCli(
            "cannot checkout the symbolic HEAD of a remote".to_string(),
        ));
    }

    let local_ref = format!("refs/heads/{local_name}");
    let exists = run_git(repo_path, &["rev-parse", "--verify", "--quiet", &local_ref]).await?;
    if exists.success {
        run_git(repo_path, &["checkout", local_name]).await
    } else {
        run_git(repo_path, &["checkout", "--track", remote_branch]).await
    }
}

pub async fn create_branch(
    repo_path: &Path,
    branch_name: &str,
    start_point: Option<&str>,
) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    match start_point {
        Some(sp) => {
            safe_ref(sp)?;
            run_git(repo_path, &["checkout", "-b", branch_name, sp]).await
        }
        None => run_git(repo_path, &["checkout", "-b", branch_name]).await,
    }
}

pub async fn rename_branch(
    repo_path: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(old_name)?;
    safe_ref(new_name)?;
    run_git(repo_path, &["branch", "-m", old_name, new_name]).await
}

pub async fn delete_branch(
    repo_path: &Path,
    branch_name: &str,
    force: bool,
) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    let flag = if force { "-D" } else { "-d" };
    run_git(repo_path, &["branch", flag, branch_name]).await
}

/// Delete a branch on a remote (`git push <remote> --delete <branch>`).
/// `branch_name` is the branch name without the remote prefix.
pub async fn delete_remote_branch(
    repo_path: &Path,
    remote: &str,
    branch_name: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    safe_ref(branch_name)?;
    run_git(repo_path, &["push", remote, "--delete", branch_name]).await
}

pub async fn push_branch(
    repo_path: &Path,
    remote: &str,
    branch_name: &str,
    set_upstream: bool,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    safe_ref(branch_name)?;
    if set_upstream {
        run_git(repo_path, &["push", "-u", remote, branch_name]).await
    } else {
        run_git(repo_path, &["push", remote, branch_name]).await
    }
}

pub async fn pull(repo_path: &Path, remote: &str, branch: Option<&str>) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    match branch {
        Some(b) => {
            safe_ref(b)?;
            run_git(repo_path, &["pull", remote, b]).await
        }
        None => run_git(repo_path, &["pull", remote]).await,
    }
}

pub async fn has_uncommitted_changes(repo_path: &Path) -> Result<bool, TwigError> {
    let output = run_git(repo_path, &["status", "--porcelain"]).await?;
    Ok(!output.stdout.trim().is_empty())
}

pub async fn fetch_all(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["fetch", "--all", "--prune"]).await
}

pub async fn merge_branch(repo_path: &Path, branch_name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    run_git(repo_path, &["merge", branch_name]).await
}

// ── Undo operations ──────────────────────────────────────────────────

pub async fn undo_last_commit(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["reset", "--soft", "HEAD~1"]).await
}

// ── Discard operations ────────────────────────────────────────────────

pub async fn restore_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    let mut args = vec!["restore", "--"];
    args.extend(paths);
    run_git(repo_path, &args).await
}

pub async fn clean_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    // `-d` is required to remove untracked directories; without it `git clean`
    // silently leaves directories behind while still reporting success.
    let mut args = vec!["clean", "-fd", "--"];
    args.extend(paths);
    run_git(repo_path, &args).await
}

// ── Commit operations ─────────────────────────────────────────────────

pub async fn stage_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    let mut args = vec!["add", "--"];
    args.extend(paths);
    run_git(repo_path, &args).await
}

pub async fn unstage_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    let mut args = vec!["restore", "--staged", "--"];
    args.extend(paths);
    run_git(repo_path, &args).await
}

pub async fn commit(repo_path: &Path, message: &str) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["commit", "-m", message]).await
}

// ── Stash ─────────────────────────────────────────────────────────────

pub async fn stash_push(
    repo_path: &Path,
    message: Option<&str>,
) -> Result<GitOutput, TwigError> {
    match message {
        Some(msg) => run_git(repo_path, &["stash", "push", "--include-untracked", "-m", msg]).await,
        None => run_git(repo_path, &["stash", "push", "--include-untracked"]).await,
    }
}

pub async fn stash_pop(repo_path: &Path, index: u32) -> Result<GitOutput, TwigError> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_path, &["stash", "pop", &stash_ref]).await
}

pub async fn stash_apply(repo_path: &Path, index: u32) -> Result<GitOutput, TwigError> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_path, &["stash", "apply", &stash_ref]).await
}

pub async fn stash_drop(repo_path: &Path, index: u32) -> Result<GitOutput, TwigError> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_path, &["stash", "drop", &stash_ref]).await
}

pub async fn stash_list(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(
        repo_path,
        &["stash", "list", "--format=%gd%x00%s%x00%aI"],
    )
    .await
}

/// Resolve the commit SHA of the stash entry at the top of the stack
/// (`stash@{0}`), or `None` if there is no stash.
pub async fn stash_top_sha(repo_path: &Path) -> Result<Option<String>, TwigError> {
    let out = run_git(repo_path, &["rev-parse", "--verify", "--quiet", "stash@{0}"]).await?;
    let sha = out.stdout.trim().to_string();
    Ok(if sha.is_empty() { None } else { Some(sha) })
}

/// Find the current stack index of the stash entry whose commit matches `sha`.
/// Stash indices shift as entries are added/removed, so a previously-captured
/// SHA must be re-resolved to an index before popping it.
pub async fn stash_index_for_sha(repo_path: &Path, sha: &str) -> Result<Option<u32>, TwigError> {
    let out = run_git(repo_path, &["stash", "list", "--format=%gd %H"]).await?;
    for line in out.stdout.lines() {
        if let Some((reference, line_sha)) = line.split_once(' ') {
            if line_sha.trim() == sha {
                return Ok(reference
                    .trim_start_matches("stash@{")
                    .trim_end_matches('}')
                    .parse::<u32>()
                    .ok());
            }
        }
    }
    Ok(None)
}
