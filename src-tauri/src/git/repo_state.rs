//! Read-only summary of the repository HEAD / operation state for the
//! status bar (branch, detached HEAD, upstream ahead/behind, merge/rebase...).
use git2::{BranchType, Repository, RepositoryState};
use serde::Serialize;

use crate::error::TwigError;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RepoStatusSummary {
    /// Current local branch name, `None` when HEAD is detached or unborn.
    pub branch: Option<String>,
    /// True when HEAD points directly at a commit.
    pub detached: bool,
    /// True for a repo whose current branch has no commits yet.
    pub unborn: bool,
    /// Abbreviated HEAD commit id (empty for an unborn branch).
    pub head_short_oid: String,
    /// Upstream shorthand like `origin/main`, if configured.
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    /// In-progress operation: "clean", "merge", "revert", "cherry-pick",
    /// "bisect", "rebase", "rebase-interactive", "rebase-merge", "apply-mailbox".
    pub state: String,
}

/// Stable string for a libgit2 repository state.
pub fn state_name(state: RepositoryState) -> &'static str {
    match state {
        RepositoryState::Clean => "clean",
        RepositoryState::Merge => "merge",
        RepositoryState::Revert | RepositoryState::RevertSequence => "revert",
        RepositoryState::CherryPick | RepositoryState::CherryPickSequence => "cherry-pick",
        RepositoryState::Bisect => "bisect",
        RepositoryState::Rebase => "rebase",
        RepositoryState::RebaseInteractive => "rebase-interactive",
        RepositoryState::RebaseMerge => "rebase-merge",
        RepositoryState::ApplyMailbox | RepositoryState::ApplyMailboxOrRebase => "apply-mailbox",
    }
}

pub fn repo_status_summary(repo: &Repository) -> Result<RepoStatusSummary, TwigError> {
    let state = state_name(repo.state()).to_string();
    let detached = repo.head_detached().unwrap_or(false);

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch || e.code() == git2::ErrorCode::NotFound => {
            // Unborn branch: HEAD is a symbolic ref to a branch with no commits.
            let branch = repo
                .find_reference("HEAD")
                .ok()
                .and_then(|r| r.symbolic_target().map(String::from))
                .map(|t| t.trim_start_matches("refs/heads/").to_string());
            return Ok(RepoStatusSummary {
                branch,
                detached: false,
                unborn: true,
                head_short_oid: String::new(),
                upstream: None,
                ahead: 0,
                behind: 0,
                state,
            });
        }
        Err(e) => return Err(e.into()),
    };

    let head_oid = head.target();
    let head_short_oid = head_oid
        .map(|oid| {
            let s = oid.to_string();
            s.chars().take(7).collect()
        })
        .unwrap_or_default();

    let mut summary = RepoStatusSummary {
        branch: None,
        detached,
        unborn: false,
        head_short_oid,
        upstream: None,
        ahead: 0,
        behind: 0,
        state,
    };

    if detached || !head.is_branch() {
        return Ok(summary);
    }

    let Some(name) = head.shorthand().map(String::from) else {
        return Ok(summary);
    };
    if let Ok(branch) = repo.find_branch(&name, BranchType::Local) {
        if let Ok(up) = branch.upstream() {
            summary.upstream = up.name().ok().flatten().map(String::from);
            if let (Some(local), Some(remote)) = (head_oid, up.get().target()) {
                if let Ok((a, b)) = repo.graph_ahead_behind(local, remote) {
                    summary.ahead = a;
                    summary.behind = b;
                }
            }
        }
    }
    summary.branch = Some(name);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo() -> Option<(std::path::PathBuf, Repository)> {
        let dir = std::env::temp_dir().join(format!(
            "twig-repo-state-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).ok()?;
        let repo = Repository::init(&dir).ok()?;
        Some((dir, repo))
    }

    #[test]
    fn unborn_repo_reports_clean_and_unborn() {
        let Some((dir, repo)) = init_repo() else { return };
        let s = repo_status_summary(&repo);
        let _ = std::fs::remove_dir_all(&dir);
        let s = match s {
            Ok(s) => s,
            Err(e) => panic!("summary failed: {e}"),
        };
        assert!(s.unborn);
        assert!(!s.detached);
        assert_eq!(s.state, "clean");
        assert!(s.branch.is_some());
    }

    #[test]
    fn state_names_are_stable() {
        assert_eq!(state_name(RepositoryState::Merge), "merge");
        assert_eq!(state_name(RepositoryState::RebaseMerge), "rebase-merge");
        assert_eq!(state_name(RepositoryState::CherryPickSequence), "cherry-pick");
    }
}
