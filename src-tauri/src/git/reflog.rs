//! HEAD reflog reading (git2) for the undo history panel.
use git2::{Oid, Repository};
use serde::Serialize;

use crate::error::TwigError;

#[derive(Debug, Serialize)]
pub struct ReflogEntry {
    /// Position in the reflog (`HEAD@{index}`), 0 = most recent.
    pub index: usize,
    pub old_oid: String,
    pub new_oid: String,
    pub short_new_oid: String,
    /// Operation keyword, e.g. "commit", "checkout", "reset", "merge",
    /// "rebase", "cherry-pick", "pull" (text before the first ':').
    pub action: String,
    /// Full reflog message, e.g. "checkout: moving from main to feature".
    pub message: String,
    pub timestamp: i64,
    /// Summary of the commit HEAD moved to, if it still exists.
    pub commit_summary: Option<String>,
    /// Local branch that restoring this entry would check out: set for
    /// "checkout: moving from X to Y" when branch Y still points at `new_oid`.
    pub checkout_branch: Option<String>,
}

fn action_of(message: &str) -> String {
    let head = message.split(':').next().unwrap_or("").trim();
    // "rebase (finish)", "commit (amend)", "pull --rebase (start)" -> first word.
    head.split(|c: char| c.is_whitespace() || c == '(')
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

fn checkout_target(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("checkout: moving from ")?;
    rest.rsplit(" to ").next().map(str::trim)
}

/// Read up to `limit` entries of the HEAD reflog, newest first.
pub fn read_head_reflog(repo: &Repository, limit: usize) -> Result<Vec<ReflogEntry>, TwigError> {
    let reflog = match repo.reflog("HEAD") {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for (index, entry) in reflog.iter().enumerate().take(limit) {
        let new_id = entry.id_new();
        if new_id == Oid::zero() {
            continue;
        }
        let message = entry.message().unwrap_or("").to_string();
        let commit_summary = repo
            .find_commit(new_id)
            .ok()
            .map(|c| c.summary().unwrap_or("").to_string());
        let checkout_branch = checkout_target(&message).and_then(|name| {
            let r = repo.find_branch(name, git2::BranchType::Local).ok()?;
            (r.get().target()? == new_id).then(|| name.to_string())
        });
        let new_oid = new_id.to_string();
        out.push(ReflogEntry {
            index,
            old_oid: entry.id_old().to_string(),
            short_new_oid: new_oid.chars().take(7).collect(),
            new_oid,
            action: action_of(&message),
            message,
            timestamp: entry.committer().when().seconds(),
            commit_summary,
            checkout_branch,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_actions() {
        assert_eq!(action_of("commit (amend): fix"), "commit");
        assert_eq!(action_of("rebase (finish): returning to refs/heads/x"), "rebase");
        assert_eq!(action_of("checkout: moving from a to b"), "checkout");
        assert_eq!(action_of("reset: moving to HEAD~1"), "reset");
        assert_eq!(action_of("pull --rebase (start): checkout x"), "pull");
        assert_eq!(checkout_target("checkout: moving from main to feat/x"), Some("feat/x"));
        assert_eq!(checkout_target("commit: x"), None);
    }

    async fn git(dir: &std::path::Path, args: &[&str]) {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = crate::git::writer::run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    #[tokio::test]
    async fn reads_head_reflog() {
        let dir = std::env::temp_dir().join(format!("twig-reflog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        let repo = Repository::open(&dir).unwrap();
        assert!(read_head_reflog(&repo, 10).unwrap().is_empty());

        std::fs::write(dir.join("a"), "1").unwrap();
        git(&dir, &["add", "a"]).await;
        git(&dir, &["commit", "-q", "-m", "one"]).await;
        git(&dir, &["checkout", "-q", "-b", "side"]).await;
        git(&dir, &["checkout", "-q", "main"]).await;

        let entries = read_head_reflog(&repo, 10).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].action, "checkout");
        assert_eq!(entries[0].checkout_branch.as_deref(), Some("main"));
        assert_eq!(entries[2].action, "commit");
        assert_eq!(entries[2].commit_summary.as_deref(), Some("one"));
        assert_eq!(read_head_reflog(&repo, 1).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
