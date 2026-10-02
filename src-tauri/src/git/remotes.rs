//! Remote management.
//! Reads (listing remotes, resolving a remote-tracking ref's remote) use git2;
//! writes (add/remove/rename/set-url/fetch/prune) use the git CLI.
use std::path::Path;

use git2::Repository;
use serde::Serialize;

use super::writer::{run_git, safe_ref, GitOutput};
use crate::error::TwigError;

#[derive(Debug, Clone, Serialize)]
pub struct RemoteInfo {
    pub name: String,
    /// `remote.<name>.url`, if configured.
    pub fetch_url: Option<String>,
    /// Effective push URL: `remote.<name>.pushurl` if set, else the fetch URL.
    pub push_url: Option<String>,
    /// Whether a dedicated `pushurl` is configured.
    pub has_separate_push_url: bool,
}

/// Split a remote-tracking branch into `(remote, short_name)`.
///
/// `full_ref` is the full ref (`refs/remotes/<remote>/<branch>`) and `name`
/// the shorthand (`<remote>/<branch>`). The remote is resolved through the
/// configured fetch refspecs, so remote names containing `/` work; if that
/// fails (e.g. a stale ref of a removed remote) it falls back to the first `/`.
pub(crate) fn remote_parts(
    repo: &Repository,
    full_ref: Option<&str>,
    name: &str,
) -> (Option<String>, String) {
    let resolved = full_ref
        .and_then(|full| repo.branch_remote_name(full).ok())
        .and_then(|buf| buf.as_str().map(String::from));
    if let Some(remote) = resolved {
        if let Some(short) = name.strip_prefix(&format!("{remote}/")) {
            return (Some(remote), short.to_string());
        }
    }
    match name.split_once('/') {
        Some((remote, short)) => (Some(remote.to_string()), short.to_string()),
        None => (None, name.to_string()),
    }
}

pub fn list_remotes(repo: &Repository) -> Result<Vec<RemoteInfo>, TwigError> {
    let names = repo.remotes()?;
    let mut out = Vec::new();
    for name in names.iter().flatten() {
        // A remote with a broken config entry should not hide the others.
        let Ok(remote) = repo.find_remote(name) else {
            continue;
        };
        let fetch_url = remote.url().map(String::from);
        let explicit_push = remote.pushurl().map(String::from);
        out.push(RemoteInfo {
            name: name.to_string(),
            has_separate_push_url: explicit_push.is_some(),
            push_url: explicit_push.or_else(|| fetch_url.clone()),
            fetch_url,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Reject URLs git could read as options; URLs never start with `-`.
fn safe_url(url: &str) -> Result<(), TwigError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(TwigError::InvalidArgument("remote URL must not be empty".to_string()));
    }
    if url.starts_with('-') {
        return Err(TwigError::InvalidArgument(format!(
            "invalid remote URL '{url}': must not start with '-'"
        )));
    }
    Ok(())
}

pub async fn add_remote(repo_path: &Path, name: &str, url: &str) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    safe_url(url)?;
    run_git(repo_path, &["remote", "add", "--", name, url.trim()]).await
}

pub async fn remove_remote(repo_path: &Path, name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    run_git(repo_path, &["remote", "remove", "--", name]).await
}

pub async fn rename_remote(
    repo_path: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(old_name)?;
    safe_ref(new_name)?;
    run_git(repo_path, &["remote", "rename", "--", old_name, new_name]).await
}

/// Set the fetch URL and, optionally, a separate push URL. A `None`/empty
/// push URL (or one equal to the fetch URL) removes any dedicated `pushurl`.
pub async fn set_remote_urls(
    repo_path: &Path,
    name: &str,
    fetch_url: &str,
    push_url: Option<&str>,
) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    safe_url(fetch_url)?;
    let fetch_url = fetch_url.trim();
    let out = run_git(repo_path, &["remote", "set-url", "--", name, fetch_url]).await?;
    if !out.success {
        return Ok(out);
    }

    let push = push_url.map(str::trim).filter(|p| !p.is_empty() && *p != fetch_url);
    match push {
        Some(p) => {
            safe_url(p)?;
            // Replace every existing pushurl with exactly this one.
            let key = format!("remote.{name}.pushurl");
            let _ = run_git(repo_path, &["config", "--unset-all", &key]).await?;
            run_git(repo_path, &["remote", "set-url", "--push", "--", name, p]).await
        }
        None => {
            let key = format!("remote.{name}.pushurl");
            let unset = run_git(repo_path, &["config", "--unset-all", &key]).await?;
            // Exit code 5 means "no such key", which is fine here; any other
            // failure is reported. `run_git` only exposes success, so treat a
            // failure with an empty stderr as the benign "not set" case.
            if unset.success || unset.stderr.trim().is_empty() {
                Ok(out)
            } else {
                Ok(unset)
            }
        }
    }
}

pub async fn fetch_remote(repo_path: &Path, name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    run_git(repo_path, &["fetch", "--prune", "--", name]).await
}

pub async fn prune_remote(repo_path: &Path, name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    run_git(repo_path, &["remote", "prune", "--", name]).await
}

/// `<base>-<pid>-<n>`: a scratch ref name no concurrent call shares.
pub fn unique_scratch_name(base: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!("{base}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed))
}

/// Fetch one ref (e.g. `refs/pull/7/head`) from `remote` and return the
/// commit it points to. The ref is fetched into a private scratch ref rather
/// than read from `FETCH_HEAD`, which a concurrent background fetch could
/// overwrite in between; the scratch ref is removed afterwards.
/// Returns `Ok(Err(output))` when the fetch itself fails.
pub async fn fetch_ref_commit(
    repo_path: &Path,
    remote: &str,
    src_ref: &str,
    scratch_name: &str,
) -> Result<Result<String, GitOutput>, TwigError> {
    safe_ref(remote)?;
    safe_ref(src_ref)?;
    if !src_ref.starts_with("refs/") || src_ref.contains(':') {
        return Err(TwigError::InvalidArgument(format!("'{src_ref}' is not a full ref name")));
    }
    if scratch_name.is_empty() || !scratch_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(TwigError::InvalidArgument(format!("bad scratch ref name '{scratch_name}'")));
    }
    let scratch = format!("refs/twig/fetch/{scratch_name}");
    let refspec = format!("+{src_ref}:{scratch}");
    let fetch = run_git(
        repo_path,
        &["fetch", "--no-tags", "--no-write-fetch-head", "--", remote, &refspec],
    )
    .await?;
    if !fetch.success {
        return Ok(Err(fetch));
    }
    let spec = format!("{scratch}^{{commit}}");
    let sha = run_git(repo_path, &["rev-parse", "--verify", "--quiet", &spec]).await?;
    let _ = run_git(repo_path, &["update-ref", "-d", &scratch]).await?;
    let oid = sha.stdout.trim().to_string();
    if !sha.success || oid.is_empty() {
        return Err(TwigError::GitCli(format!("could not resolve the fetched {src_ref}")));
    }
    Ok(Ok(oid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git_ok(dir: &Path, args: &[&str]) {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-remotes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        git_ok(&dir, &["add", "a.txt"]).await;
        git_ok(&dir, &["commit", "-q", "-m", "c1"]).await;
        dir
    }

    #[tokio::test]
    async fn fetch_ref_commit_uses_a_scratch_ref() {
        let upstream = temp_repo("pr-up").await;
        git_ok(&upstream, &["commit", "-q", "--allow-empty", "-m", "pr head"]).await;
        let head = run_git(&upstream, &["rev-parse", "HEAD"]).await.unwrap().stdout.trim().to_string();
        git_ok(&upstream, &["update-ref", "refs/pull/7/head", &head]).await;
        git_ok(&upstream, &["reset", "-q", "--hard", "HEAD~1"]).await;

        let dir = temp_repo("pr").await;
        let url = upstream.to_string_lossy().to_string();
        git_ok(&dir, &["remote", "add", "origin", &url]).await;
        // A stale FETCH_HEAD pointing elsewhere must not be used.
        std::fs::write(dir.join(".git/FETCH_HEAD"), format!("{}\t\tbogus\n", "0".repeat(40))).unwrap();

        let got = fetch_ref_commit(&dir, "origin", "refs/pull/7/head", "pr-7").await.unwrap();
        assert_eq!(got.ok().as_deref(), Some(head.as_str()));
        let scratch = run_git(&dir, &["rev-parse", "--verify", "--quiet", "refs/twig/fetch/pr-7"]).await.unwrap();
        assert!(!scratch.success, "scratch ref left behind");

        // Concurrent fetches of the same PR use distinct scratch refs.
        let (a, b) = (unique_scratch_name("pr-7"), unique_scratch_name("pr-7"));
        assert_ne!(a, b);
        let (ra, rb) = tokio::join!(
            fetch_ref_commit(&dir, "origin", "refs/pull/7/head", &a),
            fetch_ref_commit(&dir, "origin", "refs/pull/7/head", &b),
        );
        assert_eq!(ra.unwrap().ok().as_deref(), Some(head.as_str()));
        assert_eq!(rb.unwrap().ok().as_deref(), Some(head.as_str()));

        let missing = fetch_ref_commit(&dir, "origin", "refs/pull/8/head", "pr-8").await.unwrap();
        assert!(missing.is_err());
        assert!(fetch_ref_commit(&dir, "origin", "main:x", "pr-9").await.is_err());
        assert!(fetch_ref_commit(&dir, "origin", "refs/pull/7/head", "../x").await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&upstream);
    }

    #[tokio::test]
    async fn remote_crud_and_slashed_names() {
        let upstream = temp_repo("crud-up").await;
        let dir = temp_repo("crud").await;
        let url = upstream.to_string_lossy().to_string();

        assert!(add_remote(&dir, "-bad", &url).await.is_err());
        assert!(add_remote(&dir, "x", "--upload-pack=evil").await.is_err());

        let out = add_remote(&dir, "team/up", &url).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let out = fetch_remote(&dir, "team/up").await.unwrap();
        assert!(out.success, "{}", out.stderr);

        {
            let repo = Repository::open(&dir).unwrap();
            let remotes = list_remotes(&repo).unwrap();
            assert_eq!(remotes.len(), 1);
            assert_eq!(remotes[0].name, "team/up");
            assert_eq!(remotes[0].fetch_url.as_deref(), Some(url.as_str()));
            assert!(!remotes[0].has_separate_push_url);

            let (remote, short) =
                remote_parts(&repo, Some("refs/remotes/team/up/main"), "team/up/main");
            assert_eq!(remote.as_deref(), Some("team/up"));
            assert_eq!(short, "main");
        }

        let out = set_remote_urls(&dir, "team/up", &url, Some("/tmp/push-only")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        {
            let repo = Repository::open(&dir).unwrap();
            let r = &list_remotes(&repo).unwrap()[0];
            assert!(r.has_separate_push_url);
            assert_eq!(r.push_url.as_deref(), Some("/tmp/push-only"));
        }
        let out = set_remote_urls(&dir, "team/up", &url, None).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        {
            let repo = Repository::open(&dir).unwrap();
            assert!(!list_remotes(&repo).unwrap()[0].has_separate_push_url);
        }

        let out = rename_remote(&dir, "team/up", "origin").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let out = prune_remote(&dir, "origin").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let out = remove_remote(&dir, "origin").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        {
            let repo = Repository::open(&dir).unwrap();
            assert!(list_remotes(&repo).unwrap().is_empty());
        }

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&upstream);
    }
}
