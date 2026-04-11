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

// ── Branch operations ─────────────────────────────────────────────────

pub async fn checkout_branch(repo_path: &Path, branch_name: &str) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["checkout", branch_name]).await
}

pub async fn create_branch(
    repo_path: &Path,
    branch_name: &str,
    start_point: Option<&str>,
) -> Result<GitOutput, TwigError> {
    match start_point {
        Some(sp) => run_git(repo_path, &["checkout", "-b", branch_name, sp]).await,
        None => run_git(repo_path, &["checkout", "-b", branch_name]).await,
    }
}

pub async fn rename_branch(
    repo_path: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["branch", "-m", old_name, new_name]).await
}

pub async fn delete_branch(
    repo_path: &Path,
    branch_name: &str,
    force: bool,
) -> Result<GitOutput, TwigError> {
    let flag = if force { "-D" } else { "-d" };
    run_git(repo_path, &["branch", flag, branch_name]).await
}

pub async fn push_branch(
    repo_path: &Path,
    remote: &str,
    branch_name: &str,
    set_upstream: bool,
) -> Result<GitOutput, TwigError> {
    if set_upstream {
        run_git(repo_path, &["push", "-u", remote, branch_name]).await
    } else {
        run_git(repo_path, &["push", remote, branch_name]).await
    }
}

pub async fn pull(repo_path: &Path, remote: &str, branch: &str) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["pull", remote, branch]).await
}

pub async fn fetch_all(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["fetch", "--all", "--prune"]).await
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
        Some(msg) => run_git(repo_path, &["stash", "push", "-m", msg]).await,
        None => run_git(repo_path, &["stash", "push"]).await,
    }
}

pub async fn stash_pop(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["stash", "pop"]).await
}
