//! Squash a contiguous run of commits on the current branch into one, driven
//! through the interactive rebase backend (first = pick, rest = squash with
//! the combined message, later commits picked on top).

use std::collections::HashSet;
use std::path::Path;

use git2::{Oid, Repository};
use serde::Serialize;

use crate::error::TwigError;
use crate::git::history::{self, RebaseTodoItem};
use crate::git::writer::GitOutput;

/// How far down HEAD's first-parent line selected commits are looked for.
const MAX_DEPTH: usize = 1000;
/// Commits walked in total when looking for the squashed commits on remotes.
const PUSHED_SCAN_BUDGET: usize = 20_000;

#[derive(Debug, Clone, Serialize)]
pub struct SquashPlan {
    /// Commits squashed together.
    pub count: usize,
    /// Parent of the oldest squashed commit (`None` when it is the root).
    pub base: Option<String>,
    /// Oldest squashed commit (the one the others are melded into).
    pub first: String,
    /// The squashed commits' messages, oldest first, as the default message.
    pub message: String,
    /// Newer commits on the branch that get rebased on top.
    pub later: usize,
    /// Remote-tracking branches that already contain the squashed commits.
    pub pushed_to: Vec<String>,
    /// The remote check stopped early (very large history); `pushed_to`
    /// may be incomplete.
    pub pushed_unknown: bool,
    /// Todo for the rebase, oldest first.
    #[serde(skip)]
    pub items: Vec<RebaseTodoItem>,
}

fn invalid(msg: impl Into<String>) -> TwigError {
    TwigError::InvalidArgument(msg.into())
}

/// Validate `oids` and plan the squash. They must be a contiguous run on
/// HEAD's first-parent line with no merge commits between them and HEAD
/// (the rebase would flatten those). `check_pushed` looks for them on
/// remote-tracking branches (for the dialog's warning).
pub fn plan_squash(repo: &Repository, oids: &[String], check_pushed: bool) -> Result<SquashPlan, TwigError> {
    let selected: HashSet<Oid> = oids
        .iter()
        .map(|s| Oid::from_str(s).map_err(|_| invalid(format!("'{s}' is not a commit id"))))
        .collect::<Result<_, _>>()?;
    if selected.len() < 2 {
        return Err(invalid("select at least two commits to squash"));
    }
    let head = repo
        .head()
        .map_err(|_| invalid("HEAD has no commits"))?
        .peel_to_commit()?;

    // HEAD's first-parent line, newest first, down to the oldest selected.
    let mut line = Vec::new();
    let mut found = 0;
    let mut cur = Some(head);
    while let Some(c) = cur {
        if line.len() >= MAX_DEPTH {
            break;
        }
        if selected.contains(&c.id()) {
            found += 1;
        }
        let parent = if c.parent_count() > 0 { Some(c.parent(0)?) } else { None };
        line.push(c);
        if found == selected.len() {
            break;
        }
        cur = parent;
    }
    if found < selected.len() {
        return Err(invalid(
            "only commits on the current branch (its first-parent history) can be squashed",
        ));
    }
    let positions: Vec<usize> = line
        .iter()
        .enumerate()
        .filter(|(_, c)| selected.contains(&c.id()))
        .map(|(i, _)| i)
        .collect();
    let (newest, oldest) = (positions[0], positions[positions.len() - 1]);
    if oldest - newest + 1 != selected.len() {
        return Err(invalid(
            "the selected commits are not next to each other; select a contiguous run",
        ));
    }
    if let Some(m) = line[..=oldest].iter().find(|c| c.parent_count() > 1) {
        let id = m.id().to_string();
        return Err(invalid(format!(
            "merge commit {} is in the way; squashing would flatten it",
            &id[..7]
        )));
    }

    let first = &line[oldest];
    let message = line[newest..=oldest]
        .iter()
        .rev()
        .map(|c| crate::git::graph::commit_message(c).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut items: Vec<RebaseTodoItem> = line[..=oldest]
        .iter()
        .rev()
        .enumerate()
        .map(|(i, c)| RebaseTodoItem {
            oid: c.id().to_string(),
            action: if i == 0 || !selected.contains(&c.id()) { "pick" } else { "squash" }.to_string(),
            message: None,
        })
        .collect();
    // build_todo applies a squash group's message from its squash lines.
    items[oldest - newest].message = Some(message.clone());

    let base_oid = if first.parent_count() > 0 { Some(first.parent_id(0)?) } else { None };
    let (pushed_to, pushed_unknown) = if check_pushed {
        remotes_containing(repo, first.id(), base_oid)?
    } else {
        (Vec::new(), false)
    };

    Ok(SquashPlan {
        count: selected.len(),
        base: base_oid.map(|o| o.to_string()),
        first: first.id().to_string(),
        message,
        later: newest,
        pushed_to,
        pushed_unknown,
        items,
    })
}

/// Remote-tracking branches whose history contains `commit`. Each walk stops
/// at `base` (the commit's parent), so it only covers what diverged since;
/// a shared step budget bounds the total on huge repos (second value: the
/// budget ran out and the list may be incomplete).
fn remotes_containing(
    repo: &Repository,
    commit: Oid,
    base: Option<Oid>,
) -> Result<(Vec<String>, bool), TwigError> {
    let mut budget = PUSHED_SCAN_BUDGET;
    let mut out = Vec::new();
    for r in repo.references_glob("refs/remotes/*")? {
        let r = r?;
        let (Some(name), Some(tip)) = (r.shorthand().map(String::from), r.target()) else {
            continue;
        };
        if name.ends_with("/HEAD") {
            continue;
        }
        let mut walk = repo.revwalk()?;
        walk.push(tip)?;
        if let Some(b) = base {
            walk.hide(b)?;
        }
        let mut found = false;
        for oid in walk {
            if budget == 0 {
                out.sort();
                return Ok((out, true));
            }
            budget -= 1;
            if oid? == commit {
                found = true;
                break;
            }
        }
        if found {
            out.push(name);
        }
    }
    out.sort();
    Ok((out, false))
}

/// Run the planned squash with `message` as the combined commit message.
pub async fn squash(
    repo_path: &Path,
    gitdir: &Path,
    mut plan: SquashPlan,
    message: &str,
    autostash: bool,
) -> Result<GitOutput, TwigError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(invalid("the squashed commit needs a message"));
    }
    for item in plan.items.iter_mut().filter(|i| i.message.is_some()) {
        item.message = Some(message.to_string());
    }
    history::interactive_rebase(repo_path, gitdir, plan.base.as_deref(), &plan.items, autostash).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::writer::run_git;
    use std::path::PathBuf;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    async fn repo(name: &str, n: usize) -> (PathBuf, Vec<String>) {
        let dir = std::env::temp_dir().join(format!("twig-squash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        git(&dir, &["config", "user.name", "t"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "commit.gpgsign", "false"]).await;
        let mut oids = Vec::new();
        for i in 0..n {
            std::fs::write(dir.join(format!("f{i}.txt")), format!("{i}\n")).unwrap();
            git(&dir, &["add", "."]).await;
            git(&dir, &["commit", "-q", "-m", &format!("c{i}")]).await;
            oids.push(git(&dir, &["rev-parse", "HEAD"]).await);
        }
        (dir, oids)
    }

    async fn subjects(dir: &Path) -> Vec<String> {
        git(dir, &["log", "--format=%s", "--reverse"]).await.lines().map(String::from).collect()
    }

    #[tokio::test]
    async fn squashes_a_middle_run_and_keeps_later_commits() {
        let (dir, c) = repo("middle", 5).await;
        let r = Repository::open(&dir).unwrap();
        // Graph order (newest first) must not matter.
        let plan = plan_squash(&r, &[c[3].clone(), c[1].clone(), c[2].clone()], true).unwrap();
        assert_eq!(plan.count, 3);
        assert_eq!(plan.base.as_deref(), Some(c[0].as_str()));
        assert_eq!(plan.first, c[1]);
        assert_eq!(plan.later, 1);
        assert_eq!(plan.message, "c1\n\nc2\n\nc3");
        assert!(plan.pushed_to.is_empty());

        let out = squash(&dir, r.path(), plan, "one to three", false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(subjects(&dir).await, ["c0", "one to three", "c4"]);
        for i in 0..5 {
            assert!(dir.join(format!("f{i}.txt")).exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn squashes_from_the_root_and_reports_pushed_commits() {
        let (dir, c) = repo("root", 3).await;
        git(&dir, &["update-ref", "refs/remotes/origin/main", &c[1]]).await;
        let r = Repository::open(&dir).unwrap();
        let plan = plan_squash(&r, &[c[0].clone(), c[1].clone()], true).unwrap();
        assert!(!plan.pushed_unknown);
        // A remote ref behind the run doesn't contain it.
        git(&dir, &["update-ref", "refs/remotes/origin/old", &c[0]]).await;
        assert_eq!(plan_squash(&r, &[c[1].clone(), c[2].clone()], true).unwrap().pushed_to, ["origin/main"]);
        assert!(plan_squash(&r, &[c[1].clone(), c[2].clone()], false).unwrap().pushed_to.is_empty());
        assert_eq!(plan.base, None);
        assert_eq!(plan.pushed_to, ["origin/main"]);
        let out = squash(&dir, r.path(), plan, "base", false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(subjects(&dir).await, ["base", "c2"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn refuses_gaps_side_branches_and_merges() {
        let (dir, c) = repo("refuse", 4).await;
        let r = Repository::open(&dir).unwrap();
        let err = |oids: &[&String]| {
            let v: Vec<String> = oids.iter().map(|s| s.to_string()).collect();
            plan_squash(&r, &v, false).unwrap_err().to_string()
        };
        assert!(err(&[&c[3]]).contains("at least two"));
        assert!(err(&[&c[1], &c[3]]).contains("next to each other"));

        git(&dir, &["checkout", "-q", "-b", "side", &c[1]]).await;
        std::fs::write(dir.join("s.txt"), "s\n").unwrap();
        git(&dir, &["add", "."]).await;
        git(&dir, &["commit", "-q", "-m", "side"]).await;
        let side = git(&dir, &["rev-parse", "HEAD"]).await;
        git(&dir, &["checkout", "-q", "main"]).await;
        assert!(err(&[&c[3], &side]).contains("current branch"));

        git(&dir, &["merge", "-q", "--no-ff", "--no-edit", "side"]).await;
        assert!(err(&[&c[2], &c[3]]).contains("merge commit"));
        assert!(plan_squash(&r, &["nope".into(), c[0].clone()], false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
