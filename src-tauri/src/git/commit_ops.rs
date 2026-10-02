//! Commit-level write operations used by the commit graph's context menu and
//! the undo history: detached checkout, cherry-pick, revert, reset, and
//! restoring HEAD to a reflog entry. All writes go through the git CLI.
use std::path::Path;

use serde::Serialize;

use super::writer::{has_uncommitted_changes, rev_exists, run_git, safe_ref, GitOutput};
use crate::error::TwigError;

/// Outcome of a commit operation, with enough context for the UI to offer an
/// "Undo" (the HEAD / branch it moved away from) and to flag conflicts.
#[derive(Debug, Serialize)]
pub struct CommitOpResult {
    pub success: bool,
    pub message: String,
    /// True when a cherry-pick/revert stopped with conflicts; the repo is left
    /// in that state for the user (or the conflict UI) to resolve or abort.
    pub conflicted: bool,
    /// HEAD commit before the operation (None for an unborn branch).
    pub previous_head: Option<String>,
    /// Branch HEAD was on before the operation (None when detached).
    pub previous_branch: Option<String>,
    /// Stash commit created to preserve uncommitted changes, if any.
    pub stash_oid: Option<String>,
}

impl CommitOpResult {
    fn from_output(out: GitOutput, before: &HeadState) -> Self {
        CommitOpResult {
            success: out.success,
            message: if out.success { out.stdout } else { out.stderr },
            conflicted: false,
            previous_head: before.oid.clone(),
            previous_branch: before.branch.clone(),
            stash_oid: None,
        }
    }
}

/// Where HEAD currently points.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadState {
    pub oid: Option<String>,
    pub branch: Option<String>,
}

pub async fn head_state(repo_path: &Path) -> Result<HeadState, TwigError> {
    let oid_out = run_git(repo_path, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]).await?;
    let oid = oid_out
        .success
        .then(|| oid_out.stdout.trim().to_string())
        .filter(|s| !s.is_empty());
    let br = run_git(repo_path, &["symbolic-ref", "-q", "--short", "HEAD"]).await?;
    let branch = br
        .success
        .then(|| br.stdout.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(HeadState { oid, branch })
}

/// Resolve `rev` to a full commit id, rejecting anything that is not a commit.
async fn resolve_commit(repo_path: &Path, rev: &str) -> Result<String, TwigError> {
    safe_ref(rev)?;
    let spec = format!("{rev}^{{commit}}");
    let out = run_git(repo_path, &["rev-parse", "--verify", "--quiet", &spec]).await?;
    let oid = out.stdout.trim().to_string();
    if !out.success || oid.is_empty() {
        return Err(TwigError::InvalidArgument(format!("'{rev}' is not a commit")));
    }
    Ok(oid)
}

async fn parent_count(repo_path: &Path, oid: &str) -> Result<usize, TwigError> {
    let out = run_git(repo_path, &["rev-list", "--parents", "-n", "1", oid]).await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    Ok(out.stdout.split_whitespace().count().saturating_sub(1))
}

/// Check out a commit with a detached HEAD.
pub async fn checkout_detached(repo_path: &Path, rev: &str) -> Result<CommitOpResult, TwigError> {
    let oid = resolve_commit(repo_path, rev).await?;
    let before = head_state(repo_path).await?;
    let out = run_git(repo_path, &["checkout", "--detach", &oid]).await?;
    Ok(CommitOpResult::from_output(out, &before))
}

/// Shared implementation of cherry-pick / revert. Merge commits use the
/// first parent as mainline. On conflicts the sequencer state is kept.
async fn apply_commit(repo_path: &Path, verb: &str, rev: &str) -> Result<CommitOpResult, TwigError> {
    let oid = resolve_commit(repo_path, rev).await?;
    let before = head_state(repo_path).await?;
    if verb == "cherry-pick" && before.oid.is_none() {
        // Cherry-picking onto an unborn branch works in git, but there is
        // nothing to undo to, which the UI relies on. Refuse explicitly.
        return Err(TwigError::InvalidArgument(
            "cannot cherry-pick onto a branch without commits".to_string(),
        ));
    }
    let mut args: Vec<&str> = vec![verb];
    if verb == "revert" {
        args.push("--no-edit");
    }
    if parent_count(repo_path, &oid).await? > 1 {
        args.extend_from_slice(&["-m", "1"]);
    }
    args.push(&oid);
    let out = run_git(repo_path, &args).await?;
    let mut result = CommitOpResult::from_output(out, &before);
    if !result.success {
        let marker = if verb == "revert" { "REVERT_HEAD" } else { "CHERRY_PICK_HEAD" };
        result.conflicted = rev_exists(repo_path, marker).await?;
    }
    Ok(result)
}

pub async fn cherry_pick(repo_path: &Path, rev: &str) -> Result<CommitOpResult, TwigError> {
    apply_commit(repo_path, "cherry-pick", rev).await
}

/// Most commits one range cherry-pick accepts.
pub const MAX_PICK: usize = 500;

/// Outcome of [`cherry_pick_many`]: the usual result plus what was skipped.
#[derive(Debug, Serialize)]
pub struct PickManyResult {
    #[serde(flatten)]
    pub result: CommitOpResult,
    /// Commits applied before stopping (all of them on success).
    pub picked: usize,
    /// Commits left out because HEAD already contains them.
    pub skipped: Vec<String>,
}

/// Cherry-pick several commits onto HEAD, in the given order (the caller
/// passes them oldest first). Commits HEAD already contains are left out.
/// On conflicts git's sequencer state is kept, so the operation banner's
/// continue / skip / abort apply to the rest of the range.
pub async fn cherry_pick_many(repo_path: &Path, revs: &[String]) -> Result<PickManyResult, TwigError> {
    if revs.is_empty() {
        return Err(TwigError::InvalidArgument("no commits to cherry-pick".to_string()));
    }
    if revs.len() > MAX_PICK {
        return Err(TwigError::InvalidArgument(format!("at most {MAX_PICK} commits can be cherry-picked at once")));
    }
    let before = head_state(repo_path).await?;
    let Some(head) = before.oid.clone() else {
        return Err(TwigError::InvalidArgument(
            "cannot cherry-pick onto a branch without commits".to_string(),
        ));
    };
    let mut oids = Vec::with_capacity(revs.len());
    let mut skipped = Vec::new();
    let mut any_merge = false;
    for rev in revs {
        let oid = resolve_commit(repo_path, rev).await?;
        if oids.contains(&oid) || skipped.contains(&oid) {
            continue;
        }
        let contained = run_git(repo_path, &["merge-base", "--is-ancestor", &oid, &head]).await?;
        if contained.success {
            skipped.push(oid);
            continue;
        }
        any_merge |= parent_count(repo_path, &oid).await? > 1;
        oids.push(oid);
    }
    if oids.is_empty() {
        return Err(TwigError::InvalidArgument(
            "every selected commit is already on HEAD".to_string(),
        ));
    }
    let mut args: Vec<&str> = vec!["cherry-pick"];
    if any_merge {
        // Mainline for the merges; git ignores it for ordinary commits.
        args.extend_from_slice(&["-m", "1"]);
    }
    args.extend(oids.iter().map(String::as_str));
    let out = run_git(repo_path, &args).await?;
    let mut result = CommitOpResult::from_output(out, &before);
    let mut picked = oids.len();
    if !result.success {
        result.conflicted = rev_exists(repo_path, "CHERRY_PICK_HEAD").await?;
        // Commits applied so far: HEAD moved past the old head by that many.
        let range = format!("{head}..HEAD");
        let count = run_git(repo_path, &["rev-list", "--count", &range]).await?;
        picked = count.stdout.trim().parse().unwrap_or(0);
    }
    Ok(PickManyResult { result, picked, skipped })
}

pub async fn revert(repo_path: &Path, rev: &str) -> Result<CommitOpResult, TwigError> {
    apply_commit(repo_path, "revert", rev).await
}

/// Snapshot uncommitted (tracked) changes into the stash list without touching
/// the working tree, so a following hard reset can always be recovered from.
async fn snapshot_to_stash(repo_path: &Path, message: &str) -> Result<Option<String>, TwigError> {
    let out = run_git(repo_path, &["stash", "create", message]).await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    let sha = out.stdout.trim().to_string();
    if sha.is_empty() {
        return Ok(None);
    }
    let store = run_git(repo_path, &["stash", "store", "-m", message, &sha]).await?;
    if !store.success {
        return Err(TwigError::GitCli(store.stderr));
    }
    Ok(Some(sha))
}

/// Reset the current branch (or detached HEAD) to `rev`.
/// `mode` is one of `soft`, `mixed`, `hard`, `keep`. Before a hard reset any
/// uncommitted tracked changes are saved to the stash list.
pub async fn reset(repo_path: &Path, rev: &str, mode: &str) -> Result<CommitOpResult, TwigError> {
    let flag = match mode {
        "soft" => "--soft",
        "mixed" => "--mixed",
        "hard" => "--hard",
        "keep" => "--keep",
        other => {
            return Err(TwigError::InvalidArgument(format!("unknown reset mode '{other}'")));
        }
    };
    let oid = resolve_commit(repo_path, rev).await?;
    let before = head_state(repo_path).await?;
    let mut stash_oid = None;
    if mode == "hard" && before.oid.is_some() && has_uncommitted_changes(repo_path).await? {
        let short: String = oid.chars().take(7).collect();
        stash_oid =
            snapshot_to_stash(repo_path, &format!("Twig: changes before reset --hard to {short}"))
                .await?;
    }
    let out = run_git(repo_path, &["reset", "-q", flag, &oid]).await?;
    let mut result = CommitOpResult::from_output(out, &before);
    result.stash_oid = stash_oid;
    Ok(result)
}

/// Move HEAD back to a previous state (a reflog entry or an "undo" target).
///
/// * If `branch` names a local branch that points at `rev`, it is checked out
///   (restoring a "checkout" step).
/// * Otherwise, when HEAD is on a branch, that branch is hard-reset to `rev`.
/// * When HEAD is detached, `rev` is checked out detached.
///
/// Uncommitted changes (including untracked files) are stashed first when
/// `auto_stash` is set; otherwise a dirty working tree is refused.
pub async fn restore_head(
    repo_path: &Path,
    rev: &str,
    branch: Option<&str>,
    auto_stash: bool,
) -> Result<CommitOpResult, TwigError> {
    let oid = resolve_commit(repo_path, rev).await?;
    let before = head_state(repo_path).await?;

    let mut stash_oid = None;
    if has_uncommitted_changes(repo_path).await? {
        if !auto_stash {
            return Err(TwigError::InvalidArgument(
                "the working tree has uncommitted changes; commit or stash them first".to_string(),
            ));
        }
        let short: String = oid.chars().take(7).collect();
        let msg = format!("Twig: auto-stash before restoring to {short}");
        let out = run_git(repo_path, &["stash", "push", "--include-untracked", "-m", &msg]).await?;
        if !out.success {
            return Ok(CommitOpResult::from_output(out, &before));
        }
        let top = run_git(repo_path, &["rev-parse", "--verify", "--quiet", "refs/stash"]).await?;
        stash_oid = top
            .success
            .then(|| top.stdout.trim().to_string())
            .filter(|s| !s.is_empty());
    }

    let target_branch = match branch {
        Some(b) => {
            safe_ref(b)?;
            let full = format!("refs/heads/{b}");
            let out = run_git(repo_path, &["rev-parse", "--verify", "--quiet", &full]).await?;
            (out.success && out.stdout.trim() == oid).then_some(b)
        }
        None => None,
    };

    let out = if let Some(b) = target_branch {
        if before.branch.as_deref() == Some(b) {
            // Already there; nothing to move.
            GitOutput { success: true, stdout: String::new(), stderr: String::new() }
        } else {
            // `switch` always reads its argument as a branch, never a path.
            run_git(repo_path, &["switch", "-q", "--no-guess", b]).await?
        }
    } else if before.branch.is_some() {
        run_git(repo_path, &["reset", "-q", "--hard", &oid]).await?
    } else {
        run_git(repo_path, &["checkout", "-q", "--detach", &oid]).await?
    };

    let mut result = CommitOpResult::from_output(out, &before);
    result.stash_oid = stash_oid;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    async fn commit_file(dir: &Path, file: &str, content: &str, msg: &str) -> String {
        std::fs::write(dir.join(file), content).unwrap();
        git(dir, &["add", "--", file]).await;
        git(dir, &["commit", "-q", "-m", msg]).await;
        git(dir, &["rev-parse", "HEAD"]).await
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-commitops-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        // Commits made by the operations under test need an identity too.
        git(&dir, &["config", "user.name", "t"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "commit.gpgsign", "false"]).await;
        dir
    }

    #[tokio::test]
    async fn cherry_pick_many_mixes_merges_and_plain_commits() {
        let dir = temp_repo("pickmerge").await;
        commit_file(&dir, "a.txt", "base\n", "base").await;
        git(&dir, &["checkout", "-q", "-b", "topic"]).await;
        commit_file(&dir, "t.txt", "topic\n", "topic work").await;
        git(&dir, &["checkout", "-q", "-b", "integration", "main"]).await;
        commit_file(&dir, "i.txt", "i\n", "integration work").await;
        git(&dir, &["merge", "-q", "--no-ff", "--no-edit", "topic"]).await;
        let merge = git(&dir, &["rev-parse", "HEAD"]).await;
        let after = commit_file(&dir, "c.txt", "c\n", "after merge").await;
        git(&dir, &["checkout", "-q", "main"]).await;

        let r = cherry_pick_many(&dir, &[merge, after]).await.unwrap();
        assert!(r.result.success, "{}", r.result.message);
        assert_eq!(r.picked, 2);
        assert!(dir.join("t.txt").exists() && dir.join("c.txt").exists());
        assert!(!dir.join("i.txt").exists(), "only the merge's first-parent diff is applied");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cherry_pick_many_applies_in_order_and_stops_on_conflict() {
        let dir = temp_repo("pickmany").await;
        let base = commit_file(&dir, "a.txt", "base\n", "base").await;
        git(&dir, &["checkout", "-q", "-b", "feature"]).await;
        let one = commit_file(&dir, "b.txt", "one\n", "one").await;
        let two = commit_file(&dir, "b.txt", "two\n", "two").await;
        let three = commit_file(&dir, "a.txt", "feature side\n", "three").await;
        git(&dir, &["checkout", "-q", "main"]).await;

        // Oldest first: "two" builds on "one"'s file, so order matters.
        let r = cherry_pick_many(&dir, &[one.clone(), two.clone()]).await.unwrap();
        assert!(r.result.success, "{}", r.result.message);
        assert_eq!(r.picked, 2);
        assert_eq!(r.result.previous_head.as_deref(), Some(base.as_str()));
        assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap(), "two\n");
        let log = git(&dir, &["log", "--format=%s", "-n", "3"]).await;
        assert_eq!(log.lines().collect::<Vec<_>>(), ["two", "one", "base"]);

        // Already on HEAD (the base) is skipped; a conflict keeps the sequencer.
        commit_file(&dir, "a.txt", "main side\n", "main edit").await;
        let r = cherry_pick_many(&dir, &[base.clone(), three.clone()]).await.unwrap();
        assert_eq!(r.skipped, vec![base.clone()]);
        assert!(!r.result.success);
        assert!(r.result.conflicted);
        assert_eq!(r.picked, 0);
        git(&dir, &["cherry-pick", "--abort"]).await;

        assert!(cherry_pick_many(&dir, std::slice::from_ref(&base)).await.is_err(), "nothing left to pick");
        assert!(cherry_pick_many(&dir, &[]).await.is_err());
        assert!(cherry_pick_many(&dir, &["-n".into()]).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cherry_pick_revert_and_conflicts() {
        let dir = temp_repo("pick").await;
        let base = commit_file(&dir, "a.txt", "base\n", "base").await;
        git(&dir, &["checkout", "-q", "-b", "feature"]).await;
        let feat = commit_file(&dir, "b.txt", "feature\n", "add b").await;
        let conflicting = commit_file(&dir, "a.txt", "feature side\n", "edit a").await;
        git(&dir, &["checkout", "-q", "main"]).await;

        let r = cherry_pick(&dir, &feat).await.unwrap();
        assert!(r.success, "{}", r.message);
        assert_eq!(r.previous_head.as_deref(), Some(base.as_str()));
        assert_eq!(r.previous_branch.as_deref(), Some("main"));
        assert!(dir.join("b.txt").exists());

        let r = revert(&dir, "HEAD").await.unwrap();
        assert!(r.success, "{}", r.message);
        assert!(!dir.join("b.txt").exists());

        commit_file(&dir, "a.txt", "main side\n", "main edit").await;
        let r = cherry_pick(&dir, &conflicting).await.unwrap();
        assert!(!r.success);
        assert!(r.conflicted);
        assert!(rev_exists(&dir, "CHERRY_PICK_HEAD").await.unwrap());
        git(&dir, &["cherry-pick", "--abort"]).await;

        assert!(cherry_pick(&dir, "-n").await.is_err());
        assert!(cherry_pick(&dir, "does-not-exist").await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reset_modes_and_hard_reset_snapshot() {
        let dir = temp_repo("reset").await;
        let c1 = commit_file(&dir, "a.txt", "1\n", "c1").await;
        let c2 = commit_file(&dir, "a.txt", "2\n", "c2").await;

        let r = reset(&dir, &c1, "soft").await.unwrap();
        assert!(r.success, "{}", r.message);
        assert_eq!(r.previous_head.as_deref(), Some(c2.as_str()));
        assert_eq!(git(&dir, &["diff", "--cached", "--name-only"]).await, "a.txt");

        // Undo: back to c2 with the same mode.
        assert!(reset(&dir, &c2, "soft").await.unwrap().success);
        assert!(reset(&dir, &c1, "bogus").await.is_err());

        std::fs::write(dir.join("a.txt"), "dirty\n").unwrap();
        let r = reset(&dir, &c1, "hard").await.unwrap();
        assert!(r.success, "{}", r.message);
        assert!(r.stash_oid.is_some());
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "1\n");
        let stash = git(&dir, &["stash", "list"]).await;
        assert!(stash.contains("before reset --hard"), "{stash}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn restore_head_variants() {
        let dir = temp_repo("restore").await;
        let c1 = commit_file(&dir, "a.txt", "1\n", "c1").await;
        let c2 = commit_file(&dir, "a.txt", "2\n", "c2").await;

        // Dirty tree is refused without auto-stash.
        std::fs::write(dir.join("new.txt"), "x").unwrap();
        assert!(restore_head(&dir, &c1, None, false).await.is_err());

        // With auto-stash: untracked file is stashed, branch reset.
        let r = restore_head(&dir, &c1, None, true).await.unwrap();
        assert!(r.success, "{}", r.message);
        assert!(r.stash_oid.is_some());
        assert!(!dir.join("new.txt").exists());
        assert_eq!(git(&dir, &["rev-parse", "main"]).await, c1);

        // Restoring a checkout step switches to the named branch.
        git(&dir, &["branch", "other", &c2]).await;
        let r = restore_head(&dir, &c2, Some("other"), false).await.unwrap();
        assert!(r.success, "{}", r.message);
        assert_eq!(head_state(&dir).await.unwrap().branch.as_deref(), Some("other"));
        assert_eq!(git(&dir, &["rev-parse", "main"]).await, c1);

        // Detached HEAD moves detached.
        let r = checkout_detached(&dir, &c1).await.unwrap();
        assert!(r.success, "{}", r.message);
        assert_eq!(r.previous_branch.as_deref(), Some("other"));
        let r = restore_head(&dir, &c2, None, false).await.unwrap();
        assert!(r.success, "{}", r.message);
        let head = head_state(&dir).await.unwrap();
        assert_eq!(head.branch, None);
        assert_eq!(head.oid.as_deref(), Some(c2.as_str()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
