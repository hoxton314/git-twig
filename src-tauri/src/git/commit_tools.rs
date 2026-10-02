//! Commit helpers for the staging panel: amend / sign-off commits, HEAD
//! inspection for the amend toggle, recent authors for co-author trailers and
//! the `commit.template` file.
//!
//! Reads use git2; the commit itself goes through the git CLI so hooks,
//! signing and `commit.*` config behave exactly as on the command line.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use git2::{BranchType, Oid, Repository};
use serde::Serialize;

use crate::error::TwigError;
use crate::git::writer::{run_git, GitOutput};

/// Options for [`commit_with_options`].
#[derive(Debug, Default, Clone, Copy)]
pub struct CommitOptions {
    /// Replace HEAD (`--amend`). Allowed with nothing staged (message-only).
    pub amend: bool,
    /// Append a `Signed-off-by` trailer for the committer.
    pub signoff: bool,
}

/// Whether `line` looks like a git trailer (`Token: value`).
fn is_trailer_line(line: &str) -> bool {
    match line.split_once(": ") {
        Some((token, _)) => {
            !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        }
        None => false,
    }
}

/// Append `trailer` to `message` unless already present: into the trailing
/// trailer block when there is one, otherwise as a new paragraph.
///
/// Done here rather than with `git commit --signoff` because git treats lines
/// starting with `#` (e.g. "#42 fixed") as comments when locating the trailer
/// block and would insert the sign-off above them.
pub fn append_trailer(message: &str, trailer: &str) -> String {
    let body = message.trim_end();
    if body.lines().any(|l| l.trim_end() == trailer) {
        return body.to_string();
    }
    let last_para = body.rsplit("\n\n").next().unwrap_or("");
    let has_block = body.contains("\n\n")
        && !last_para.trim().is_empty()
        && last_para.lines().all(is_trailer_line);
    if has_block {
        format!("{body}\n{trailer}")
    } else {
        format!("{body}\n\n{trailer}")
    }
}

/// `Signed-off-by: Name <email>` for the committer identity git would use.
async fn signoff_trailer(repo_path: &Path) -> Result<String, TwigError> {
    let out = run_git(repo_path, &["var", "GIT_COMMITTER_IDENT"]).await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    // Format: "Name <email> 1700000000 +0000"
    let ident = out.stdout.trim();
    let end = ident.rfind('>').ok_or_else(|| {
        TwigError::GitCli(format!("unexpected committer identity '{ident}'"))
    })?;
    Ok(format!("Signed-off-by: {}", &ident[..=end]))
}

/// Create (or amend) a commit with the given message.
pub async fn commit_with_options(
    repo_path: &Path,
    message: &str,
    opts: CommitOptions,
) -> Result<GitOutput, TwigError> {
    if message.trim().is_empty() {
        return Err(TwigError::InvalidArgument(
            "commit message must not be empty".to_string(),
        ));
    }
    let message = if opts.signoff {
        append_trailer(message, &signoff_trailer(repo_path).await?)
    } else {
        message.to_string()
    };
    // `--cleanup=whitespace` keeps lines starting with `#` (issue references)
    // while still trimming trailing blank lines, matching plain `-m`.
    let mut args = vec!["commit", "--cleanup=whitespace"];
    if opts.amend {
        args.push("--amend");
    }
    args.push("-m");
    args.push(&message);
    run_git(repo_path, &args).await
}

/// Information about HEAD used when toggling "Amend".
#[derive(Debug, Serialize)]
pub struct HeadCommitInfo {
    pub oid: String,
    /// Full commit message (summary + body).
    pub message: String,
    /// HEAD is reachable from its upstream (or, without an upstream, from any
    /// remote-tracking branch), so amending rewrites published history.
    pub pushed: bool,
    /// Name of the remote branch that already contains HEAD, if any.
    pub pushed_to: Option<String>,
}

fn contains(repo: &Repository, tip: Oid, commit: Oid) -> bool {
    tip == commit || repo.graph_descendant_of(tip, commit).unwrap_or(false)
}

pub fn read_head_commit(repo: &Repository) -> Result<HeadCommitInfo, TwigError> {
    let head = repo
        .head()
        .map_err(|_| TwigError::InvalidArgument("there is no commit to amend yet".to_string()))?;
    let commit = head.peel_to_commit()?;
    let oid = commit.id();
    let message = String::from_utf8_lossy(commit.message_bytes())
        .trim_end()
        .to_string();

    let mut pushed_to: Option<String> = None;

    // Prefer the branch's configured upstream.
    if head.is_branch() {
        if let Some(name) = head.shorthand() {
            if let Ok(branch) = repo.find_branch(name, BranchType::Local) {
                if let Ok(upstream) = branch.upstream() {
                    if let (Some(target), Ok(Some(up_name))) =
                        (upstream.get().target(), upstream.name())
                    {
                        if contains(repo, target, oid) {
                            pushed_to = Some(up_name.to_string());
                        }
                    }
                }
            }
        }
    }

    // Without an upstream match, any remote-tracking branch containing HEAD
    // still means the commit is published.
    if pushed_to.is_none() {
        for branch in repo.branches(Some(BranchType::Remote))? {
            let (branch, _) = branch?;
            let Some(target) = branch.get().target() else { continue };
            if contains(repo, target, oid) {
                pushed_to = branch.name().ok().flatten().map(str::to_string);
                break;
            }
        }
    }

    Ok(HeadCommitInfo {
        oid: oid.to_string(),
        message,
        pushed: pushed_to.is_some(),
        pushed_to,
    })
}

/// A commit author seen in recent history, for the co-author picker.
#[derive(Debug, Serialize, PartialEq)]
pub struct AuthorInfo {
    pub name: String,
    pub email: String,
    /// Number of commits among the scanned ones.
    pub count: u32,
}

/// Distinct authors of the last `max_commits` commits reachable from HEAD,
/// most frequent first. The configured `user.email` is excluded since you
/// cannot be your own co-author.
pub fn read_recent_authors(
    repo: &Repository,
    max_commits: usize,
) -> Result<Vec<AuthorInfo>, TwigError> {
    let mut walk = repo.revwalk()?;
    if walk.push_head().is_err() {
        // Unborn branch: no history yet.
        return Ok(Vec::new());
    }
    walk.set_sorting(git2::Sort::TIME)?;

    let me = repo
        .config()
        .ok()
        .and_then(|c| c.get_string("user.email").ok())
        .map(|e| e.to_lowercase());

    // email (lowercased) -> (index in `authors`)
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut authors: Vec<AuthorInfo> = Vec::new();
    for oid in walk.take(max_commits) {
        let commit = repo.find_commit(oid?)?;
        let sig = commit.author();
        let (Some(name), Some(email)) = (sig.name(), sig.email()) else { continue };
        let key = email.to_lowercase();
        if key.is_empty() || me.as_deref() == Some(key.as_str()) {
            continue;
        }
        match seen.get(&key) {
            Some(&i) => authors[i].count += 1,
            None => {
                seen.insert(key, authors.len());
                authors.push(AuthorInfo {
                    name: name.to_string(),
                    email: email.to_string(),
                    count: 1,
                });
            }
        }
    }
    authors.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    Ok(authors)
}

/// Expand a leading `~/` to the home directory, like git does for paths.
fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        if let Some(home) = home {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

/// Contents of the `commit.template` file, if configured and readable.
/// Comment lines (`#...`) are removed, since git would strip them from an
/// editor-composed message but we commit with `-m`.
pub fn read_commit_template(repo: &Repository) -> Result<Option<String>, TwigError> {
    let config = repo.config()?;
    let Ok(raw) = config.get_string("commit.template") else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let mut path = expand_home(raw);
    if path.is_relative() {
        // git resolves a relative template path against the working tree
        // root (the directory commands run from).
        if let Some(workdir) = repo.workdir() {
            path = workdir.join(path);
        }
    }
    let content = std::fs::read_to_string(&path).map_err(|e| {
        TwigError::Config(format!(
            "cannot read commit.template '{}': {e}",
            path.display()
        ))
    })?;
    let cleaned: Vec<&str> = content.lines().filter(|l| !l.starts_with('#')).collect();
    let text = cleaned.join("\n").trim_end().to_string();
    Ok(if text.trim().is_empty() { None } else { Some(text) })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn git_ok(dir: &Path, args: &[&str]) {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-commit-tools-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        // Local config overrides any global signing/identity setup.
        git_ok(&dir, &["config", "user.name", "Me"]).await;
        git_ok(&dir, &["config", "user.email", "me@example.com"]).await;
        git_ok(&dir, &["config", "commit.gpgsign", "false"]).await;
        git_ok(&dir, &["config", "core.hooksPath", "/dev/null"]).await;
        dir
    }

    #[test]
    fn trailers() {
        assert_eq!(append_trailer("fix", "A: b"), "fix\n\nA: b");
        assert_eq!(append_trailer("fix\n\nbody text\n", "A: b"), "fix\n\nbody text\n\nA: b");
        assert_eq!(append_trailer("fix\n\nCo-authored-by: X <x@y>", "A: b"), "fix\n\nCo-authored-by: X <x@y>\nA: b");
        assert_eq!(append_trailer("fix\n\nA: b\n", "A: b"), "fix\n\nA: b");
        // A single-line summary that looks like a trailer is not a block.
        assert_eq!(append_trailer("docs: update", "A: b"), "docs: update\n\nA: b");
    }

    #[tokio::test]
    async fn amend_message_only_and_signoff() {
        let dir = temp_repo("amend").await;
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        git_ok(&dir, &["add", "--", "a.txt"]).await;

        let opts = CommitOptions { amend: false, signoff: true };
        let out = commit_with_options(&dir, "first\n\n#42 keeps hash lines", opts).await.unwrap();
        assert!(out.success, "{}", out.stderr);

        let repo = Repository::open(&dir).unwrap();
        let head = read_head_commit(&repo).unwrap();
        assert!(head.message.starts_with("first\n\n#42 keeps hash lines"), "{:?}", head.message);
        assert!(head.message.ends_with("hash lines\n\nSigned-off-by: Me <me@example.com>"), "{:?}", head.message);
        assert!(!head.pushed);

        // Nothing staged: amend still rewrites the message.
        let opts = CommitOptions { amend: true, signoff: false };
        let out = commit_with_options(&dir, "reworded", opts).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let head2 = read_head_commit(&repo).unwrap();
        assert_eq!(head2.message, "reworded");
        assert_ne!(head.oid, head2.oid);
        // Still a single (root) commit.
        assert!(!crate::git::writer::rev_exists(&dir, "HEAD~1").await.unwrap());

        assert!(commit_with_options(&dir, "  ", opts).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn pushed_detection_and_authors() {
        let dir = temp_repo("pushed").await;
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        git_ok(&dir, &["add", "--", "a.txt"]).await;
        git_ok(&dir, &["-c", "user.name=Alice", "-c", "user.email=alice@x", "commit", "-q", "-m", "c1"]).await;
        std::fs::write(dir.join("a.txt"), "b\n").unwrap();
        git_ok(&dir, &["-c", "user.name=Bob", "-c", "user.email=bob@x", "commit", "-q", "-am", "c2"]).await;
        std::fs::write(dir.join("a.txt"), "c\n").unwrap();
        git_ok(&dir, &["-c", "user.name=Bob", "-c", "user.email=bob@x", "commit", "-q", "-am", "c3"]).await;
        std::fs::write(dir.join("a.txt"), "d\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "mine"]).await;

        let repo = Repository::open(&dir).unwrap();
        let authors = read_recent_authors(&repo, 100).unwrap();
        assert_eq!(authors.len(), 2, "{authors:?}");
        assert_eq!(authors[0].email, "bob@x");
        assert_eq!(authors[0].count, 2);
        assert_eq!(authors[1].email, "alice@x");

        // Simulate a pushed HEAD with a remote-tracking ref.
        assert!(!read_head_commit(&repo).unwrap().pushed);
        git_ok(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]).await;
        let info = read_head_commit(&repo).unwrap();
        assert!(info.pushed);
        assert_eq!(info.pushed_to.as_deref(), Some("origin/main"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn commit_template_strips_comments() {
        let dir = temp_repo("template").await;
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(read_commit_template(&repo).unwrap(), None);
        std::fs::write(dir.join(".gitmessage"), "# comment\nSummary\n\nBody\n# trailing\n").unwrap();
        git_ok(&dir, &["config", "commit.template", ".gitmessage"]).await;
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(read_commit_template(&repo).unwrap().as_deref(), Some("Summary\n\nBody"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
