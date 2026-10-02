//! Worktrees: listing via git2, add/remove/prune via the git CLI.

use std::path::{Path, PathBuf};

use git2::Repository;
use serde::Serialize;

use crate::error::TwigError;
use crate::git::writer::{run_git, safe_ref, GitOutput};

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeInfo {
    /// Worktree name (`None` for the main worktree).
    pub name: Option<String>,
    pub path: String,
    pub is_main: bool,
    /// The worktree this repo handle was opened from.
    pub is_current: bool,
    /// Checked-out branch (short name), `None` when detached or unborn.
    pub branch: Option<String>,
    pub head_short: Option<String>,
    pub is_locked: bool,
    /// The worktree directory is missing (can be pruned).
    pub is_prunable: bool,
}

fn head_of(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(repo) = Repository::open(path) else {
        return (None, None);
    };
    let Ok(head) = repo.head() else {
        return (None, None);
    };
    let branch = if head.is_branch() {
        head.shorthand().map(String::from)
    } else {
        None
    };
    let short = head.target().map(|o| {
        let mut s = o.to_string();
        s.truncate(7);
        s
    });
    (branch, short)
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

pub fn list_worktrees(repo: &Repository) -> Result<Vec<WorktreeInfo>, TwigError> {
    // Always enumerate from the main repository so linked worktrees see the
    // full set too.
    let main = Repository::open(repo.commondir())?;
    let current = repo.workdir().map(canon);
    let mut out = Vec::new();

    if let Some(main_wd) = main.workdir() {
        let main_wd = canon(main_wd);
        let (branch, head_short) = head_of(&main_wd);
        out.push(WorktreeInfo {
            name: None,
            path: main_wd.to_string_lossy().to_string(),
            is_main: true,
            is_current: current.as_deref() == Some(main_wd.as_path()),
            branch,
            head_short,
            is_locked: false,
            is_prunable: false,
        });
    }

    for name in main.worktrees()?.iter().flatten() {
        let Ok(wt) = main.find_worktree(name) else {
            continue;
        };
        let path = canon(wt.path());
        let prunable = wt.validate().is_err();
        let (branch, head_short) = if prunable { (None, None) } else { head_of(&path) };
        out.push(WorktreeInfo {
            name: Some(name.to_string()),
            is_current: current.as_deref() == Some(path.as_path()),
            path: path.to_string_lossy().to_string(),
            is_main: false,
            branch,
            head_short,
            is_locked: !matches!(wt.is_locked(), Ok(git2::WorktreeLockStatus::Unlocked)),
            is_prunable: prunable,
        });
    }
    Ok(out)
}

fn require_abs(path: &str) -> Result<(), TwigError> {
    if path.is_empty() || !Path::new(path).is_absolute() {
        return Err(TwigError::InvalidArgument(format!(
            "worktree path must be absolute: '{path}'"
        )));
    }
    Ok(())
}

/// Add a worktree at `path`. With `new_branch`, create that branch starting
/// at `commitish` (default HEAD); otherwise check out `commitish` (a branch).
pub async fn worktree_add(
    repo_path: &Path,
    path: &str,
    commitish: Option<&str>,
    new_branch: Option<&str>,
) -> Result<GitOutput, TwigError> {
    require_abs(path)?;
    let mut args: Vec<&str> = vec!["worktree", "add"];
    if let Some(b) = new_branch {
        safe_ref(b)?;
        args.push("-b");
        args.push(b);
    }
    if let Some(c) = commitish {
        safe_ref(c)?;
    }
    args.push("--");
    args.push(path);
    if let Some(c) = commitish {
        args.push(c);
    }
    run_git(repo_path, &args).await
}

/// Remove a linked worktree (`--force` also discards its local changes).
pub async fn worktree_remove(repo_path: &Path, path: &str, force: bool) -> Result<GitOutput, TwigError> {
    require_abs(path)?;
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push("--");
    args.push(path);
    run_git(repo_path, &args).await
}

pub async fn worktree_prune(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["worktree", "prune"]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn git_ok(dir: &Path, args: &[&str]) {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    #[tokio::test]
    async fn add_list_remove_worktrees() {
        let base = std::env::temp_dir().join(format!("twig-wt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let main = base.join("main");
        std::fs::create_dir_all(&main).unwrap();
        git_ok(&main, &["init", "-q", "-b", "main"]).await;
        std::fs::write(main.join("f.txt"), "f\n").unwrap();
        git_ok(&main, &["add", "."]).await;
        git_ok(&main, &["commit", "-q", "-m", "init"]).await;
        git_ok(&main, &["branch", "existing"]).await;

        let wt1 = base.join("wt one");
        let wt1s = wt1.to_string_lossy().to_string();
        let out = worktree_add(&main, &wt1s, None, Some("feature")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let wt2 = base.join("wt2");
        let wt2s = wt2.to_string_lossy().to_string();
        let out = worktree_add(&main, &wt2s, Some("existing"), None).await.unwrap();
        assert!(out.success, "{}", out.stderr);

        let repo = Repository::open(&wt1).unwrap();
        let list = list_worktrees(&repo).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list[0].is_main);
        assert_eq!(list[0].branch.as_deref(), Some("main"));
        let w1 = list.iter().find(|w| w.branch.as_deref() == Some("feature")).unwrap();
        assert!(w1.is_current);
        assert!(list.iter().any(|w| w.branch.as_deref() == Some("existing")));

        assert!(worktree_add(&main, "relative/path", None, None).await.is_err());
        assert!(worktree_add(&main, &wt2s, Some("-x"), None).await.is_err());

        let out = worktree_remove(&main, &wt2s, false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(list_worktrees(&Repository::open(&main).unwrap()).unwrap().len(), 2);
        assert!(worktree_prune(&main).await.unwrap().success);
        let _ = std::fs::remove_dir_all(&base);
    }
}
