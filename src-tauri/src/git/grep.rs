//! Code search with `git grep`, over the working tree's tracked files or a
//! commit. Output is streamed and cut off at a match limit so a search in a
//! huge repository can't flood the UI (or memory).

use std::path::Path;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::error::TwigError;
use crate::git::writer::safe_ref;

/// Default and hard upper bound on returned matches.
const DEFAULT_MAX: usize = 2000;
const HARD_MAX: usize = 20_000;
/// Longest line text sent to the UI (minified files have huge lines).
const MAX_LINE_CHARS: usize = 400;

#[derive(Debug, Clone, Deserialize)]
pub struct GrepOptions {
    pub pattern: String,
    /// Extended regex (`-E`) instead of a fixed string (`-F`).
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub ignore_case: bool,
    #[serde(default)]
    pub whole_word: bool,
    /// Search this commit instead of the working tree.
    #[serde(default)]
    pub rev: Option<String>,
    /// Pathspecs limiting the search (e.g. `src/`, `*.rs`).
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrepMatch {
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrepFile {
    pub path: String,
    pub matches: Vec<GrepMatch>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrepResult {
    pub files: Vec<GrepFile>,
    pub total: usize,
    /// The match limit was hit; more matches exist.
    pub truncated: bool,
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_LINE_CHARS {
        return text.to_string();
    }
    let mut s: String = text.chars().take(MAX_LINE_CHARS).collect();
    s.push('…');
    s
}

/// Parse one `path\0line\0text` record (`rev:` prefix already known).
fn parse_record(rec: &[u8], prefix: &str) -> Option<(String, GrepMatch)> {
    let mut parts = rec.splitn(3, |b| *b == 0);
    let path = String::from_utf8_lossy(parts.next()?).into_owned();
    let line = std::str::from_utf8(parts.next()?).ok()?.parse().ok()?;
    let text = String::from_utf8_lossy(parts.next().unwrap_or_default());
    let path = path.strip_prefix(prefix).map(String::from).unwrap_or(path);
    Some((path, GrepMatch { line, text: truncate(text.trim_end_matches(['\n', '\r'])) }))
}

pub async fn grep(repo_path: &Path, opts: &GrepOptions) -> Result<GrepResult, TwigError> {
    if opts.pattern.is_empty() {
        return Err(TwigError::InvalidArgument("enter something to search for".into()));
    }
    let max = opts.max_results.unwrap_or(DEFAULT_MAX).clamp(1, HARD_MAX);
    let mut args: Vec<&str> = vec![
        "-c",
        "grep.lineNumber=false",
        "-c",
        "grep.fullName=false",
        "grep",
        "-n",
        "-I",
        "--null",
        "--no-color",
        "--full-name",
    ];
    args.push(if opts.regex { "-E" } else { "-F" });
    if opts.ignore_case {
        args.push("-i");
    }
    if opts.whole_word {
        args.push("-w");
    }
    args.push("-e");
    args.push(&opts.pattern);
    let mut prefix = String::new();
    if let Some(rev) = opts.rev.as_deref().filter(|r| !r.is_empty()) {
        safe_ref(rev)?;
        if rev.starts_with('-') {
            return Err(TwigError::InvalidArgument(format!("'{rev}' is not a revision")));
        }
        args.push(rev);
        prefix = format!("{rev}:");
    }
    args.push("--");
    let paths: Vec<&str> = opts.paths.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    args.extend(paths);

    let mut child = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git: {e}")))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let mut reader = BufReader::new(stdout);

    let mut files: Vec<GrepFile> = Vec::new();
    let mut total = 0;
    let mut truncated = false;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf).await? == 0 {
            break;
        }
        if total >= max {
            truncated = true;
            break;
        }
        let Some((path, m)) = parse_record(&buf, &prefix) else { continue };
        total += 1;
        match files.last_mut() {
            Some(f) if f.path == path => f.matches.push(m),
            _ => files.push(GrepFile { path, matches: vec![m] }),
        }
    }
    if truncated {
        // Stop git instead of reading the rest.
        let _ = child.kill().await;
        return Ok(GrepResult { files, total, truncated });
    }
    let mut err = String::new();
    let _ = stderr.read_to_string(&mut err).await;
    let status = child.wait().await?;
    // Exit code 1 means "no matches"; anything else is an error.
    if !status.success() && status.code() != Some(1) {
        let msg = err.trim();
        return Err(TwigError::InvalidArgument(if msg.is_empty() {
            "git grep failed".to_string()
        } else {
            msg.to_string()
        }));
    }
    Ok(GrepResult { files, total, truncated })
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

    async fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-grep-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        git(&dir, &["config", "user.name", "t"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "commit.gpgsign", "false"]).await;
        std::fs::write(dir.join("src/a b.rs"), "fn Main() {}\nlet main = 1;\n// domain\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "main idea\n").unwrap();
        std::fs::write(dir.join("bin.dat"), [b'm', b'a', b'i', b'n', 0, 1]).unwrap();
        git(&dir, &["add", "."]).await;
        git(&dir, &["commit", "-q", "-m", "init"]).await;
        dir
    }

    fn opts(pattern: &str) -> GrepOptions {
        GrepOptions {
            pattern: pattern.into(),
            regex: false,
            ignore_case: false,
            whole_word: false,
            rev: None,
            paths: vec![],
            max_results: None,
        }
    }

    #[tokio::test]
    async fn searches_with_options_paths_and_revisions() {
        let dir = repo("search").await;
        let r = grep(&dir, &opts("main")).await.unwrap();
        // Binary files are skipped; spaces in paths survive.
        let paths: Vec<&str> = r.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["notes.txt", "src/a b.rs"]);
        assert_eq!(r.files[1].matches, [
            GrepMatch { line: 2, text: "let main = 1;".into() },
            GrepMatch { line: 3, text: "// domain".into() },
        ]);
        assert_eq!(r.total, 3);
        assert!(!r.truncated);

        let r = grep(&dir, &GrepOptions { ignore_case: true, whole_word: true, ..opts("main") }).await.unwrap();
        assert_eq!(r.total, 3, "Main, main, main idea; not domain");
        let r = grep(&dir, &GrepOptions { regex: true, ..opts("^fn [A-Z]") }).await.unwrap();
        assert_eq!(r.total, 1);
        let r = grep(&dir, &GrepOptions { paths: vec!["*.txt".into()], ..opts("main") }).await.unwrap();
        assert_eq!(r.files.len(), 1);

        // A commit: the working tree change isn't there, and paths lose the rev prefix.
        std::fs::write(dir.join("notes.txt"), "no match now\n").unwrap();
        let head = git(&dir, &["rev-parse", "HEAD"]).await;
        let r = grep(&dir, &GrepOptions { rev: Some(head.clone()), ..opts("idea") }).await.unwrap();
        assert_eq!(r.files[0].path, "notes.txt");
        assert_eq!(grep(&dir, &opts("idea")).await.unwrap().total, 0);

        // No match is not an error; a bad regex or revision is.
        assert_eq!(grep(&dir, &opts("zzz-nothing")).await.unwrap().total, 0);
        assert!(grep(&dir, &GrepOptions { regex: true, ..opts("(") }).await.is_err());
        assert!(grep(&dir, &GrepOptions { rev: Some("--output=x".into()), ..opts("a") }).await.is_err());
        assert!(grep(&dir, &opts("")).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn stops_at_the_match_limit() {
        let dir = repo("limit").await;
        let big: String = (0..5000).map(|i| format!("needle {i}\n")).collect();
        std::fs::write(dir.join("big.txt"), big).unwrap();
        git(&dir, &["add", "big.txt"]).await;
        let r = grep(&dir, &GrepOptions { max_results: Some(100), ..opts("needle") }).await.unwrap();
        assert_eq!(r.total, 100);
        assert!(r.truncated);
        let long = "x".repeat(1000);
        std::fs::write(dir.join("long.txt"), format!("{long}needle\n")).unwrap();
        git(&dir, &["add", "long.txt"]).await;
        let r = grep(&dir, &GrepOptions { paths: vec!["long.txt".into()], ..opts("needle") }).await.unwrap();
        assert!(r.files[0].matches[0].text.ends_with('…'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
