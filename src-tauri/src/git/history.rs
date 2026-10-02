//! History rewriting: rebase onto, non-interactive driving of interactive
//! rebase, and force push with lease.
use std::path::Path;

use std::collections::HashSet;

use git2::{Commit, Oid, Repository, Sort};
use serde::{Deserialize, Serialize};

use crate::error::TwigError;
use crate::git::conflicts::{run_git_noedit, shell_path, shell_quote};
use crate::git::writer::{run_git, safe_ref, GitOutput};

/// Upper bound on commits shown in the interactive rebase editor.
const MAX_REBASE_COMMITS: usize = 1000;
/// Upper bound on new-base commits compared by patch id (like
/// `git rebase`'s cherry-pick detection); beyond it the check is skipped.
const MAX_UPSTREAM_SCAN: usize = 2000;

#[derive(Debug, Clone, Serialize)]
pub struct RebaseCommit {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub message: String,
    pub author_name: String,
    pub timestamp: i64,
    /// The new base already has a commit with the same patch id; `git rebase`
    /// would leave this commit out, and picking it makes it empty.
    pub already_upstream: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RebaseCommitList {
    /// Oldest first (todo order).
    pub commits: Vec<RebaseCommit>,
    /// Merge commits in the range; git drops them during a plain `rebase -i`.
    pub merges_skipped: u32,
    /// Full OID of the resolved base (`None` when rebasing from the root).
    pub base_oid: Option<String>,
    /// The base is not an ancestor of HEAD, so even an unchanged todo moves
    /// the branch onto it.
    pub onto_new_base: bool,
}

/// Patch id of a non-merge commit against its parent; `None` for merges,
/// commits without changes, and commits touching binary files (libgit2's
/// patch id ignores binary content, so any two edits of one binary file
/// would match).
fn patch_id(repo: &Repository, commit: &Commit) -> Result<Option<Oid>, TwigError> {
    if commit.parent_count() > 1 {
        return Ok(None);
    }
    let parent_tree = match commit.parent_count() {
        0 => None,
        _ => Some(commit.parent(0)?.tree()?),
    };
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit.tree()?), None)?;
    if diff.deltas().len() == 0 {
        return Ok(None);
    }
    for idx in 0..diff.deltas().len() {
        // Loading the patch is what detects binary content.
        let binary = match git2::Patch::from_diff(&diff, idx)? {
            Some(patch) => patch.delta().flags().is_binary(),
            None => true,
        };
        if binary {
            return Ok(None);
        }
    }
    Ok(Some(diff.patchid(None)?))
}

/// Patch ids of the commits on the new base's side (`HEAD..base`), or `None`
/// when there are too many to compare.
fn upstream_patch_ids(
    repo: &Repository,
    head: Oid,
    base: Oid,
) -> Result<Option<HashSet<Oid>>, TwigError> {
    let mut walk = repo.revwalk()?;
    walk.push(base)?;
    walk.hide(head)?;
    let oids: Vec<Oid> = walk.take(MAX_UPSTREAM_SCAN + 1).collect::<Result<_, _>>()?;
    if oids.len() > MAX_UPSTREAM_SCAN {
        return Ok(None);
    }
    let mut ids = HashSet::new();
    for oid in oids {
        if let Some(id) = patch_id(repo, &repo.find_commit(oid)?)? {
            ids.insert(id);
        }
    }
    Ok(Some(ids))
}

#[derive(Debug, Clone, Deserialize)]
pub struct RebaseTodoItem {
    pub oid: String,
    /// pick | reword | edit | squash | fixup | drop
    pub action: String,
    /// New message for `reword`, or replacement combined message for a
    /// `squash` group (applied after the group's last squash/fixup).
    pub message: Option<String>,
}

/// List the commits `git rebase -i <base>` would offer (base..HEAD), marking
/// those the new base already has.
pub fn list_rebase_commits(
    repo: &Repository,
    base: Option<&str>,
) -> Result<RebaseCommitList, TwigError> {
    list_rebase_commits_opts(repo, base, true)
}

/// Like [`list_rebase_commits`]; `detect_upstream = false` skips the patch-id
/// comparison (`already_upstream` is then always false).
pub fn list_rebase_commits_opts(
    repo: &Repository,
    base: Option<&str>,
    detect_upstream: bool,
) -> Result<RebaseCommitList, TwigError> {
    let head = repo
        .head()
        .map_err(|_| TwigError::InvalidArgument("HEAD has no commits".to_string()))?
        .peel_to_commit()?;

    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    walk.push(head.id())?;
    let base_oid = match base {
        Some(b) => {
            safe_ref(b)?;
            let oid = repo
                .revparse_single(b)
                .and_then(|o| o.peel_to_commit())
                .map_err(|_| TwigError::InvalidArgument(format!("unknown revision '{b}'")))?
                .id();
            walk.hide(oid)?;
            Some(oid.to_string())
        }
        None => None,
    };

    let mut onto_new_base = false;
    let mut upstream_ids = None;
    if let Some(base) = base_oid.as_deref() {
        let base = Oid::from_str(base)?;
        if repo.merge_base(head.id(), base).ok() != Some(base) {
            onto_new_base = true;
            if detect_upstream {
                // Best effort: e.g. a partial clone may lack the blobs.
                upstream_ids = upstream_patch_ids(repo, head.id(), base).unwrap_or_else(|e| {
                    log::warn!("skipping already-upstream check: {e}");
                    None
                });
            }
        }
    }

    let mut commits = Vec::new();
    let mut merges_skipped = 0u32;
    for oid in walk {
        let commit = repo.find_commit(oid?)?;
        if commit.parent_count() > 1 {
            merges_skipped += 1;
            continue;
        }
        if commits.len() >= MAX_REBASE_COMMITS {
            return Err(TwigError::InvalidArgument(format!(
                "more than {MAX_REBASE_COMMITS} commits in range; pick a closer base"
            )));
        }
        let already_upstream = match &upstream_ids {
            Some(ids) if !ids.is_empty() => {
                patch_id(repo, &commit).ok().flatten().is_some_and(|p| ids.contains(&p))
            }
            _ => false,
        };
        let id = commit.id().to_string();
        commits.push(RebaseCommit {
            short_oid: id.chars().take(7).collect(),
            oid: id,
            summary: crate::git::graph::commit_summary(&commit),
            message: crate::git::graph::commit_message(&commit).trim_end().to_string(),
            author_name: crate::git::graph::commit_author_name(&commit),
            timestamp: commit.time().seconds(),
            already_upstream,
        });
    }

    Ok(RebaseCommitList {
        commits,
        merges_skipped,
        base_oid,
        onto_new_base,
    })
}

/// `git rebase [--autostash] <upstream>`.
pub async fn rebase_onto(
    repo_path: &Path,
    upstream: &str,
    autostash: bool,
) -> Result<GitOutput, TwigError> {
    safe_ref(upstream)?;
    let mut args = vec!["rebase"];
    if autostash {
        args.push("--autostash");
    }
    args.push(upstream);
    args.push("--");
    run_git_noedit(repo_path, &args, &[]).await
}

fn is_hex_oid(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Writes the helper files that `exec` lines in the todo refer to.
struct TodoFiles<'a> {
    dir: &'a Path,
    count: usize,
}

impl TodoFiles<'_> {
    fn path(&mut self, prefix: &str) -> std::path::PathBuf {
        self.count += 1;
        self.dir.join(format!("{prefix}-{}.txt", self.count))
    }

    /// `exec` line recording HEAD (empty when unborn) into a new mark file.
    fn record_head(&mut self) -> (String, String) {
        let mark = shell_quote(&shell_path(&self.path("head")));
        (format!("exec git rev-parse -q --verify HEAD > {mark} || :"), mark)
    }

    /// `exec` line that replaces HEAD's message with `msg`, but only if
    /// `current` (a shell expression for a commit id) differs from the HEAD
    /// recorded in `mark`. A pick that ends up empty is dropped (or skipped
    /// by the user), and amending then would rewrite the previous, unrelated
    /// commit.
    fn guarded_amend(&mut self, msg: &str, current: &str, mark: &str) -> Result<String, TwigError> {
        let file = self.path("msg");
        std::fs::write(&file, format!("{}\n", msg.trim()))?;
        Ok(format!(
            "exec test \"{current}\" = \"$(cat {mark})\" || \
             git commit --amend --allow-empty --no-verify --cleanup=whitespace -F {}",
            shell_quote(&shell_path(&file))
        ))
    }
}

const HEAD_NOW: &str = "$(git rev-parse -q --verify HEAD)";

/// Close a squash group that has a replacement message. `group` holds the
/// `lines` indices where the group's first commit starts and ends. HEAD is
/// recorded on both sides of that commit, and the message is applied after
/// the group only if the first commit landed: if it was dropped as empty,
/// git melds the squashes into the previous commit, whose message must stay.
fn end_group(
    lines: &mut Vec<String>,
    files: &mut TodoFiles,
    msg: &str,
    group: Option<(usize, usize)>,
) -> Result<(), TwigError> {
    let (start, end) = group.ok_or_else(|| {
        TwigError::InvalidArgument("squash without a commit to squash into".to_string())
    })?;
    let (before, before_mark) = files.record_head();
    let (after, after_mark) = files.record_head();
    lines.insert(end, after);
    lines.insert(start, before);
    lines.push(files.guarded_amend(msg, &format!("$(cat {after_mark})"), &before_mark)?);
    Ok(())
}

/// Build the todo list text, writing reword/squash messages to files in
/// `msg_dir`. Validates actions and ordering.
pub(crate) fn build_todo(items: &[RebaseTodoItem], msg_dir: &Path) -> Result<String, TwigError> {
    let mut lines: Vec<String> = Vec::new();
    let mut files = TodoFiles { dir: msg_dir, count: 0 };
    let mut saw_commit = false;
    // Message to apply after the current squash/fixup group ends.
    let mut pending_group_msg: Option<String> = None;
    // Index range in `lines` of the current group's first commit.
    let mut group: Option<(usize, usize)> = None;

    for (i, item) in items.iter().enumerate() {
        if !is_hex_oid(&item.oid) {
            return Err(TwigError::InvalidArgument(format!(
                "'{}' is not a full commit id",
                item.oid
            )));
        }
        let action = item.action.as_str();
        let msg = item
            .message
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty());

        // A group ends at any line that is not squash/fixup (drops are
        // transparent: they don't break the group).
        if !matches!(action, "squash" | "fixup" | "drop") {
            if let Some(m) = pending_group_msg.take() {
                end_group(&mut lines, &mut files, &m, group)?;
            }
        }
        let start = lines.len();

        match action {
            "pick" | "edit" | "drop" => {
                lines.push(format!("{action} {}", item.oid));
            }
            "reword" => {
                // `pick` + amend instead of `reword`: git would otherwise
                // open an editor for the message.
                let Some(m) = msg else {
                    return Err(TwigError::InvalidArgument(format!(
                        "reword of {} needs a message",
                        &item.oid[..7]
                    )));
                };
                let (record, mark) = files.record_head();
                lines.push(record);
                lines.push(format!("pick {}", item.oid));
                lines.push(files.guarded_amend(m, HEAD_NOW, &mark)?);
            }
            "squash" | "fixup" => {
                if !saw_commit {
                    return Err(TwigError::InvalidArgument(format!(
                        "cannot {action} commit {} (#{}) without a previous commit",
                        &item.oid[..7],
                        i + 1
                    )));
                }
                lines.push(format!("{action} {}", item.oid));
                if action == "squash" {
                    if let Some(m) = msg {
                        pending_group_msg = Some(m.to_string());
                    }
                }
            }
            other => {
                return Err(TwigError::InvalidArgument(format!(
                    "unknown rebase action '{other}'"
                )))
            }
        }
        if !matches!(action, "squash" | "fixup" | "drop") {
            group = Some((start, lines.len()));
        }
        if action != "drop" {
            saw_commit = true;
        }
    }
    if let Some(m) = pending_group_msg.take() {
        end_group(&mut lines, &mut files, &m, group)?;
    }
    if !saw_commit {
        // An all-drop todo would make git abort with "nothing to do";
        // `noop` makes the intent (drop everything) explicit.
        lines.push("noop".to_string());
    }
    Ok(lines.join("\n") + "\n")
}

/// Run `git rebase -i` with a prepared todo list. Messages for reword and
/// squash are applied with `exec git commit --amend`, so no editor is needed.
pub async fn interactive_rebase(
    repo_path: &Path,
    gitdir: &Path,
    base: Option<&str>,
    items: &[RebaseTodoItem],
    autostash: bool,
) -> Result<GitOutput, TwigError> {
    if let Some(b) = base {
        safe_ref(b)?;
    }
    // Files stay until the next run: later `exec` lines still need them if
    // the rebase stops on a conflict midway.
    let work = gitdir.join("twig-rebase");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let todo = build_todo(items, &work)?;
    let todo_path = work.join("git-rebase-todo");
    std::fs::write(&todo_path, todo)?;

    let seq_editor = format!("cp {}", shell_quote(&shell_path(&todo_path)));
    let mut args = vec![
        "-c",
        "rebase.updateRefs=false",
        "-c",
        "rebase.missingCommitsCheck=ignore",
        "-c",
        "rebase.autoSquash=false",
        "-c",
        "rebase.abbreviateCommands=false",
        "rebase",
        "-i",
    ];
    if autostash {
        args.push("--autostash");
    }
    match base {
        Some(b) => {
            args.push(b);
            args.push("--");
        }
        None => args.push("--root"),
    }
    let out = run_git_noedit(repo_path, &args, &[("GIT_SEQUENCE_EDITOR", &seq_editor)]).await?;
    if out.success && !gitdir.join("rebase-merge").exists() {
        let _ = std::fs::remove_dir_all(&work);
    }
    Ok(out)
}

/// `git push --force-with-lease --force-if-includes <remote> <branch>`.
/// `--force-if-includes` keeps the lease meaningful even after background
/// fetches updated the remote-tracking ref.
pub async fn force_push_with_lease(
    repo_path: &Path,
    remote: &str,
    branch: &str,
    set_upstream: bool,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    safe_ref(branch)?;
    let mut args = vec!["push", "--force-with-lease", "--force-if-includes"];
    if set_upstream {
        args.push("-u");
    }
    args.push(remote);
    args.push(branch);
    let out = run_git(repo_path, &args).await?;
    if !out.success && out.stderr.contains("unknown option") && out.stderr.contains("force-if-includes") {
        // Git older than 2.30.
        let args: Vec<&str> = args.into_iter().filter(|a| *a != "--force-if-includes").collect();
        return run_git(repo_path, &args).await;
    }
    Ok(out)
}

/// Remote configured for `branch` (`branch.<name>.remote`), if any.
pub fn upstream_remote(repo: &Repository, branch: &str) -> Option<String> {
    let cfg = repo.config().ok()?;
    cfg.get_string(&format!("branch.{branch}.remote"))
        .ok()
        .filter(|r| !r.is_empty() && r != ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git_ok(dir: &Path, args: &[&str]) -> String {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout
    }

    async fn repo_with_commits(name: &str, n: usize) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        git_ok(&dir, &["config", "user.name", "t"]).await;
        git_ok(&dir, &["config", "user.email", "t@t"]).await;
        git_ok(&dir, &["config", "commit.gpgsign", "false"]).await;
        for i in 0..n {
            std::fs::write(dir.join(format!("f{i}.txt")), format!("{i}\n")).unwrap();
            git_ok(&dir, &["add", "."]).await;
            git_ok(&dir, &["commit", "-q", "-m", &format!("c{i}")]).await;
        }
        dir
    }

    fn subjects(dir: &Path) -> Vec<String> {
        let repo = Repository::open(dir).unwrap();
        list_rebase_commits(&repo, None)
            .unwrap()
            .commits
            .into_iter()
            .map(|c| c.message)
            .collect()
    }

    #[tokio::test]
    async fn interactive_reorder_reword_squash_drop() {
        let dir = repo_with_commits("irebase", 5).await;
        let repo = Repository::open(&dir).unwrap();
        let list = list_rebase_commits(&repo, Some("HEAD~4")).unwrap();
        assert_eq!(list.commits.len(), 4);
        let c: Vec<String> = list.commits.iter().map(|c| c.oid.clone()).collect();
        // c1..c4 → c3 (reword), c1, c2 squashed into c1 with new message, drop c4
        let items = vec![
            RebaseTodoItem { oid: c[2].clone(), action: "reword".into(), message: Some("third 'quoted'".into()) },
            RebaseTodoItem { oid: c[0].clone(), action: "pick".into(), message: None },
            RebaseTodoItem { oid: c[1].clone(), action: "squash".into(), message: Some("one+two".into()) },
            RebaseTodoItem { oid: c[3].clone(), action: "drop".into(), message: None },
        ];
        let out = interactive_rebase(&dir, repo.path(), Some("HEAD~4"), &items, false)
            .await
            .unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(subjects(&dir), vec!["c0", "third 'quoted'", "one+two"]);
        assert!(!dir.join("f4.txt").exists());
        assert!(dir.join("f1.txt").exists() && dir.join("f2.txt").exists());
        assert!(!repo.path().join("twig-rebase").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn interactive_root_fixup_and_validation() {
        let dir = repo_with_commits("iroot", 3).await;
        let repo = Repository::open(&dir).unwrap();
        let list = list_rebase_commits(&repo, None).unwrap();
        assert_eq!(list.commits.len(), 3);
        let c: Vec<String> = list.commits.iter().map(|c| c.oid.clone()).collect();

        let bad = vec![RebaseTodoItem { oid: c[0].clone(), action: "fixup".into(), message: None }];
        assert!(interactive_rebase(&dir, repo.path(), None, &bad, false).await.is_err());
        let bad = vec![RebaseTodoItem { oid: "HEAD".into(), action: "pick".into(), message: None }];
        assert!(interactive_rebase(&dir, repo.path(), None, &bad, false).await.is_err());

        let items = vec![
            RebaseTodoItem { oid: c[0].clone(), action: "pick".into(), message: None },
            RebaseTodoItem { oid: c[1].clone(), action: "fixup".into(), message: None },
            RebaseTodoItem { oid: c[2].clone(), action: "pick".into(), message: None },
        ];
        let out = interactive_rebase(&dir, repo.path(), None, &items, false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(subjects(&dir), vec!["c0", "c2"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn interactive_stops_on_conflict_then_continues() {
        use crate::git::conflicts::{continue_operation, mark_resolved, read_operation_state};
        let dir = repo_with_commits("iconflict", 1).await;
        for (i, content) in ["one\n", "two\n"].iter().enumerate() {
            std::fs::write(dir.join("same.txt"), content).unwrap();
            git_ok(&dir, &["add", "."]).await;
            git_ok(&dir, &["commit", "-q", "-m", &format!("s{i}")]).await;
        }
        let repo = Repository::open(&dir).unwrap();
        let list = list_rebase_commits(&repo, Some("HEAD~2")).unwrap();
        let c: Vec<String> = list.commits.iter().map(|c| c.oid.clone()).collect();
        // Swap order: s1 first conflicts (same.txt does not exist yet at base
        // for s1's modification), and reword s0 afterwards.
        let items = vec![
            RebaseTodoItem { oid: c[1].clone(), action: "pick".into(), message: None },
            RebaseTodoItem { oid: c[0].clone(), action: "reword".into(), message: Some("renamed s0".into()) },
        ];
        let out = interactive_rebase(&dir, repo.path(), Some("HEAD~2"), &items, false)
            .await
            .unwrap();
        assert!(!out.success);
        let st = read_operation_state(&repo).unwrap();
        assert_eq!(st.kind, "rebase");
        assert!(!st.conflicts.is_empty());

        // Resolve every stop until done (s1, then s0 conflicts again). Each
        // resolution differs from HEAD so no pick ends up empty.
        for round in 0..4 {
            let st = read_operation_state(&repo).unwrap();
            if st.kind == "none" {
                break;
            }
            std::fs::write(dir.join("same.txt"), format!("resolved {round}\n")).unwrap();
            mark_resolved(&dir, &["same.txt"]).await.unwrap();
            let _ = continue_operation(&dir, repo.path(), "rebase", None).await.unwrap();
        }
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(read_operation_state(&repo).unwrap().kind, "none");
        let subs = subjects(&dir);
        assert_eq!(subs, vec!["c0", "s1", "renamed s0"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rebasing onto an upstream that already contains a commit's change makes
    /// that pick empty; git stops, and continuing drops it. The reword must
    /// not then land on the upstream commit before it.
    #[tokio::test]
    async fn empty_pick_does_not_amend_previous_commit() {
        use crate::git::conflicts::{continue_operation, read_operation_state};
        let dir = repo_with_commits("iempty", 1).await;
        git_ok(&dir, &["checkout", "-q", "-b", "feature"]).await;
        let commit = |name: &'static str, file: &'static str, body: &'static str| {
            let dir = dir.clone();
            async move {
                std::fs::write(dir.join(file), body).unwrap();
                git_ok(&dir, &["add", "."]).await;
                git_ok(&dir, &["commit", "-q", "-m", name]).await;
            }
        };
        commit("dup", "dup.txt", "same\n").await;
        commit("head", "h.txt", "h\n").await;
        commit("dup2", "dup2.txt", "same\n").await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        // Same changes as `dup`/`dup2`, different commits (so not filtered
        // as cherry-picks by the todo, which Twig writes itself).
        commit("upstream dup", "dup.txt", "same\n").await;
        commit("upstream dup2", "dup2.txt", "same\n").await;
        git_ok(&dir, &["checkout", "-q", "feature"]).await;

        let repo = Repository::open(&dir).unwrap();
        let list = list_rebase_commits(&repo, Some("main")).unwrap();
        let c: Vec<String> = list.commits.iter().map(|c| c.oid.clone()).collect();
        assert_eq!(c.len(), 3);
        let items = vec![
            RebaseTodoItem { oid: c[0].clone(), action: "reword".into(), message: Some("reworded dup".into()) },
            RebaseTodoItem { oid: c[1].clone(), action: "pick".into(), message: None },
            RebaseTodoItem { oid: c[2].clone(), action: "reword".into(), message: Some("reworded dup2".into()) },
        ];
        let out = interactive_rebase(&dir, repo.path(), Some("main"), &items, false)
            .await
            .unwrap();
        assert!(!out.success, "the empty pick should stop the rebase");
        for _ in 0..4 {
            if read_operation_state(&repo).unwrap().kind == "none" {
                break;
            }
            let _ = continue_operation(&dir, repo.path(), "rebase", None).await.unwrap();
        }
        assert_eq!(read_operation_state(&repo).unwrap().kind, "none");
        // Both dups were dropped; the upstream commits and `head` keep
        // their messages.
        assert_eq!(subjects(&dir), vec!["c0", "upstream dup", "upstream dup2", "head"]);
        assert!(dir.join("h.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The squash group's first commit is dropped as empty, so git melds the
    /// squash into the previous commit. That commit's message must not be
    /// replaced by the group's message.
    #[tokio::test]
    async fn empty_group_head_keeps_previous_message() {
        use crate::git::conflicts::{continue_operation, read_operation_state};
        let dir = repo_with_commits("igroup", 1).await;
        git_ok(&dir, &["checkout", "-q", "-b", "feature"]).await;
        for (name, file) in [("dup", "dup.txt"), ("b", "b.txt")] {
            std::fs::write(dir.join(file), "same\n").unwrap();
            git_ok(&dir, &["add", "."]).await;
            git_ok(&dir, &["commit", "-q", "-m", name]).await;
        }
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        std::fs::write(dir.join("dup.txt"), "same\n").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "upstream dup"]).await;
        git_ok(&dir, &["checkout", "-q", "feature"]).await;

        let repo = Repository::open(&dir).unwrap();
        let c: Vec<String> = list_rebase_commits(&repo, Some("main"))
            .unwrap()
            .commits
            .into_iter()
            .map(|c| c.oid)
            .collect();
        let items = vec![
            RebaseTodoItem { oid: c[0].clone(), action: "pick".into(), message: None },
            RebaseTodoItem { oid: c[1].clone(), action: "squash".into(), message: Some("all".into()) },
        ];
        let out = interactive_rebase(&dir, repo.path(), Some("main"), &items, false)
            .await
            .unwrap();
        assert!(!out.success, "the empty pick should stop the rebase");
        for _ in 0..4 {
            if read_operation_state(&repo).unwrap().kind == "none" {
                break;
            }
            let _ = continue_operation(&dir, repo.path(), "rebase", None).await.unwrap();
        }
        assert_eq!(read_operation_state(&repo).unwrap().kind, "none");
        let subs = subjects(&dir);
        assert_eq!(subs.len(), 2, "{subs:?}");
        assert!(subs[1].starts_with("upstream dup"), "{subs:?}");
        assert!(dir.join("b.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_marks_commits_already_upstream() {
        let dir = repo_with_commits("iupstream", 1).await;
        git_ok(&dir, &["checkout", "-q", "-b", "feature"]).await;
        for (name, file, body) in [("dup", "dup.txt", "same\n"), ("own", "own.txt", "mine\n"), ("picked", "p.txt", "p\n")] {
            std::fs::write(dir.join(file), body).unwrap();
            git_ok(&dir, &["add", "."]).await;
            git_ok(&dir, &["commit", "-q", "-m", name]).await;
        }
        let picked = git_ok(&dir, &["rev-parse", "HEAD"]).await.trim().to_string();
        std::fs::write(dir.join("logo.bin"), b"\0mine").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "binary"]).await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        // A different edit of the same binary file must not match.
        std::fs::write(dir.join("logo.bin"), b"\0theirs").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "upstream binary"]).await;
        // Same change, independent commit; and a real cherry-pick.
        std::fs::write(dir.join("dup.txt"), "same\n").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "upstream dup"]).await;
        git_ok(&dir, &["cherry-pick", &picked]).await;
        git_ok(&dir, &["checkout", "-q", "feature"]).await;

        let repo = Repository::open(&dir).unwrap();
        let list = list_rebase_commits(&repo, Some("main")).unwrap();
        assert!(list.onto_new_base);
        let flags: Vec<(String, bool)> =
            list.commits.iter().map(|c| (c.summary.clone(), c.already_upstream)).collect();
        assert_eq!(
            flags,
            vec![
                ("dup".into(), true),
                ("own".into(), false),
                ("picked".into(), true),
                ("binary".into(), false),
            ]
        );
        // The start-time list skips the scan.
        let quick = list_rebase_commits_opts(&repo, Some("main"), false).unwrap();
        assert!(quick.onto_new_base);
        assert!(quick.commits.iter().all(|c| !c.already_upstream));

        // Base is an ancestor: nothing to compare against.
        let list = list_rebase_commits(&repo, Some("HEAD~3")).unwrap();
        assert!(!list.onto_new_base);
        assert!(list.commits.iter().all(|c| !c.already_upstream));
        let list = list_rebase_commits(&repo, None).unwrap();
        assert!(!list.onto_new_base);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn edit_stops_and_autostash_restores_changes() {
        use crate::git::conflicts::{continue_operation, read_operation_state};
        let dir = repo_with_commits("iedit", 3).await;
        std::fs::write(dir.join("f0.txt"), "dirty\n").unwrap();
        let repo = Repository::open(&dir).unwrap();
        let list = list_rebase_commits(&repo, Some("HEAD~2")).unwrap();
        let c: Vec<String> = list.commits.iter().map(|c| c.oid.clone()).collect();
        let items = vec![
            RebaseTodoItem { oid: c[0].clone(), action: "edit".into(), message: None },
            RebaseTodoItem { oid: c[1].clone(), action: "pick".into(), message: None },
        ];
        // Without autostash git refuses to start on a dirty tree.
        let out = interactive_rebase(&dir, repo.path(), Some("HEAD~2"), &items, false)
            .await
            .unwrap();
        assert!(!out.success);
        assert_eq!(read_operation_state(&repo).unwrap().kind, "none");

        let out = interactive_rebase(&dir, repo.path(), Some("HEAD~2"), &items, true)
            .await
            .unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(read_operation_state(&repo).unwrap().kind, "rebase", "edit should pause");
        let _ = continue_operation(&dir, repo.path(), "rebase", None).await.unwrap();
        assert_eq!(read_operation_state(&repo).unwrap().kind, "none");
        assert_eq!(subjects(&dir), vec!["c0", "c1", "c2"]);
        assert_eq!(std::fs::read_to_string(dir.join("f0.txt")).unwrap(), "dirty\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn todo_guards_message_amends() {
        let dir = std::env::temp_dir().join(format!("twig-todo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let oid = |n: u8| format!("{:040x}", n);
        let items = vec![
            RebaseTodoItem { oid: oid(1), action: "pick".into(), message: None },
            RebaseTodoItem { oid: oid(2), action: "drop".into(), message: None },
            RebaseTodoItem { oid: oid(3), action: "squash".into(), message: Some("all".into()) },
            RebaseTodoItem { oid: oid(4), action: "reword".into(), message: Some("four".into()) },
        ];
        let todo = build_todo(&items, &dir).unwrap();
        let lines: Vec<&str> = todo.lines().collect();
        let mark = |l: &str| l.rsplit("> ").next().unwrap().trim_end_matches(" || :").to_string();
        // before, pick, after, drop, squash, guarded amend, record, pick, guarded amend
        assert_eq!(lines.len(), 9, "{todo}");
        for i in [0, 2, 6] {
            assert!(lines[i].starts_with("exec git rev-parse"), "{todo}");
        }
        assert_eq!(lines[1], format!("pick {}", oid(1)));
        assert_eq!(lines[3], format!("drop {}", oid(2)));
        assert_eq!(lines[4], format!("squash {}", oid(3)));
        assert_eq!(lines[7], format!("pick {}", oid(4)));
        // The group message applies only if its first commit landed (HEAD
        // after it differs from HEAD before it); the reword only if HEAD moved.
        assert!(lines[5].starts_with(&format!("exec test \"$(cat {})\" = \"$(cat {})\"", mark(lines[2]), mark(lines[0]))), "{todo}");
        assert!(lines[8].starts_with(&format!("exec test \"{HEAD_NOW}\" = \"$(cat {})\"", mark(lines[6]))), "{todo}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rebase_onto_branch_and_force_push() {
        let dir = repo_with_commits("onto", 1).await;
        git_ok(&dir, &["checkout", "-q", "-b", "feature"]).await;
        std::fs::write(dir.join("feat.txt"), "f\n").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "feat"]).await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        std::fs::write(dir.join("main.txt"), "m\n").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "main2"]).await;
        git_ok(&dir, &["checkout", "-q", "feature"]).await;

        // Bare remote to push to.
        let remote = dir.with_extension("remote.git");
        let _ = std::fs::remove_dir_all(&remote);
        let remote_s = remote.to_string_lossy().to_string();
        git_ok(&dir, &["init", "-q", "--bare", &remote_s]).await;
        git_ok(&dir, &["remote", "add", "origin", &remote_s]).await;
        git_ok(&dir, &["push", "-q", "-u", "origin", "feature"]).await;

        assert!(rebase_onto(&dir, "-x", false).await.is_err());
        let out = rebase_onto(&dir, "main", false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(subjects(&dir), vec!["c0", "main2", "feat"]);

        // A plain push is rejected (non-fast-forward); lease push succeeds.
        let out = run_git(&dir, &["push", "origin", "feature"]).await.unwrap();
        assert!(!out.success);
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(upstream_remote(&repo, "feature").as_deref(), Some("origin"));
        let out = force_push_with_lease(&dir, "origin", "feature", false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&remote);
    }
}
