//! Creating repositories: `git init` in a folder and `git clone` from any URL.

use std::path::Path;
use std::process::Stdio;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::error::TwigError;
use crate::git::writer::{run_git, safe_ref, GitOutput};

fn require_abs(path: &str, what: &str) -> Result<(), TwigError> {
    if path.trim().is_empty() || path.starts_with('-') || !Path::new(path).is_absolute() {
        return Err(TwigError::InvalidArgument(format!("{what} must be an absolute path: '{path}'")));
    }
    Ok(())
}

/// Validate a clone URL: non-empty, no leading `-` (git would read it as an
/// option), no whitespace or control characters. Any scheme git supports is
/// accepted; git's own `protocol.*.allow` policy still applies.
pub(crate) fn validate_clone_url(url: &str) -> Result<(), TwigError> {
    let bad = url.is_empty()
        || url.starts_with('-')
        || url.chars().any(|c| c.is_whitespace() || c.is_control());
    if bad {
        return Err(TwigError::InvalidArgument(format!("'{url}' is not a valid repository URL")));
    }
    Ok(())
}

/// `git init [-b <branch>] -- <path>`, creating the folder if needed.
/// Refuses a folder that already is a repository (open it instead).
pub async fn init_repo(path: &str, initial_branch: Option<&str>) -> Result<GitOutput, TwigError> {
    require_abs(path, "Repository folder")?;
    let dir = Path::new(path);
    if dir.join(".git").exists() {
        return Err(TwigError::InvalidArgument(format!(
            "'{path}' is already a git repository; open it instead"
        )));
    }
    std::fs::create_dir_all(dir)?;
    let branch = initial_branch.map(str::trim).filter(|b| !b.is_empty());
    let mut args = vec!["init"];
    if let Some(b) = branch {
        safe_ref(b)?;
        let check = run_git(dir, &["check-ref-format", "--branch", b]).await?;
        if !check.success {
            return Err(TwigError::InvalidArgument(format!("'{b}' is not a valid branch name")));
        }
        args.extend(["-b", b]);
    }
    args.extend(["--", path]);
    run_git(dir, &args).await
}

/// `git clone --progress -- <url> <dest>`. `dest` must be absolute and either
/// missing or an empty folder. `env` is extra environment (HTTPS auth);
/// `on_progress` receives git's progress lines as they arrive.
pub async fn clone_repo(
    url: &str,
    dest: &str,
    env: &[(String, String)],
    mut on_progress: impl FnMut(&str),
) -> Result<GitOutput, TwigError> {
    let url = url.trim();
    validate_clone_url(url)?;
    require_abs(dest, "Clone destination")?;
    let dest_path = Path::new(dest);
    if dest_path.exists() {
        let empty = std::fs::read_dir(dest_path).map(|mut d| d.next().is_none()).unwrap_or(false);
        if !empty {
            return Err(TwigError::InvalidArgument(format!(
                "'{dest}' already exists and is not an empty folder"
            )));
        }
    }

    let mut child = Command::new("git")
        .args(["clone", "--progress", "--", url, dest])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git clone: {e}")))?;

    // Progress lines end in `\r` (updates) or `\n`; keep the full text for errors.
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let mut buf = [0u8; 4096];
        let mut line = Vec::new();
        loop {
            let n = pipe.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if b == b'\r' || b == b'\n' {
                    if !line.is_empty() {
                        let text = String::from_utf8_lossy(&line).into_owned();
                        on_progress(text.trim());
                        if b == b'\n' {
                            stderr.push_str(&text);
                            stderr.push('\n');
                        }
                        line.clear();
                    }
                } else {
                    line.push(b);
                }
            }
        }
        if !line.is_empty() {
            stderr.push_str(&String::from_utf8_lossy(&line));
        }
    }
    let status = child.wait().await?;
    Ok(GitOutput { success: status.success(), stdout: String::new(), stderr })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-create-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn init_creates_folder_and_branch() {
        let dir = tmp("init");
        let path = dir.join("new repo");
        let p = path.to_string_lossy().to_string();
        let out = init_repo(&p, Some("trunk")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let head = std::fs::read_to_string(path.join(".git/HEAD")).unwrap();
        assert_eq!(head.trim(), "ref: refs/heads/trunk");

        assert!(init_repo(&p, None).await.is_err(), "re-init must be refused");
        assert!(init_repo("relative/x", None).await.is_err());
        let q = dir.join("other").to_string_lossy().to_string();
        assert!(init_repo(&q, Some("bad..name")).await.is_err());
        assert!(init_repo(&q, Some("-x")).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn clone_from_local_url_with_progress() {
        let dir = tmp("clone");
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let s = src.to_string_lossy().to_string();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "c1"],
        ] {
            assert!(run_git(&src, &args).await.unwrap().success);
        }
        let url = format!("file://{s}");
        let dest = dir.join("dest").to_string_lossy().to_string();
        let mut lines = Vec::new();
        let out = clone_repo(&url, &dest, &[], |l| lines.push(l.to_string())).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(Path::new(&dest).join(".git").exists());
        assert!(!lines.is_empty(), "no progress lines");

        // Non-empty destination, bad URLs, relative destination.
        assert!(clone_repo(&url, &dest, &[], |_| {}).await.is_err());
        let fresh = dir.join("fresh").to_string_lossy().to_string();
        for bad in ["", "--upload-pack=evil", "https://x y", "a\nb"] {
            assert!(clone_repo(bad, &fresh, &[], |_| {}).await.is_err(), "{bad:?}");
        }
        assert!(clone_repo(&url, "rel/dir", &[], |_| {}).await.is_err());

        // A failing clone reports git's error.
        let missing = format!("file://{}", dir.join("nope").to_string_lossy());
        let out = clone_repo(&missing, &fresh, &[], |_| {}).await.unwrap();
        assert!(!out.success);
        assert!(!out.stderr.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
