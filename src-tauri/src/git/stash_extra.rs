//! Stash extras: SHA-addressed stash operations (show, rename, branch,
//! partial push). Indices (`stash@{N}`) shift whenever the stack changes, so
//! every write re-resolves the entry's index from its commit SHA immediately
//! before acting.

use std::path::Path;

use git2::{DiffOptions, Oid, Repository};
use serde::Serialize;

use crate::error::TwigError;
use crate::git::reader::{self, DiffFile};
use crate::git::writer::{self, run_git, run_git_paths, safe_ref, GitOutput};

#[derive(Debug, Clone, Serialize)]
pub struct StashDetail {
    pub index: u32,
    pub reference: String,
    pub oid: String,
    pub message: String,
    pub timestamp: String,
    /// Whether the entry carries untracked files (a third parent).
    pub has_untracked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StashDiff {
    /// Tracked changes: stash base commit → stashed working tree.
    pub tracked: Vec<DiffFile>,
    /// Untracked files saved with `--include-untracked` (`stash^3`).
    pub untracked: Vec<DiffFile>,
}

fn valid_sha(sha: &str) -> Result<(), TwigError> {
    if sha.len() < 7 || sha.len() > 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(TwigError::InvalidArgument(format!("'{sha}' is not a commit hash")));
    }
    Ok(())
}

/// List stash entries with their commit SHAs.
pub async fn stash_list_detailed(repo_path: &Path) -> Result<Vec<StashDetail>, TwigError> {
    let out = run_git(
        repo_path,
        &["stash", "list", "--format=%gd%x00%H%x00%P%x00%aI%x00%gs"],
    )
    .await?;
    if !out.success {
        return Ok(vec![]);
    }
    let mut entries = Vec::new();
    for line in out.stdout.lines() {
        let parts: Vec<&str> = line.splitn(5, '\0').collect();
        if parts.len() < 5 {
            continue;
        }
        let Ok(index) = parts[0]
            .trim_start_matches("stash@{")
            .trim_end_matches('}')
            .parse::<u32>()
        else {
            continue;
        };
        // `%gs` is the reflog subject ("On main: msg" / "WIP on main: ...").
        entries.push(StashDetail {
            index,
            reference: parts[0].to_string(),
            oid: parts[1].to_string(),
            has_untracked: parts[2].split_whitespace().count() >= 3,
            timestamp: parts[3].to_string(),
            message: parts[4].to_string(),
        });
    }
    Ok(entries)
}

/// Resolve a stash SHA to its current `stash@{N}` reference.
async fn stash_ref_for(repo_path: &Path, sha: &str) -> Result<String, TwigError> {
    valid_sha(sha)?;
    match writer::stash_index_for_sha(repo_path, sha).await? {
        Some(i) => Ok(format!("stash@{{{i}}}")),
        None => Err(TwigError::GitCli(
            "that stash no longer exists (the stash list changed)".to_string(),
        )),
    }
}

/// apply / pop / drop the stash whose commit is `sha`.
pub async fn stash_act(repo_path: &Path, sha: &str, action: &str) -> Result<GitOutput, TwigError> {
    let verb = match action {
        "apply" | "pop" | "drop" => action,
        _ => {
            return Err(TwigError::InvalidArgument(format!(
                "unknown stash action '{action}'"
            )))
        }
    };
    let r = stash_ref_for(repo_path, sha).await?;
    run_git(repo_path, &["stash", verb, &r]).await
}

/// Change a stash entry's message, keeping the same stash commit.
/// The new entry is stored first (so the commit is never unreferenced), then
/// the old reflog entry is dropped. The renamed entry moves to the top.
pub async fn stash_rename(repo_path: &Path, sha: &str, message: &str) -> Result<GitOutput, TwigError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(TwigError::InvalidArgument("stash message cannot be empty".to_string()));
    }
    let old_ref = stash_ref_for(repo_path, sha).await?;
    let old_index: u32 = old_ref
        .trim_start_matches("stash@{")
        .trim_end_matches('}')
        .parse()
        .map_err(|_| TwigError::GitCli("could not parse stash index".to_string()))?;
    let stored = run_git(repo_path, &["stash", "store", "-m", message, sha]).await?;
    if !stored.success {
        return Ok(stored);
    }
    // After `store` the old entry sits one position lower; verify before dropping.
    let shifted = format!("stash@{{{}}}", old_index + 1);
    let check = run_git(repo_path, &["rev-parse", "--verify", "--quiet", &shifted]).await?;
    if !check.success || check.stdout.trim() != sha {
        return Err(TwigError::GitCli(
            "stash list changed during rename; the renamed copy was kept at the top".to_string(),
        ));
    }
    run_git(repo_path, &["stash", "drop", &shifted]).await
}

/// `git stash branch <name> <stash>`: check out a new branch at the stash's
/// base commit, apply the stash and drop it on success.
pub async fn stash_branch(repo_path: &Path, sha: &str, branch: &str) -> Result<GitOutput, TwigError> {
    safe_ref(branch)?;
    let r = stash_ref_for(repo_path, sha).await?;
    run_git(repo_path, &["stash", "branch", branch, &r]).await
}

/// `git stash push` with options; `paths` limits the stash to those files.
pub async fn stash_push_ext(
    repo_path: &Path,
    message: Option<&str>,
    paths: &[&str],
    keep_index: bool,
    include_untracked: bool,
) -> Result<GitOutput, TwigError> {
    let mut args: Vec<&str> = vec!["stash", "push"];
    if include_untracked {
        args.push("--include-untracked");
    }
    if keep_index {
        args.push("--keep-index");
    }
    if let Some(m) = message.filter(|m| !m.trim().is_empty()) {
        args.push("-m");
        args.push(m);
    }
    if paths.is_empty() {
        run_git(repo_path, &args).await
    } else {
        run_git_paths(repo_path, &args, paths).await
    }
}

/// Diff of a stash entry: tracked changes and the untracked-files commit.
pub fn read_stash_diff(repo: &Repository, sha: &str) -> Result<StashDiff, TwigError> {
    valid_sha(sha)?;
    let commit = repo.find_commit(Oid::from_str(sha)?)?;
    let tree = commit.tree()?;
    let base_tree = commit.parent(0)?.tree()?;

    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    let mut diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&tree), Some(&mut opts))?;
    reader::detect_renames(&mut diff)?;
    let tracked = reader::parse_diff(&diff)?;

    let untracked = if commit.parent_count() >= 3 {
        let u_tree = commit.parent(2)?.tree()?;
        let mut uopts = DiffOptions::new();
        uopts.context_lines(3);
        let udiff = repo.diff_tree_to_tree(None, Some(&u_tree), Some(&mut uopts))?;
        reader::parse_diff(&udiff)?
    } else {
        vec![]
    };

    Ok(StashDiff { tracked, untracked })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git_ok(dir: &Path, args: &[&str]) {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-stashx-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        git_ok(&dir, &["config", "user.name", "t"]).await;
        git_ok(&dir, &["config", "user.email", "t@t"]).await;
        git_ok(&dir, &["config", "commit.gpgsign", "false"]).await;
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        std::fs::write(dir.join("b.txt"), "b\n").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "init"]).await;
        dir
    }

    /// A pop that cannot apply cleanly must keep the stash entry.
    #[tokio::test]
    async fn failed_pop_keeps_the_stash() {
        let dir = temp_repo("popfail").await;
        // Conflicting tracked change.
        std::fs::write(dir.join("a.txt"), "stashed\n").unwrap();
        let out = stash_push_ext(&dir, Some("s1"), &[], false, true).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        std::fs::write(dir.join("a.txt"), "committed\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "diverge"]).await;
        let sha = stash_list_detailed(&dir).await.unwrap()[0].oid.clone();
        let out = stash_act(&dir, &sha, "pop").await.unwrap();
        assert!(!out.success);
        assert_eq!(stash_list_detailed(&dir).await.unwrap()[0].oid, sha);
        assert!(std::fs::read_to_string(dir.join("a.txt")).unwrap().contains("<<<<<<<"));
        git_ok(&dir, &["checkout", "-q", "HEAD", "--", "a.txt"]).await;
        git_ok(&dir, &["reset", "-q"]).await;
        git_ok(&dir, &["checkout", "-q", "--", "."]).await;

        // Untracked file that now exists again in the working tree.
        std::fs::write(dir.join("u.txt"), "stashed untracked\n").unwrap();
        let out = stash_push_ext(&dir, Some("s2"), &[], false, true).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        std::fs::write(dir.join("u.txt"), "new local\n").unwrap();
        let sha2 = stash_list_detailed(&dir).await.unwrap()[0].oid.clone();
        let out = stash_act(&dir, &sha2, "pop").await.unwrap();
        assert!(!out.success);
        assert_eq!(std::fs::read_to_string(dir.join("u.txt")).unwrap(), "new local\n");
        let list = stash_list_detailed(&dir).await.unwrap();
        assert!(list.iter().any(|e| e.oid == sha2), "stash with untracked file was lost");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn partial_push_show_rename_and_branch() {
        let dir = temp_repo("ops").await;
        std::fs::write(dir.join("a.txt"), "a2\n").unwrap();
        std::fs::write(dir.join("b.txt"), "b2\n").unwrap();
        std::fs::write(dir.join("new.txt"), "n\n").unwrap();

        // Only a.txt is stashed.
        let out = stash_push_ext(&dir, Some("only a"), &["a.txt"], false, false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "a\n");
        assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap(), "b2\n");

        // Everything else, including untracked.
        let out = stash_push_ext(&dir, Some("rest"), &[], false, true).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(!dir.join("new.txt").exists());

        let list = stash_list_detailed(&dir).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].message.ends_with("rest"));
        assert!(list[0].has_untracked);
        assert!(!list[1].has_untracked);

        let repo = Repository::open(&dir).unwrap();
        let d = read_stash_diff(&repo, &list[0].oid).unwrap();
        assert_eq!(d.tracked.len(), 1);
        assert_eq!(d.untracked.len(), 1);
        assert_eq!(d.untracked[0].new_path.as_deref(), Some("new.txt"));

        // Rename the older entry; its commit is preserved.
        let older = list[1].oid.clone();
        let out = stash_rename(&dir, &older, "renamed a").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let list = stash_list_detailed(&dir).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].oid, older);
        assert_eq!(list[0].message, "renamed a");

        // Branch from it.
        let out = stash_branch(&dir, &older, "from-stash").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "a2\n");
        assert_eq!(stash_list_detailed(&dir).await.unwrap().len(), 1);
        assert!(stash_branch(&dir, &older, "x").await.is_err());
        assert!(stash_branch(&dir, &list[1].oid, "-bad").await.is_err());

        let top = stash_list_detailed(&dir).await.unwrap()[0].oid.clone();
        let out = stash_act(&dir, &top, "drop").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(stash_act(&dir, &top, "nuke").await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
