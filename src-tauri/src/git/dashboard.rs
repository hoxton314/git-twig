//! Status and bulk actions for the repository dashboard (groups / all
//! repositories), including repositories that aren't open as tabs. Paths
//! come from the user's groups and history, so each one is validated as a
//! git working tree before anything runs in it.

use std::path::{Path, PathBuf};

use git2::{BranchType, Repository, StatusOptions};
use serde::Serialize;

use crate::error::TwigError;
use crate::git::writer::{run_git, GitOutput};

/// Most repositories one status request covers.
pub const MAX_PATHS: usize = 300;
/// Changed files are counted up to this many (then reported as "N+").
const MAX_CHANGES: usize = 1000;

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct RepoStatusRow {
    pub path: String,
    pub name: String,
    /// Current branch, or a short id when detached; `None` when unborn.
    pub branch: Option<String>,
    pub detached: bool,
    /// Upstream of the current branch (e.g. `origin/main`).
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// Uncommitted changes (tracked and untracked), capped at 1000.
    pub changes: usize,
    pub changes_capped: bool,
    /// When `FETCH_HEAD` was last written (unix seconds), if ever.
    pub last_fetch: Option<i64>,
    /// Why the repository couldn't be read (missing folder, not a repo, …).
    pub error: Option<String>,
}

/// `path` must be an absolute path to the working tree of a non-bare repo.
pub fn validate_workdir(path: &str) -> Result<PathBuf, TwigError> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(TwigError::InvalidArgument(format!("'{path}' is not an absolute path")));
    }
    if !p.is_dir() {
        return Err(TwigError::InvalidArgument("folder not found".into()));
    }
    let repo = Repository::open(p).map_err(|_| TwigError::NotARepo(path.to_string()))?;
    let workdir = repo.workdir().ok_or_else(|| TwigError::NotARepo(format!("{path} (bare repository)")))?;
    let same = match (std::fs::canonicalize(workdir), std::fs::canonicalize(p)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        return Err(TwigError::NotARepo(format!("{path} is inside a repository, not its top folder")));
    }
    Ok(p.to_path_buf())
}

fn name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn read_status(path: &str) -> Result<RepoStatusRow, TwigError> {
    let dir = validate_workdir(path)?;
    let repo = Repository::open(&dir)?;
    let mut row = RepoStatusRow { path: path.to_string(), name: name_of(path), ..Default::default() };

    if let Ok(head) = repo.head() {
        if head.is_branch() {
            row.branch = head.shorthand().map(String::from);
            if let (Some(name), Some(local)) = (head.shorthand(), head.target()) {
                if let Ok(branch) = repo.find_branch(name, BranchType::Local) {
                    if let Ok(up) = branch.upstream() {
                        row.upstream = up.name().ok().flatten().map(String::from);
                        if let Some(remote) = up.get().target() {
                            if let Ok((a, b)) = repo.graph_ahead_behind(local, remote) {
                                row.ahead = a;
                                row.behind = b;
                            }
                        }
                    }
                }
            }
        } else {
            row.detached = true;
            row.branch = head.target().map(|o| o.to_string().chars().take(7).collect());
        }
    }

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(false).include_ignored(false);
    let statuses = repo.statuses(Some(&mut opts))?;
    row.changes = statuses.len().min(MAX_CHANGES);
    row.changes_capped = statuses.len() > MAX_CHANGES;

    row.last_fetch = std::fs::metadata(repo.path().join("FETCH_HEAD"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    Ok(row)
}

/// Status of each path; a repository that can't be read gets an `error`
/// instead of failing the whole request.
pub fn status_rows(paths: &[String]) -> Vec<RepoStatusRow> {
    paths
        .iter()
        .take(MAX_PATHS)
        .map(|p| {
            read_status(p).unwrap_or_else(|e| RepoStatusRow {
                path: p.clone(),
                name: name_of(p),
                error: Some(e.to_string()),
                ..Default::default()
            })
        })
        .collect()
}

/// `git fetch --all --prune` (run inside network auth by the command).
pub async fn fetch(dir: &Path) -> Result<GitOutput, TwigError> {
    run_git(dir, &["fetch", "--all", "--prune"]).await
}

/// Fast-forward the current branch to its upstream (`git pull --ff-only`);
/// never creates a merge commit.
pub async fn pull_ff(dir: &Path) -> Result<GitOutput, TwigError> {
    run_git(dir, &["pull", "--ff-only", "--no-rebase"]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    async fn commit(dir: &Path, file: &str, msg: &str) {
        std::fs::write(dir.join(file), msg).unwrap();
        git(dir, &["add", "."]).await;
        git(dir, &["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "commit", "-q", "-m", msg]).await;
    }

    #[tokio::test]
    async fn reports_status_and_fast_forwards() {
        let root = std::env::temp_dir().join(format!("twig-dashboard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (origin, clone) = (root.join("origin"), root.join("clone"));
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "-q", "-b", "main"]).await;
        commit(&origin, "a.txt", "one").await;
        git(&root, &["clone", "-q", &origin.to_string_lossy(), "clone"]).await;
        commit(&origin, "b.txt", "two").await;
        let s = |p: &Path| p.to_string_lossy().into_owned();

        // Before fetching: nothing behind yet; a local change and a commit ahead.
        std::fs::write(clone.join("dirty.txt"), "x").unwrap();
        let rows = status_rows(&[s(&clone)]);
        let r = &rows[0];
        assert_eq!((r.branch.as_deref(), r.upstream.as_deref()), (Some("main"), Some("origin/main")));
        assert_eq!((r.ahead, r.behind, r.changes, r.error.as_deref()), (0, 0, 1, None));
        assert!(r.last_fetch.is_none());

        fetch(&clone).await.unwrap();
        let r = &status_rows(&[s(&clone)])[0];
        assert_eq!(r.behind, 1);
        assert!(r.last_fetch.is_some());

        let out = pull_ff(&clone).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(status_rows(&[s(&clone)])[0].behind, 0);

        // Diverged: --ff-only refuses instead of merging.
        commit(&clone, "c.txt", "local").await;
        commit(&origin, "d.txt", "remote").await;
        fetch(&clone).await.unwrap();
        let out = pull_ff(&clone).await.unwrap();
        assert!(!out.success);
        assert_eq!(git(&clone, &["rev-list", "--count", "--merges", "HEAD"]).await, "0");

        // Problems are per-row errors, not a failed request.
        let rows = status_rows(&[s(&root.join("nope")), s(&clone.join("sub")), "relative/path".into()]);
        assert!(rows.iter().all(|r| r.error.is_some()), "{rows:?}");
        std::fs::create_dir_all(clone.join("sub")).unwrap();
        assert!(validate_workdir(&s(&clone.join("sub"))).is_err(), "inside a repo, not its top folder");
        assert!(validate_workdir(&s(&clone)).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }
}
