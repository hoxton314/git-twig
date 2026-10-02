//! History rewriting: rebase onto, non-interactive driving of interactive
//! rebase, and force push with lease.
use std::path::Path;

use git2::{Repository, Sort};
use serde::{Deserialize, Serialize};

use crate::error::TwigError;
use crate::git::conflicts::{run_git_noedit, shell_path, shell_quote};
use crate::git::writer::{run_git, safe_ref, GitOutput};

/// Upper bound on commits shown in the interactive rebase editor.
const MAX_REBASE_COMMITS: usize = 1000;

#[derive(Debug, Clone, Serialize)]
pub struct RebaseCommit {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub message: String,
    pub author_name: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RebaseCommitList {
    /// Oldest first (todo order).
    pub commits: Vec<RebaseCommit>,
    /// Merge commits in the range; git drops them during a plain `rebase -i`.
    pub merges_skipped: u32,
    /// Full OID of the resolved base (`None` when rebasing from the root).
    pub base_oid: Option<String>,
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

/// List the commits `git rebase -i <base>` would offer (base..HEAD).
pub fn list_rebase_commits(
    repo: &Repository,
    base: Option<&str>,
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
        let id = commit.id().to_string();
        commits.push(RebaseCommit {
            short_oid: id.chars().take(7).collect(),
            oid: id,
            summary: commit.summary().unwrap_or("").to_string(),
            message: commit.message().unwrap_or("").trim_end().to_string(),
            author_name: commit.author().name().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
        });
    }

    Ok(RebaseCommitList {
        commits,
        merges_skipped,
        base_oid,
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

    /// `exec` line that replaces HEAD's message with `msg`, but only if HEAD
    /// moved since `mark` was recorded. A pick that ends up empty is dropped
    /// (or skipped by the user), and amending then would rewrite the
    /// previous, unrelated commit.
    fn guarded_amend(&mut self, msg: &str, mark: &str) -> Result<String, TwigError> {
        let file = self.path("msg");
        std::fs::write(&file, format!("{}\n", msg.trim()))?;
        Ok(format!(
            "exec test \"$(git rev-parse -q --verify HEAD)\" = \"$(cat {mark})\" || \
             git commit --amend --allow-empty --no-verify --cleanup=whitespace -F {}",
            shell_quote(&shell_path(&file))
        ))
    }
}

/// Close a squash group that has a replacement message: record HEAD before
/// the group's first commit (at `start`) and amend after its last line.
fn end_group(
    lines: &mut Vec<String>,
    files: &mut TodoFiles,
    msg: &str,
    start: Option<usize>,
) -> Result<(), TwigError> {
    let start = start.ok_or_else(|| {
        TwigError::InvalidArgument("squash without a commit to squash into".to_string())
    })?;
    let (record, mark) = files.record_head();
    lines.insert(start, record);
    lines.push(files.guarded_amend(msg, &mark)?);
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
    // Index in `lines` where the current group's first commit starts.
    let mut group_start: Option<usize> = None;

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
                end_group(&mut lines, &mut files, &m, group_start)?;
            }
            group_start = Some(lines.len());
        }

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
                lines.push(files.guarded_amend(m, &mark)?);
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
        if action != "drop" {
            saw_commit = true;
        }
    }
    if let Some(m) = pending_group_msg.take() {
        end_group(&mut lines, &mut files, &m, group_start)?;
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
        let kinds: Vec<&str> = lines.iter().map(|l| l.split(' ').nth(1).unwrap_or("")).collect();
        // record, pick, drop, squash, guarded amend, record, pick, guarded amend
        assert_eq!(lines.len(), 8, "{todo}");
        assert!(lines[0].starts_with("exec git rev-parse"));
        assert_eq!(&lines[1..4], &[
            format!("pick {}", oid(1)).as_str(),
            format!("drop {}", oid(2)).as_str(),
            format!("squash {}", oid(3)).as_str(),
        ]);
        assert_eq!(kinds[4], "test");
        assert!(lines[5].starts_with("exec git rev-parse"));
        assert_eq!(lines[6], format!("pick {}", oid(4)));
        assert_eq!(kinds[7], "test");
        // Each guard compares against its own mark file.
        let mark = |l: &str| l.rsplit("> ").next().unwrap().trim_end_matches(" || :").to_string();
        assert!(lines[4].contains(&mark(lines[0])));
        assert!(lines[7].contains(&mark(lines[5])));
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
