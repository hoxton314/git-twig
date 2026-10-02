//! Tag operations: listing via git2, create/delete/push via the git CLI.
use std::path::Path;

use git2::{ObjectType, Repository};
use serde::Serialize;

use super::writer::{run_git, safe_ref, GitOutput};
use crate::error::TwigError;

#[derive(Debug, Serialize)]
pub struct TagInfo {
    pub name: String,
    /// Commit the tag ultimately points at (None if it targets a non-commit).
    pub target_oid: Option<String>,
    pub short_target_oid: Option<String>,
    pub annotated: bool,
    /// Annotation message (annotated tags only).
    pub message: Option<String>,
    pub tagger_name: Option<String>,
    /// Tagger date for annotated tags, otherwise the target commit's time.
    pub timestamp: i64,
    pub commit_summary: Option<String>,
}

/// List all tags, newest first.
pub fn read_tags(repo: &Repository) -> Result<Vec<TagInfo>, TwigError> {
    let mut tags = Vec::new();
    for reference in repo.references_glob("refs/tags/*")? {
        let Ok(reference) = reference else { continue };
        let Some(name) = reference.shorthand().map(str::to_string) else { continue };
        let Some(direct) = reference.target() else { continue };

        let mut annotated = false;
        let mut message = None;
        let mut tagger_name = None;
        let mut timestamp = 0;
        if let Ok(tag) = repo.find_tag(direct) {
            annotated = true;
            message = tag.message().map(|m| m.trim_end().to_string());
            if let Some(sig) = tag.tagger() {
                tagger_name = sig.name().map(str::to_string);
                timestamp = sig.when().seconds();
            }
        }

        let commit = reference
            .peel(ObjectType::Commit)
            .ok()
            .and_then(|o| o.into_commit().ok());
        if timestamp == 0 {
            if let Some(c) = &commit {
                timestamp = c.time().seconds();
            }
        }
        let target_oid = commit.as_ref().map(|c| c.id().to_string());
        tags.push(TagInfo {
            name,
            short_target_oid: target_oid.as_ref().map(|s| s.chars().take(7).collect()),
            target_oid,
            annotated,
            message,
            tagger_name,
            timestamp,
            commit_summary: commit.as_ref().and_then(|c| c.summary().map(str::to_string)),
        });
    }
    tags.sort_by(|a, b| b.timestamp.cmp(&a.timestamp).then_with(|| a.name.cmp(&b.name)));
    Ok(tags)
}

/// Create a tag at `target`. A non-empty `message` makes it annotated.
pub async fn create_tag(
    repo_path: &Path,
    name: &str,
    target: &str,
    message: Option<&str>,
) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    safe_ref(target)?;
    let check = run_git(repo_path, &["check-ref-format", &format!("refs/tags/{name}")]).await?;
    if !check.success {
        return Err(TwigError::InvalidArgument(format!("'{name}' is not a valid tag name")));
    }
    match message.map(str::trim).filter(|m| !m.is_empty()) {
        Some(msg) => run_git(repo_path, &["tag", "-a", "-m", msg, "--", name, target]).await,
        None => run_git(repo_path, &["tag", "--", name, target]).await,
    }
}

pub async fn delete_tag(repo_path: &Path, name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(name)?;
    run_git(repo_path, &["tag", "-d", "--", name]).await
}

/// Pick `remote`, or `origin`, or the only configured remote.
pub async fn resolve_remote(repo_path: &Path, remote: Option<&str>) -> Result<String, TwigError> {
    if let Some(r) = remote.filter(|r| !r.is_empty()) {
        safe_ref(r)?;
        return Ok(r.to_string());
    }
    let out = run_git(repo_path, &["remote"]).await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    let remotes: Vec<&str> = out.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if remotes.contains(&"origin") {
        return Ok("origin".to_string());
    }
    match remotes.as_slice() {
        [only] => Ok((*only).to_string()),
        [] => Err(TwigError::InvalidArgument("this repository has no remotes".to_string())),
        _ => Err(TwigError::InvalidArgument(
            "several remotes are configured and none is named 'origin'".to_string(),
        )),
    }
}

/// Push one tag (`Some(name)`) or all tags (`None`) to `remote`.
pub async fn push_tags(repo_path: &Path, remote: &str, name: Option<&str>) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    match name {
        Some(n) => {
            safe_ref(n)?;
            let spec = format!("refs/tags/{n}:refs/tags/{n}");
            run_git(repo_path, &["push", remote, &spec]).await
        }
        None => run_git(repo_path, &["push", remote, "--tags"]).await,
    }
}

pub async fn delete_remote_tag(repo_path: &Path, remote: &str, name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    safe_ref(name)?;
    let spec = format!(":refs/tags/{name}");
    run_git(repo_path, &["push", remote, &spec]).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-tags-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        git(&dir, &["config", "user.name", "t"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "tag.gpgsign", "false"]).await;
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        git(&dir, &["add", "--", "a.txt"]).await;
        git(&dir, &["commit", "-q", "-m", "first"]).await;
        dir
    }

    #[tokio::test]
    async fn create_list_push_delete() {
        let dir = temp_repo("crud").await;
        let head = git(&dir, &["rev-parse", "HEAD"]).await;

        assert!(create_tag(&dir, "v1", &head, None).await.unwrap().success);
        let out = create_tag(&dir, "v2", "HEAD", Some("Release two\n\nnotes")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(create_tag(&dir, "-bad", "HEAD", None).await.is_err());
        assert!(create_tag(&dir, "bad..name", "HEAD", None).await.is_err());
        assert!(!create_tag(&dir, "v1", "HEAD", None).await.unwrap().success);

        let repo = Repository::open(&dir).unwrap();
        let tags = read_tags(&repo).unwrap();
        assert_eq!(tags.len(), 2);
        let v1 = tags.iter().find(|t| t.name == "v1").unwrap();
        let v2 = tags.iter().find(|t| t.name == "v2").unwrap();
        assert!(!v1.annotated);
        assert!(v2.annotated);
        assert_eq!(v2.message.as_deref(), Some("Release two\n\nnotes"));
        assert_eq!(v2.target_oid.as_deref(), Some(head.as_str()));
        assert_eq!(v1.commit_summary.as_deref(), Some("first"));

        // Push to / delete from a bare remote.
        let remote = dir.with_extension("remote.git");
        let _ = std::fs::remove_dir_all(&remote);
        git(&dir, &["init", "-q", "--bare", remote.to_str().unwrap()]).await;
        git(&dir, &["remote", "add", "origin", remote.to_str().unwrap()]).await;
        assert_eq!(resolve_remote(&dir, None).await.unwrap(), "origin");
        let out = push_tags(&dir, "origin", Some("v2")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(git(&remote, &["tag"]).await, "v2");
        assert!(push_tags(&dir, "origin", None).await.unwrap().success);
        let out = delete_remote_tag(&dir, "origin", "v2").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(git(&remote, &["tag"]).await, "v1");

        assert!(delete_tag(&dir, "v1").await.unwrap().success);
        assert_eq!(read_tags(&repo).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&remote);
    }
}
