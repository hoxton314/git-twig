//! Per-file actions for the staging panel's context menu: open in the default
//! app, reveal in the file manager, add to `.gitignore`, and launch an
//! external diff tool.
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

use crate::error::TwigError;
use crate::git::writer::{run_git, run_git_paths};

/// Resolve a repo-relative path (as reported by `git status`) to an absolute
/// path inside the working tree. Rejects absolute paths and `..` so a crafted
/// argument cannot address files outside the repository.
pub fn resolve_in_repo(repo_path: &Path, rel: &str) -> Result<PathBuf, TwigError> {
    let trimmed = rel.trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(repo_path.to_path_buf());
    }
    let rel_path = Path::new(trimmed);
    for comp in rel_path.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(TwigError::InvalidArgument(format!(
                    "path '{rel}' must be relative to the repository"
                )))
            }
        }
    }
    Ok(repo_path.join(rel_path))
}

/// Spawn a GUI helper without waiting for it; a thread reaps it on exit so
/// no zombie process is left behind.
fn spawn_detached(mut cmd: StdCommand) -> Result<(), TwigError> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(TwigError::Io)?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Open a file (or directory) with the system's default application.
pub fn open_with_default_app(target: &Path) -> Result<(), TwigError> {
    if !target.exists() {
        return Err(TwigError::InvalidArgument(format!(
            "'{}' does not exist in the working tree",
            target.display()
        )));
    }
    #[cfg(target_os = "macos")]
    let cmd = {
        let mut c = StdCommand::new("open");
        c.arg("--").arg(target);
        c
    };
    #[cfg(target_os = "windows")]
    let cmd = {
        // `explorer <file>` opens it with its associated program without
        // going through `cmd /c start`, which would re-parse the path.
        let mut c = StdCommand::new("explorer");
        c.arg(target);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let cmd = {
        let mut c = StdCommand::new("xdg-open");
        c.arg(target);
        c
    };
    spawn_detached(cmd)
}

/// Percent-encode a filesystem path into a `file://` URI.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut out = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Show a file selected in the platform file manager. Falls back to opening
/// the containing folder when the file manager can't select items (or the
/// file no longer exists, e.g. a deletion).
pub async fn reveal_in_file_manager(target: &Path) -> Result<(), TwigError> {
    // Walk up to the nearest existing path (deleted files have none).
    let mut existing = target.to_path_buf();
    while !existing.exists() {
        match existing.parent() {
            Some(p) => existing = p.to_path_buf(),
            None => {
                return Err(TwigError::InvalidArgument(format!(
                    "'{}' does not exist",
                    target.display()
                )))
            }
        }
    }
    let select_file = existing == target;

    #[cfg(target_os = "macos")]
    {
        let mut c = StdCommand::new("open");
        if select_file {
            c.arg("-R");
        }
        c.arg("--").arg(&existing);
        spawn_detached(c)
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut c = StdCommand::new("explorer");
        if select_file {
            // explorer parses `/select,` itself; Windows paths cannot contain
            // `"`, so quoting the path verbatim is safe.
            c.raw_arg(format!("/select,\"{}\"", existing.display()));
        } else {
            c.arg(&existing);
        }
        spawn_detached(c)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if select_file {
            // The freedesktop FileManager1 interface (Nautilus, Dolphin,
            // Nemo, Thunar, ...) highlights the item in its folder.
            let uri = file_uri(&existing);
            let out = tokio::process::Command::new("dbus-send")
                .args([
                    "--session",
                    "--print-reply",
                    "--reply-timeout=2000",
                    "--dest=org.freedesktop.FileManager1",
                    "--type=method_call",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                ])
                .arg(format!("array:string:{uri}"))
                .arg("string:")
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output()
                .await;
            if matches!(out, Ok(ref o) if o.status.success()) {
                return Ok(());
            }
        }
        let dir = if existing.is_dir() {
            existing.clone()
        } else {
            existing
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| existing.clone())
        };
        let mut c = StdCommand::new("xdg-open");
        c.arg(dir);
        spawn_detached(c)
    }
}

// ── .gitignore ───────────────────────────────────────────────────────

/// What to add to `.gitignore` for a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreKind {
    /// Exactly this path, anchored at the repo root (`/dir/file.txt`).
    Path,
    /// Every file with the same extension anywhere (`*.log`).
    Extension,
    /// The given path is a folder; ignore it (`/dir/`).
    Folder,
}

impl IgnoreKind {
    pub fn parse(s: &str) -> Result<Self, TwigError> {
        match s {
            "path" => Ok(Self::Path),
            "extension" => Ok(Self::Extension),
            "folder" => Ok(Self::Folder),
            other => Err(TwigError::InvalidArgument(format!("unknown ignore kind '{other}'"))),
        }
    }
}

/// Escape gitignore glob characters so a path matches only itself.
fn escape_ignore(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | '!' | '#') {
            out.push('\\');
        }
        out.push(c);
    }
    // Trailing spaces are ignored by git unless escaped.
    let trimmed_len = out.trim_end_matches(' ').len();
    let spaces = out.len() - trimmed_len;
    out.truncate(trimmed_len);
    for _ in 0..spaces {
        out.push_str("\\ ");
    }
    out
}

/// Build the `.gitignore` line for `rel` (repo-relative, `/`-separated).
pub fn ignore_pattern(rel: &str, kind: IgnoreKind) -> Result<String, TwigError> {
    let clean = rel.trim_matches('/');
    if clean.is_empty() {
        return Err(TwigError::InvalidArgument("empty path".to_string()));
    }
    if clean.contains('\n') || clean.contains('\r') {
        return Err(TwigError::InvalidArgument("path contains a newline".to_string()));
    }
    match kind {
        IgnoreKind::Path => {
            let dir_suffix = if rel.ends_with('/') { "/" } else { "" };
            Ok(format!("/{}{dir_suffix}", escape_ignore(clean)))
        }
        IgnoreKind::Folder => Ok(format!("/{}/", escape_ignore(clean))),
        IgnoreKind::Extension => {
            let name = clean.rsplit('/').next().unwrap_or(clean);
            match name.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => {
                    Ok(format!("*.{}", escape_ignore(ext)))
                }
                _ => Err(TwigError::InvalidArgument(format!(
                    "'{name}' has no file extension"
                ))),
            }
        }
    }
}

/// Result of [`add_to_gitignore`].
#[derive(Debug, serde::Serialize)]
pub struct IgnoreResult {
    /// The pattern that is now in `.gitignore`.
    pub pattern: String,
    /// The pattern was already present; the file was left unchanged.
    pub already_present: bool,
    /// Some matching files are tracked, so ignoring them has no effect until
    /// they are removed from the index.
    pub tracked: bool,
}

/// Append an ignore pattern for `rel` to the root `.gitignore`.
pub async fn add_to_gitignore(
    repo_path: &Path,
    rel: &str,
    kind: IgnoreKind,
) -> Result<IgnoreResult, TwigError> {
    resolve_in_repo(repo_path, rel)?;
    let pattern = ignore_pattern(rel, kind)?;
    let file = repo_path.join(".gitignore");
    let existing = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };

    let already_present = existing.lines().any(|l| l.trim_end() == pattern);
    if !already_present {
        let mut addition = String::new();
        if !existing.is_empty() && !existing.ends_with('\n') {
            addition.push('\n');
        }
        addition.push_str(&pattern);
        addition.push('\n');
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)?;
        f.write_all(addition.as_bytes())?;
    }

    let tracked = if kind == IgnoreKind::Extension {
        false
    } else {
        let out = run_git_paths(repo_path, &["ls-files"], &[rel.trim_end_matches('/')]).await?;
        out.success && !out.stdout.trim().is_empty()
    };

    Ok(IgnoreResult { pattern, already_present, tracked })
}

// ── External diff tool ───────────────────────────────────────────────

/// Launch an external diff for one file via `git difftool`. When `tool` is
/// given (Settings > External diff tool) it is run as `<tool> $LOCAL $REMOTE`;
/// otherwise git's configured `diff.tool` is used. Returns once the tool is
/// running (or has already exited with an error).
pub async fn open_external_diff(
    repo_path: &Path,
    rel: &str,
    staged: bool,
    tool: Option<&str>,
) -> Result<(), TwigError> {
    resolve_in_repo(repo_path, rel)?;
    let tool = tool.map(str::trim).filter(|t| !t.is_empty());

    if tool.is_none() {
        // Without a configured tool, difftool would fall back to a terminal
        // tool (vimdiff) that can never be shown from the GUI.
        let configured = run_git(repo_path, &["config", "--get", "diff.tool"]).await?;
        if !configured.success || configured.stdout.trim().is_empty() {
            return Err(TwigError::Config(
                "No external diff tool configured. Set one in Settings > Editor & Diff, or set git's diff.tool."
                    .to_string(),
            ));
        }
    }

    let mut args: Vec<String> = vec![
        "--literal-pathspecs".into(),
        "difftool".into(),
        "--no-prompt".into(),
    ];
    if let Some(t) = tool {
        args.push(format!("--extcmd={t}"));
    }
    if staged {
        args.push("--cached".into());
    }
    args.push("--".into());
    args.push(rel.to_string());

    let repo = repo_path.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), TwigError> {
        let mut child = StdCommand::new("git")
            .args(&args)
            .current_dir(&repo)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .env("GIT_TERMINAL_PROMPT", "0")
            .spawn()
            .map_err(|e| TwigError::GitCli(format!("Failed to execute git: {e}")))?;

        // Give the tool a moment: a bad command fails almost immediately,
        // while a real diff viewer keeps running until the user closes it.
        for _ in 0..15 {
            if let Some(status) = child.try_wait()? {
                let mut err = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut err);
                }
                return if status.success() {
                    Ok(())
                } else {
                    Err(TwigError::GitCli(if err.trim().is_empty() {
                        format!("diff tool exited with {status}")
                    } else {
                        err.trim().to_string()
                    }))
                };
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        // Still running: keep draining stderr and reap it when it exits.
        std::thread::spawn(move || {
            if let Some(mut s) = child.stderr.take() {
                let mut sink = Vec::new();
                let _ = s.read_to_end(&mut sink);
            }
            let _ = child.wait();
        });
        Ok(())
    })
    .await
    .map_err(|e| TwigError::Task(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_escapes() {
        let root = Path::new("/repo");
        assert_eq!(resolve_in_repo(root, "a/b.txt").unwrap(), PathBuf::from("/repo/a/b.txt"));
        assert_eq!(resolve_in_repo(root, "dir/").unwrap(), PathBuf::from("/repo/dir"));
        assert!(resolve_in_repo(root, "../x").is_err());
        assert!(resolve_in_repo(root, "a/../../x").is_err());
        assert!(resolve_in_repo(root, "/etc/passwd").is_err());
    }

    #[test]
    fn patterns() {
        assert_eq!(ignore_pattern("src/a.log", IgnoreKind::Path).unwrap(), "/src/a.log");
        assert_eq!(ignore_pattern("build/", IgnoreKind::Path).unwrap(), "/build/");
        assert_eq!(ignore_pattern("x/[a]*.txt", IgnoreKind::Path).unwrap(), "/x/\\[a]\\*.txt");
        assert_eq!(ignore_pattern("#notes ", IgnoreKind::Path).unwrap(), "/\\#notes\\ ");
        assert_eq!(ignore_pattern("src/a.log", IgnoreKind::Extension).unwrap(), "*.log");
        assert!(ignore_pattern("Makefile", IgnoreKind::Extension).is_err());
        assert!(ignore_pattern(".env", IgnoreKind::Extension).is_err());
        assert_eq!(ignore_pattern("src/gen", IgnoreKind::Folder).unwrap(), "/src/gen/");
        assert!(ignore_pattern("a\nb", IgnoreKind::Path).is_err());
    }

    #[tokio::test]
    async fn gitignore_append_and_effect() {
        let dir = std::env::temp_dir().join(format!("twig-file-ops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        assert!(run_git(&dir, &["init", "-q"]).await.unwrap().success);
        std::fs::write(dir.join(".gitignore"), "target").unwrap();
        std::fs::write(dir.join("logs/a.log"), "").unwrap();
        std::fs::write(dir.join("we*ird.txt"), "").unwrap();
        std::fs::write(dir.join("weXird.txt"), "").unwrap();

        let r = add_to_gitignore(&dir, "we*ird.txt", IgnoreKind::Path).await.unwrap();
        assert!(!r.already_present && !r.tracked);
        let r = add_to_gitignore(&dir, "we*ird.txt", IgnoreKind::Path).await.unwrap();
        assert!(r.already_present);
        add_to_gitignore(&dir, "logs/a.log", IgnoreKind::Extension).await.unwrap();

        let content = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(content, "target\n/we\\*ird.txt\n*.log\n");

        let out = run_git(&dir, &["status", "--porcelain", "--untracked-files=all"]).await.unwrap();
        assert!(!out.stdout.contains("we*ird.txt"), "{}", out.stdout);
        assert!(out.stdout.contains("weXird.txt"), "{}", out.stdout);
        assert!(!out.stdout.contains("a.log"), "{}", out.stdout);
        assert!(add_to_gitignore(&dir, "../x", IgnoreKind::Path).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
