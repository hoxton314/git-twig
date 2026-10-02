//! Git LFS management through the `git lfs` CLI: tracked patterns
//! (`.gitattributes`), file locks, and fetch / prune. Network commands
//! (locks, fetch) are run by the command layer inside `with_network_auth`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::TwigError;
use crate::git::writer::{run_git, GitOutput};

fn invalid(msg: impl Into<String>) -> TwigError {
    TwigError::InvalidArgument(msg.into())
}

fn cli_error(out: &GitOutput) -> TwigError {
    let msg = out.stderr.trim();
    let msg = if msg.is_empty() { out.stdout.trim() } else { msg };
    // git-lfs appends "Errors logged to …" boilerplate; the first line says it.
    let first = msg.lines().find(|l| !l.trim().is_empty()).unwrap_or("git lfs failed");
    TwigError::GitCli(first.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LfsPattern {
    pub pattern: String,
    /// The `.gitattributes` file it comes from.
    pub source: String,
    #[serde(default)]
    pub lockable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LfsStatus {
    /// `git lfs version`, or `None` when git-lfs isn't installed.
    pub version: Option<String>,
    pub patterns: Vec<LfsPattern>,
}

#[derive(Debug, Deserialize)]
struct TrackJson {
    #[serde(default)]
    patterns: Vec<TrackJsonPattern>,
}

#[derive(Debug, Deserialize)]
struct TrackJsonPattern {
    pattern: String,
    source: String,
    #[serde(default)]
    lockable: bool,
    #[serde(default = "yes")]
    tracked: bool,
}

fn yes() -> bool {
    true
}

pub async fn lfs_version(repo_path: &Path) -> Option<String> {
    let out = run_git(repo_path, &["lfs", "version"]).await.ok()?;
    out.success.then(|| out.stdout.trim().to_string()).filter(|v| !v.is_empty())
}

/// Installed version and the tracked patterns.
pub async fn status(repo_path: &Path) -> Result<LfsStatus, TwigError> {
    let Some(version) = lfs_version(repo_path).await else {
        return Ok(LfsStatus { version: None, patterns: Vec::new() });
    };
    let out = run_git(repo_path, &["lfs", "track", "--json"]).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    let parsed: TrackJson = serde_json::from_str(&out.stdout)
        .map_err(|e| invalid(format!("unexpected `git lfs track --json` output: {e}")))?;
    let patterns = parsed
        .patterns
        .into_iter()
        .filter(|p| p.tracked)
        .map(|p| LfsPattern { pattern: p.pattern, source: p.source, lockable: p.lockable })
        .collect();
    Ok(LfsStatus { version: Some(version), patterns })
}

fn check_pattern(pattern: &str) -> Result<&str, TwigError> {
    let p = pattern.trim();
    if p.is_empty() {
        return Err(invalid("enter a pattern such as *.psd"));
    }
    if p.contains(['\n', '\r']) {
        return Err(invalid("a pattern must be a single line"));
    }
    Ok(p)
}

/// Track `pattern` with LFS (writes `.gitattributes`; commit it to share).
pub async fn track(repo_path: &Path, pattern: &str, lockable: bool) -> Result<String, TwigError> {
    let p = check_pattern(pattern)?;
    let mut args = vec!["lfs", "track"];
    if lockable {
        args.push("--lockable");
    }
    args.extend_from_slice(&["--", p]);
    let out = run_git(repo_path, &args).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    Ok(out.stdout.trim().to_string())
}

pub async fn untrack(repo_path: &Path, pattern: &str) -> Result<String, TwigError> {
    let p = check_pattern(pattern)?;
    let out = run_git(repo_path, &["lfs", "untrack", "--", p]).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    Ok(out.stdout.trim().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    pub owner: String,
    pub locked_at: String,
    /// Held by the current user.
    #[serde(default)]
    pub ours: bool,
}

#[derive(Debug, Deserialize)]
struct RawLock {
    id: String,
    path: String,
    #[serde(default)]
    owner: Option<RawOwner>,
    #[serde(default)]
    locked_at: String,
}

#[derive(Debug, Deserialize)]
struct RawOwner {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct VerifiedLocks {
    #[serde(default)]
    ours: Vec<RawLock>,
    #[serde(default)]
    theirs: Vec<RawLock>,
}

fn to_lock(raw: RawLock, ours: bool) -> LfsLock {
    LfsLock {
        id: raw.id,
        path: raw.path,
        owner: raw.owner.map(|o| o.name).unwrap_or_default(),
        locked_at: raw.locked_at,
        ours,
    }
}

/// Parse `git lfs locks --verify --json` (ours / theirs), sorted by path.
pub(crate) fn parse_locks(json: &str) -> Result<Vec<LfsLock>, TwigError> {
    let v: VerifiedLocks =
        serde_json::from_str(json).map_err(|e| invalid(format!("unexpected `git lfs locks` output: {e}")))?;
    let mut locks: Vec<LfsLock> = v
        .ours
        .into_iter()
        .map(|l| to_lock(l, true))
        .chain(v.theirs.into_iter().map(|l| to_lock(l, false)))
        .collect();
    locks.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(locks)
}

/// Locks on the remote, marked ours / theirs (needs network auth).
pub async fn locks(repo_path: &Path) -> Result<Vec<LfsLock>, TwigError> {
    let out = run_git(repo_path, &["lfs", "locks", "--verify", "--json"]).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    parse_locks(&out.stdout)
}

fn check_path(path: &str) -> Result<&str, TwigError> {
    let p = path.trim();
    if p.is_empty() || p.starts_with('-') || p.contains(['\n', '\r']) || Path::new(p).is_absolute() {
        return Err(invalid(format!("'{path}' is not a repository path")));
    }
    Ok(p)
}

/// Lock `path` (repo-relative) for the current user.
pub async fn lock(repo_path: &Path, path: &str) -> Result<(), TwigError> {
    let p = check_path(path)?;
    let out = run_git(repo_path, &["lfs", "lock", "--json", p]).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    Ok(())
}

/// Unlock by lock id; `force` releases someone else's lock (needs rights).
pub async fn unlock(repo_path: &Path, id: &str, force: bool) -> Result<(), TwigError> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') || id.starts_with('-') {
        return Err(invalid(format!("'{id}' is not a lock id")));
    }
    let id_arg = format!("--id={id}");
    let mut args = vec!["lfs", "unlock", id_arg.as_str()];
    if force {
        args.push("--force");
    }
    let out = run_git(repo_path, &args).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    Ok(())
}

/// `git lfs fetch` (objects for the current checkout; `all`: every ref).
pub async fn fetch(repo_path: &Path, all: bool) -> Result<String, TwigError> {
    let mut args = vec!["lfs", "fetch"];
    if all {
        args.push("--all");
    }
    let out = run_git(repo_path, &args).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    Ok(format!("{}{}", out.stdout, out.stderr).trim().to_string())
}

/// `git lfs prune` (`dry_run`: only report what would be deleted).
pub async fn prune(repo_path: &Path, dry_run: bool) -> Result<String, TwigError> {
    let mut args = vec!["lfs", "prune"];
    if dry_run {
        // The summary line ("N files would be pruned (size)") is only
        // printed without --verbose.
        args.push("--dry-run");
    }
    let out = run_git(repo_path, &args).await?;
    if !out.success {
        return Err(cli_error(&out));
    }
    Ok(format!("{}{}", out.stdout, out.stderr).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    #[tokio::test]
    async fn tracks_and_untracks_patterns() {
        let dir = std::env::temp_dir().join(format!("twig-lfs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        if lfs_version(&dir).await.is_none() {
            eprintln!("git-lfs not installed; skipping");
            return;
        }
        assert!(status(&dir).await.unwrap().patterns.is_empty());
        track(&dir, "*.psd", false).await.unwrap();
        track(&dir, "docs/*.bin", true).await.unwrap();
        // A leading dash is a pattern, not an option.
        track(&dir, "-odd*.x", false).await.unwrap();
        let st = status(&dir).await.unwrap();
        assert!(st.version.as_deref().is_some_and(|v| v.starts_with("git-lfs/")));
        assert_eq!(
            st.patterns,
            [
                LfsPattern { pattern: "*.psd".into(), source: ".gitattributes".into(), lockable: false },
                LfsPattern { pattern: "docs/*.bin".into(), source: ".gitattributes".into(), lockable: true },
                LfsPattern { pattern: "-odd*.x".into(), source: ".gitattributes".into(), lockable: false },
            ]
        );
        untrack(&dir, "*.psd").await.unwrap();
        untrack(&dir, "-odd*.x").await.unwrap();
        let attrs = std::fs::read_to_string(dir.join(".gitattributes")).unwrap();
        assert_eq!(attrs.trim(), "docs/*.bin filter=lfs diff=lfs merge=lfs -text lockable");
        assert!(track(&dir, "  ", false).await.is_err());
        assert!(track(&dir, "a\nb", false).await.is_err());

        // Without a commit prune fails: the error is git-lfs's first line,
        // without its "Errors logged to …" boilerplate.
        let err = prune(&dir, true).await.unwrap_err().to_string();
        assert!(err.contains("HEAD") && !err.contains("Errors logged to"), "{err}");
        git(&dir, &["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "i"]).await;
        prune(&dir, true).await.unwrap();
        // Nothing to download is not an error.
        fetch(&dir, false).await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn prunes_unreferenced_objects() {
        let dir = std::env::temp_dir().join(format!("twig-lfs-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        if lfs_version(&dir).await.is_none() {
            eprintln!("git-lfs not installed; skipping");
            return;
        }
        // Filters in the repo's own config, independent of the user's setup.
        git(&dir, &["lfs", "install", "--local"]).await;
        let c = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        track(&dir, "*.bin", false).await.unwrap();
        std::fs::write(dir.join("a.bin"), vec![1u8; 3000]).unwrap();
        git(&dir, &["add", "."]).await;
        git(&dir, &[&c[..], &["commit", "-q", "-m", "one"]].concat()).await;
        std::fs::write(dir.join("a.bin"), vec![2u8; 3000]).unwrap();
        git(&dir, &[&c[..], &["commit", "-q", "-am", "two"]].concat()).await;
        git(&dir, &["reset", "-q", "--hard", "HEAD~1"]).await;
        git(&dir, &["reflog", "expire", "--expire=now", "--all"]).await;

        let dry = prune(&dir, true).await.unwrap();
        assert!(dry.contains("1 file would be pruned"), "{dry}");
        prune(&dir, false).await.unwrap();
        let again = prune(&dir, true).await.unwrap();
        assert!(!again.contains("would be pruned"), "{again}");
        assert_eq!(std::fs::read(dir.join("a.bin")).unwrap(), vec![1u8; 3000], "the current object stays");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_verified_locks() {
        let json = r#"{"ours":[{"id":"2","path":"b.psd","owner":{"name":"me"},"locked_at":"2026-01-01T00:00:00Z"}],
                      "theirs":[{"id":"1","path":"a.psd","owner":{"name":"Ada"},"locked_at":"2026-01-02T00:00:00Z"}]}"#;
        let locks = parse_locks(json).unwrap();
        assert_eq!(locks.len(), 2);
        assert_eq!((locks[0].path.as_str(), locks[0].owner.as_str(), locks[0].ours), ("a.psd", "Ada", false));
        assert_eq!((locks[1].id.as_str(), locks[1].ours), ("2", true));
        assert!(parse_locks(r#"{"ours":[],"theirs":[]}"#).unwrap().is_empty());
        assert!(parse_locks("Error: no remote").is_err());
    }

    #[test]
    fn rejects_unsafe_paths_and_ids() {
        assert!(check_path("--force").is_err());
        assert!(check_path("/etc/passwd").is_err());
        assert!(check_path("").is_err());
        assert_eq!(check_path(" art/a.psd ").unwrap(), "art/a.psd");
    }

    #[tokio::test]
    async fn unlock_validates_ids() {
        let dir = std::env::temp_dir();
        assert!(unlock(&dir, "--all", false).await.is_err());
        assert!(unlock(&dir, "a b", false).await.is_err());
        assert!(unlock(&dir, "", false).await.is_err());
    }
}
