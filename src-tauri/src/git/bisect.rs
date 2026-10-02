//! `git bisect`: start (with either or both ends), mark commits, reset, and
//! read the progress (marks, remaining candidates, the first bad commit).
//!
//! State is git's own (`BISECT_*` files, `refs/bisect/*`), so a bisect
//! started on the command line shows up here too. Custom terms
//! (`--term-old/--term-new`) are honoured.

use std::path::Path;

use serde::Serialize;

use crate::error::TwigError;
use crate::git::writer::{run_git, safe_ref, GitOutput};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BisectInfo {
    /// Name used for "bad" (default `bad`, or e.g. `new`).
    pub term_bad: String,
    /// Name used for "good" (default `good`, or e.g. `old`).
    pub term_good: String,
    /// The bad end (newest known bad commit).
    pub bad: Option<String>,
    pub good: Vec<String>,
    pub skipped: Vec<String>,
    /// HEAD: the commit being tested.
    pub current: Option<String>,
    pub current_subject: Option<String>,
    /// Candidates left (including the bad end), once both ends are known.
    pub remaining: Option<u32>,
    /// git's estimate of the steps left.
    pub steps: Option<u32>,
    /// Set once bisect has narrowed it down to one commit.
    pub first_bad: Option<String>,
    pub first_bad_subject: Option<String>,
}

fn invalid(msg: impl Into<String>) -> TwigError {
    TwigError::InvalidArgument(msg.into())
}

async fn git_path(repo_path: &Path, name: &str) -> Result<std::path::PathBuf, TwigError> {
    let out = run_git(repo_path, &["rev-parse", "--git-path", name]).await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    Ok(repo_path.join(out.stdout.trim()))
}

async fn subject(repo_path: &Path, oid: &str) -> Option<String> {
    let out = run_git(repo_path, &["log", "-1", "--format=%s", oid, "--"]).await.ok()?;
    out.success.then(|| out.stdout.trim().to_string())
}

/// Parse `git rev-list --bisect-vars` output into (all, steps).
fn parse_vars(out: &str) -> (Option<u32>, Option<u32>) {
    let mut all = None;
    let mut steps = None;
    for line in out.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim_matches('\'').parse().ok();
            match k {
                "bisect_all" => all = v,
                "bisect_steps" => steps = v,
                _ => {}
            }
        }
    }
    (all, steps)
}

/// Split git's shell-quoted argument list (`'a' 'b c' 'it'\''s'`).
fn parse_sq_quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut any = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match (in_quote, c) {
            (true, '\'') => in_quote = false,
            (true, c) => cur.push(c),
            (false, '\'') => {
                in_quote = true;
                any = true;
            }
            (false, '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                    any = true;
                }
            }
            (false, c) if c.is_whitespace() => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (false, c) => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

/// The first bad commit git recorded in `BISECT_LOG` once it finished
/// (`# first bad commit: [<oid>] …`), unless marks were made after it.
fn first_bad_from_log(log: &str) -> Option<String> {
    let mut found = None;
    for line in log.lines() {
        if let Some(rest) = line.strip_prefix("# first ") {
            if let Some((_, after)) = rest.split_once(" commit: [") {
                found = after.split_once(']').map(|(oid, _)| oid.to_string());
            }
        } else if line.starts_with("git bisect ") {
            found = None;
        }
    }
    found
}

/// The bisect in progress, or `None` when there is none.
pub async fn bisect_info(repo_path: &Path) -> Result<Option<BisectInfo>, TwigError> {
    if !git_path(repo_path, "BISECT_START").await?.exists() {
        return Ok(None);
    }
    let terms = std::fs::read_to_string(git_path(repo_path, "BISECT_TERMS").await?).unwrap_or_default();
    let mut lines = terms.lines().map(str::trim).filter(|l| !l.is_empty());
    let term_bad = lines.next().unwrap_or("bad").to_string();
    let term_good = lines.next().unwrap_or("good").to_string();

    let refs = run_git(repo_path, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/bisect/"]).await?;
    let (mut bad, mut good, mut skipped) = (None, Vec::new(), Vec::new());
    let bad_ref = format!("refs/bisect/{term_bad}");
    let good_prefix = format!("refs/bisect/{term_good}-");
    for line in refs.stdout.lines() {
        let Some((name, oid)) = line.split_once(' ') else { continue };
        if name == bad_ref {
            bad = Some(oid.to_string());
        } else if name.starts_with(&good_prefix) {
            good.push(oid.to_string());
        } else if name.starts_with("refs/bisect/skip-") {
            skipped.push(oid.to_string());
        }
    }

    let head = run_git(repo_path, &["rev-parse", "--verify", "--quiet", "HEAD"]).await?;
    let current = head.success.then(|| head.stdout.trim().to_string()).filter(|s| !s.is_empty());
    let current_subject = match &current {
        Some(c) => subject(repo_path, c).await,
        None => None,
    };

    // Paths the bisect is limited to (`git bisect start … -- <paths>`).
    let names = std::fs::read_to_string(git_path(repo_path, "BISECT_NAMES").await?).unwrap_or_default();
    let paths: Vec<String> = parse_sq_quoted(&names).into_iter().filter(|p| p != "--").collect();
    let log = std::fs::read_to_string(git_path(repo_path, "BISECT_LOG").await?).unwrap_or_default();

    let (mut remaining, mut steps, mut first_bad) = (None, None, first_bad_from_log(&log));
    if let (Some(b), false) = (&bad, good.is_empty()) {
        let mut args = vec!["rev-list", "--bisect-vars", b.as_str(), "--not"];
        args.extend(good.iter().map(String::as_str));
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
        let out = run_git(repo_path, &args).await?;
        if out.success {
            let (all, s) = parse_vars(&out.stdout);
            remaining = all;
            steps = s;
            if all == Some(1) && first_bad.is_none() {
                first_bad = Some(b.clone());
            }
        }
    }
    let first_bad_subject = match &first_bad {
        Some(f) => subject(repo_path, f).await,
        None => None,
    };

    Ok(Some(BisectInfo {
        term_bad,
        term_good,
        bad,
        good,
        skipped,
        current,
        current_subject,
        remaining,
        steps,
        first_bad,
        first_bad_subject,
    }))
}

fn check_rev(rev: &str) -> Result<(), TwigError> {
    safe_ref(rev)?;
    if rev.starts_with('-') {
        return Err(invalid(format!("'{rev}' is not a commit")));
    }
    Ok(())
}

/// Start bisecting. Either end may be left out and marked later.
pub async fn start(repo_path: &Path, bad: Option<&str>, good: Option<&str>) -> Result<GitOutput, TwigError> {
    if git_path(repo_path, "BISECT_START").await?.exists() {
        return Err(invalid("a bisect is already in progress; reset it first"));
    }
    let mut args = vec!["bisect", "start"];
    match (bad, good) {
        (Some(b), Some(g)) => {
            check_rev(b)?;
            check_rev(g)?;
            args.extend_from_slice(&[b, g, "--"]);
        }
        (Some(b), None) => {
            check_rev(b)?;
            args.extend_from_slice(&[b, "--"]);
        }
        (None, Some(g)) => {
            check_rev(g)?;
            let out = run_git(repo_path, &["bisect", "start"]).await?;
            if !out.success {
                return Ok(out);
            }
            return run_git(repo_path, &["bisect", "good", g]).await;
        }
        (None, None) => {}
    }
    run_git(repo_path, &args).await
}

/// Mark `rev` (default: HEAD) as `verdict`: good, bad or skip. Good and bad
/// map to the bisect's terms.
pub async fn mark(repo_path: &Path, verdict: &str, rev: Option<&str>) -> Result<GitOutput, TwigError> {
    let info = bisect_info(repo_path).await?.ok_or_else(|| invalid("no bisect is in progress"))?;
    let term = match verdict {
        "good" => info.term_good,
        "bad" => info.term_bad,
        "skip" => "skip".to_string(),
        other => return Err(invalid(format!("unknown bisect verdict '{other}'"))),
    };
    let mut args = vec!["bisect", term.as_str()];
    if let Some(r) = rev {
        check_rev(r)?;
        args.push(r);
    }
    run_git(repo_path, &args).await
}

/// End the bisect and return to where it started.
pub async fn reset(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["bisect", "reset"]).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    /// c0..c7 where the bug appears in c5.
    async fn repo(name: &str) -> (PathBuf, Vec<String>) {
        let dir = std::env::temp_dir().join(format!("twig-bisect-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        git(&dir, &["config", "user.name", "t"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "commit.gpgsign", "false"]).await;
        let mut oids = Vec::new();
        for i in 0..8 {
            let body = if i >= 5 { format!("{i}\nbug\n") } else { format!("{i}\n") };
            std::fs::write(dir.join("f.txt"), body).unwrap();
            git(&dir, &["add", "."]).await;
            git(&dir, &["commit", "-q", "-m", &format!("c{i}")]).await;
            oids.push(git(&dir, &["rev-parse", "HEAD"]).await);
        }
        (dir, oids)
    }

    fn has_bug(dir: &Path) -> bool {
        std::fs::read_to_string(dir.join("f.txt")).unwrap().contains("bug")
    }

    #[tokio::test]
    async fn finds_the_first_bad_commit() {
        let (dir, c) = repo("find").await;
        assert_eq!(bisect_info(&dir).await.unwrap(), None);

        let out = start(&dir, Some(&c[7]), Some(&c[0])).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(start(&dir, Some(&c[7]), None).await.is_err(), "already bisecting");
        let info = bisect_info(&dir).await.unwrap().unwrap();
        assert_eq!(info.bad.as_deref(), Some(c[7].as_str()));
        assert_eq!(info.good, [c[0].clone()]);
        assert_eq!(info.remaining, Some(7));
        assert!(info.steps.is_some() && info.first_bad.is_none());

        let mut rounds = 0;
        loop {
            let info = bisect_info(&dir).await.unwrap().unwrap();
            if let Some(first) = info.first_bad {
                assert_eq!(first, c[5]);
                assert_eq!(info.first_bad_subject.as_deref(), Some("c5"));
                break;
            }
            rounds += 1;
            assert!(rounds < 6, "bisect did not converge");
            let verdict = if has_bug(&dir) { "bad" } else { "good" };
            let out = mark(&dir, verdict, None).await.unwrap();
            assert!(out.success, "{}", out.stderr);
        }

        let out = reset(&dir).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(bisect_info(&dir).await.unwrap(), None);
        assert_eq!(git(&dir, &["rev-parse", "HEAD"]).await, c[7]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn starts_from_one_end_skips_and_custom_terms() {
        let (dir, c) = repo("ends").await;
        // Only the bad end: waiting for a good one.
        start(&dir, Some(&c[7]), None).await.unwrap();
        let info = bisect_info(&dir).await.unwrap().unwrap();
        assert_eq!(info.bad.as_deref(), Some(c[7].as_str()));
        assert!(info.good.is_empty() && info.remaining.is_none());
        let out = mark(&dir, "good", Some(&c[2])).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let out = mark(&dir, "skip", None).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let info = bisect_info(&dir).await.unwrap().unwrap();
        assert_eq!(info.skipped.len(), 1);
        assert!(mark(&dir, "maybe", None).await.is_err());
        assert!(mark(&dir, "good", Some("--help")).await.is_err());
        reset(&dir).await.unwrap();

        // Only the good end.
        start(&dir, None, Some(&c[0])).await.unwrap();
        let info = bisect_info(&dir).await.unwrap().unwrap();
        assert_eq!(info.good, [c[0].clone()]);
        assert!(info.bad.is_none());
        reset(&dir).await.unwrap();

        // Started on the command line with other terms.
        git(&dir, &["bisect", "start", "--term-old=fast", "--term-new=slow", &c[7], &c[0]]).await;
        let info = bisect_info(&dir).await.unwrap().unwrap();
        assert_eq!((info.term_bad.as_str(), info.term_good.as_str()), ("slow", "fast"));
        assert_eq!(info.bad.as_deref(), Some(c[7].as_str()));
        let out = mark(&dir, "good", None).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(bisect_info(&dir).await.unwrap().unwrap().good.len(), 2);
        reset(&dir).await.unwrap();
        assert!(mark(&dir, "good", None).await.is_err(), "nothing to mark");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn path_limited_bisect_finishes() {
        let dir = std::env::temp_dir().join(format!("twig-bisect-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        git(&dir, &["config", "user.name", "t"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "commit.gpgsign", "false"]).await;
        // A commit that doesn't touch f.txt sits right before the first bad
        // one, so it still counts as a candidate without the path limit.
        let steps: [(&str, &str); 6] =
            [("f.txt", "0\n"), ("f.txt", "1\n"), ("f.txt", "2\n"), ("other.txt", "x\n"), ("f.txt", "bug\n"), ("f.txt", "bug 2\n")];
        let mut oids = Vec::new();
        for (i, (file, body)) in steps.iter().enumerate() {
            std::fs::write(dir.join(file), body).unwrap();
            git(&dir, &["add", "."]).await;
            git(&dir, &["commit", "-q", "-m", &format!("s{i}")]).await;
            oids.push(git(&dir, &["rev-parse", "HEAD"]).await);
        }
        git(&dir, &["bisect", "start", &oids[5], &oids[0], "--", "f.txt"]).await;
        for _ in 0..6 {
            let info = bisect_info(&dir).await.unwrap().unwrap();
            if let Some(first) = info.first_bad {
                assert_eq!(first, oids[4]);
                reset(&dir).await.unwrap();
                let _ = std::fs::remove_dir_all(&dir);
                return;
            }
            let verdict = if has_bug(&dir) { "bad" } else { "good" };
            assert!(mark(&dir, verdict, None).await.unwrap().success);
        }
        panic!("path-limited bisect never reported the first bad commit");
    }

    #[test]
    fn parses_names_and_log() {
        assert_eq!(parse_sq_quoted(" '--' 'a' 'd x/b'\\''q'"), ["--", "a", "d x/b'q"]);
        assert!(parse_sq_quoted("").is_empty());
        let log = "git bisect start 'x'\n# bad: [aaa] c\ngit bisect bad aaa\n# first 'bad' commit: [abc123] c1\n";
        assert_eq!(first_bad_from_log(log).as_deref(), Some("abc123"));
        assert_eq!(first_bad_from_log("# first bad commit: [def] x\n").as_deref(), Some("def"));
        // Marked again afterwards: no longer the result.
        assert_eq!(first_bad_from_log(&format!("{log}git bisect good abc123\n")), None);
    }

    #[test]
    fn parses_bisect_vars() {
        let out = "bisect_rev='abc'\nbisect_nr=3\nbisect_all=7\nbisect_steps=2\n";
        assert_eq!(parse_vars(out), (Some(7), Some(2)));
        assert_eq!(parse_vars(""), (None, None));
    }
}
