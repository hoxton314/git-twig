//! Branch-list operations: upstream tracking, fast-forwarding a branch that is
//! not checked out, pushing a specific branch, rebasing, comparing two refs.
//! Writes go through the git CLI; the comparison is a git2 read.
use std::path::Path;

use git2::Repository;
use serde::Serialize;

use super::writer::{run_git, safe_ref, GitOutput};
use crate::error::TwigError;

/// Read a single git config value (`None` if unset).
async fn config_get(repo_path: &Path, key: &str) -> Result<Option<String>, TwigError> {
    let out = run_git(repo_path, &["config", "--get", key]).await?;
    let value = out.stdout.trim().to_string();
    Ok(if out.success && !value.is_empty() { Some(value) } else { None })
}

/// `(remote, merge_ref)` of a local branch's upstream, if configured.
async fn upstream_of(repo_path: &Path, branch: &str) -> Result<Option<(String, String)>, TwigError> {
    let remote = config_get(repo_path, &format!("branch.{branch}.remote")).await?;
    let merge = config_get(repo_path, &format!("branch.{branch}.merge")).await?;
    Ok(remote.zip(merge))
}

/// Short name of the checked-out branch, or `None` when detached/unborn.
async fn current_branch(repo_path: &Path) -> Result<Option<String>, TwigError> {
    let out = run_git(repo_path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await?;
    let name = out.stdout.trim().to_string();
    Ok(if out.success && !name.is_empty() { Some(name) } else { None })
}

fn failed(message: String) -> GitOutput {
    GitOutput { success: false, stdout: String::new(), stderr: message }
}

pub async fn set_upstream(
    repo_path: &Path,
    branch: &str,
    upstream: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(branch)?;
    safe_ref(upstream)?;
    run_git(repo_path, &["branch", "--set-upstream-to", upstream, branch]).await
}

pub async fn unset_upstream(repo_path: &Path, branch: &str) -> Result<GitOutput, TwigError> {
    safe_ref(branch)?;
    run_git(repo_path, &["branch", "--unset-upstream", branch]).await
}

/// Fast-forward a local branch that is *not* checked out to its upstream
/// (`git fetch <remote> <merge>:refs/heads/<branch>`). Without a leading `+`
/// the refspec only allows fast-forwards, and git refuses to update a branch
/// checked out in any worktree, so this can never lose commits.
pub async fn fast_forward_branch(repo_path: &Path, branch: &str) -> Result<GitOutput, TwigError> {
    safe_ref(branch)?;
    if current_branch(repo_path).await?.as_deref() == Some(branch) {
        return Ok(failed(format!(
            "'{branch}' is checked out; use Pull to update the current branch."
        )));
    }
    let Some((remote, merge)) = upstream_of(repo_path, branch).await? else {
        return Ok(failed(format!(
            "'{branch}' has no upstream branch. Set an upstream first."
        )));
    };
    safe_ref(&remote)?;
    safe_ref(&merge)?;
    if merge.contains(':') || remote.contains(':') {
        return Ok(failed(format!("unsupported upstream configuration for '{branch}'")));
    }
    let refspec = format!("{merge}:refs/heads/{branch}");
    run_git(repo_path, &["fetch", "--no-write-fetch-head", "--", &remote, &refspec]).await
}

/// Push a local branch. With an upstream, push to it (even if its name
/// differs); without one, push to `remote` (or `origin`) and set upstream.
pub async fn push_local_branch(
    repo_path: &Path,
    branch: &str,
    remote: Option<&str>,
) -> Result<GitOutput, TwigError> {
    safe_ref(branch)?;
    let local = format!("refs/heads/{branch}");
    if !super::writer::rev_exists(repo_path, &local).await? {
        return Ok(failed(format!("'{branch}' is not a local branch")));
    }

    if remote.is_none() {
        if let Some((up_remote, merge)) = upstream_of(repo_path, branch).await? {
            // `.` means the upstream is another local branch; nothing to push.
            if up_remote != "." {
                safe_ref(&up_remote)?;
                safe_ref(&merge)?;
                let refspec = format!("{local}:{merge}");
                return run_git(repo_path, &["push", "--", &up_remote, &refspec]).await;
            }
        }
    }

    let remote = remote.unwrap_or("origin");
    safe_ref(remote)?;
    let refspec = format!("{local}:{local}");
    run_git(repo_path, &["push", "--set-upstream", "--", remote, &refspec]).await
}

/// Rebase `branch` (or the current branch when `None`) onto `onto`.
/// On conflicts git stops mid-rebase; the conflict banner offers continue/abort.
pub async fn rebase_branch(
    repo_path: &Path,
    onto: &str,
    branch: Option<&str>,
) -> Result<GitOutput, TwigError> {
    safe_ref(onto)?;
    match branch {
        Some(b) => {
            safe_ref(b)?;
            run_git(repo_path, &["rebase", onto, b]).await
        }
        None => run_git(repo_path, &["rebase", onto]).await,
    }
}

/// Create a branch at `start_point` without switching to it (used for
/// "create branch from here" and to undo a branch deletion).
pub async fn create_branch_at(
    repo_path: &Path,
    name: &str,
    start_point: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    safe_ref(start_point)?;
    run_git(repo_path, &["branch", name, start_point]).await
}

/// Checkout `<remote>/<branch>` as local branch `<branch>` that tracks it.
/// Takes the remote and branch separately so remote names with `/` work.
/// An existing local branch with that name is checked out instead.
pub async fn checkout_remote_tracking(
    repo_path: &Path,
    remote: &str,
    branch: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    safe_ref(branch)?;
    if branch == "HEAD" {
        return Ok(failed("cannot checkout the symbolic HEAD of a remote".to_string()));
    }
    let local_ref = format!("refs/heads/{branch}");
    if super::writer::rev_exists(repo_path, &local_ref).await? {
        return run_git(repo_path, &["checkout", branch, "--"]).await;
    }
    let remote_ref = format!("refs/remotes/{remote}/{branch}");
    run_git(repo_path, &["checkout", "-b", branch, "--track", &remote_ref, "--"]).await
}

// ── Compare ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ComparedCommit {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub author_name: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchComparison {
    pub base: String,
    pub other: String,
    /// Commits reachable from `other` but not from `base` (newest first).
    pub ahead: Vec<ComparedCommit>,
    /// Commits reachable from `base` but not from `other` (newest first).
    pub behind: Vec<ComparedCommit>,
    pub ahead_count: usize,
    pub behind_count: usize,
}

fn commits_between(
    repo: &Repository,
    include: git2::Oid,
    exclude: git2::Oid,
    limit: usize,
) -> Result<(Vec<ComparedCommit>, usize), TwigError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    walk.push(include)?;
    walk.hide(exclude)?;
    let mut list = Vec::new();
    let mut count = 0;
    for oid in walk {
        let oid = oid?;
        count += 1;
        if list.len() < limit {
            let commit = repo.find_commit(oid)?;
            let full = oid.to_string();
            list.push(ComparedCommit {
                short_oid: full.chars().take(7).collect(),
                oid: full,
                summary: crate::git::graph::commit_summary(&commit),
                author_name: crate::git::graph::commit_author_name(&commit),
                timestamp: commit.time().seconds(),
            });
        }
    }
    Ok((list, count))
}

/// Compare two revisions (branch names, `HEAD`, SHAs).
pub fn compare_refs(
    repo: &Repository,
    base: &str,
    other: &str,
    limit: usize,
) -> Result<BranchComparison, TwigError> {
    let base_oid = repo.revparse_single(base)?.peel_to_commit()?.id();
    let other_oid = repo.revparse_single(other)?.peel_to_commit()?.id();
    let (ahead, ahead_count) = commits_between(repo, other_oid, base_oid, limit)?;
    let (behind, behind_count) = commits_between(repo, base_oid, other_oid, limit)?;
    Ok(BranchComparison {
        base: base.to_string(),
        other: other.to_string(),
        ahead,
        behind,
        ahead_count,
        behind_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git_ok(dir: &Path, args: &[&str]) -> String {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    async fn commit_file(dir: &Path, file: &str, msg: &str) {
        std::fs::write(dir.join(file), msg).unwrap();
        git_ok(dir, &["add", "--", file]).await;
        git_ok(dir, &["commit", "-q", "-m", msg]).await;
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-branchops-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        // Rebase creates commits through plain `run_git`, so identity must
        // come from the repo config rather than `-c` flags.
        git_ok(&dir, &["config", "user.name", "t"]).await;
        git_ok(&dir, &["config", "user.email", "t@t"]).await;
        git_ok(&dir, &["config", "commit.gpgsign", "false"]).await;
        commit_file(&dir, "a.txt", "c1").await;
        dir
    }

    #[tokio::test]
    async fn upstream_fast_forward_and_push() {
        let up = temp_repo("ff-up").await;
        git_ok(&up, &["branch", "feature/x"]).await;
        let dir = std::env::temp_dir().join(format!("twig-branchops-ff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        git_ok(
            std::env::temp_dir().as_path(),
            &["clone", "-q", "-o", "team/up", up.to_str().unwrap(), dir.to_str().unwrap()],
        )
        .await;

        // Remote with a slash in its name: checkout a tracking branch.
        let out = checkout_remote_tracking(&dir, "team/up", "feature/x").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(
            config_get(&dir, "branch.feature/x.remote").await.unwrap().as_deref(),
            Some("team/up")
        );
        git_ok(&dir, &["checkout", "-q", "main"]).await;

        // Upstream advances; fast-forward the non-checked-out branch.
        git_ok(&up, &["checkout", "-q", "feature/x"]).await;
        commit_file(&up, "b.txt", "c2").await;
        git_ok(&up, &["checkout", "-q", "main"]).await;
        let out = fast_forward_branch(&dir, "feature/x").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let local = git_ok(&dir, &["rev-parse", "feature/x"]).await;
        let remote = git_ok(&up, &["rev-parse", "feature/x"]).await;
        assert_eq!(local, remote);

        // Refuses the checked-out branch.
        let out = fast_forward_branch(&dir, "main").await.unwrap();
        assert!(!out.success);

        // Unset / set upstream.
        let out = unset_upstream(&dir, "feature/x").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(upstream_of(&dir, "feature/x").await.unwrap().is_none());
        let out = fast_forward_branch(&dir, "feature/x").await.unwrap();
        assert!(!out.success);
        let out = set_upstream(&dir, "feature/x", "team/up/feature/x").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(upstream_of(&dir, "feature/x").await.unwrap().is_some());

        // Push a new branch without upstream sets it.
        git_ok(&dir, &["branch", "topic"]).await;
        let out = push_local_branch(&dir, "topic", Some("team/up")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(upstream_of(&dir, "topic").await.unwrap().is_some());
        assert!(super::super::writer::rev_exists(&up, "refs/heads/topic").await.unwrap());

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&up);
    }

    #[tokio::test]
    async fn rebase_compare_and_restore() {
        let dir = temp_repo("rebase").await;
        git_ok(&dir, &["checkout", "-q", "-b", "feature"]).await;
        commit_file(&dir, "f.txt", "feature work").await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        commit_file(&dir, "m.txt", "main work").await;

        {
            let repo = Repository::open(&dir).unwrap();
            let cmp = compare_refs(&repo, "main", "feature", 100).unwrap();
            assert_eq!(cmp.ahead_count, 1);
            assert_eq!(cmp.behind_count, 1);
            assert_eq!(cmp.ahead[0].summary, "feature work");
        }

        let out = rebase_branch(&dir, "main", Some("feature")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        {
            let repo = Repository::open(&dir).unwrap();
            let cmp = compare_refs(&repo, "main", "feature", 100).unwrap();
            assert_eq!(cmp.behind_count, 0);
        }

        // Delete and restore at the old SHA.
        let sha = git_ok(&dir, &["rev-parse", "feature"]).await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        git_ok(&dir, &["branch", "-D", "feature"]).await;
        let out = create_branch_at(&dir, "feature", &sha).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(git_ok(&dir, &["rev-parse", "feature"]).await, sha);
        assert!(create_branch_at(&dir, "-x", &sha).await.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
