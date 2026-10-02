//! Submodules: listing via git2, init/update/sync via the git CLI.

use std::path::Path;

use git2::{Repository, SubmoduleIgnore, SubmoduleStatus};
use serde::Serialize;

use crate::error::TwigError;
use crate::git::history::validate_rel_path;
use crate::git::writer::{run_git, run_git_paths, GitOutput};

#[derive(Debug, Clone, Serialize)]
pub struct SubmoduleInfo {
    pub name: String,
    /// Path relative to the superproject root.
    pub path: String,
    /// Absolute path on disk (for opening it as a repo tab).
    pub abs_path: String,
    pub url: Option<String>,
    pub branch: Option<String>,
    /// Commit recorded in the superproject's HEAD.
    pub head_oid: Option<String>,
    /// Commit currently checked out inside the submodule.
    pub workdir_oid: Option<String>,
    /// "uninitialized" | "out_of_date" | "dirty" | "up_to_date"
    pub status: String,
}

fn status_label(st: SubmoduleStatus, head: Option<git2::Oid>, wd: Option<git2::Oid>) -> &'static str {
    if st.contains(SubmoduleStatus::WD_UNINITIALIZED) || !st.contains(SubmoduleStatus::IN_WD) {
        return "uninitialized";
    }
    if st.contains(SubmoduleStatus::WD_MODIFIED) || (head.is_some() && wd.is_some() && head != wd) {
        return "out_of_date";
    }
    if st.intersects(
        SubmoduleStatus::WD_INDEX_MODIFIED
            | SubmoduleStatus::WD_WD_MODIFIED
            | SubmoduleStatus::WD_UNTRACKED,
    ) {
        return "dirty";
    }
    "up_to_date"
}

pub fn list_submodules(repo: &Repository) -> Result<Vec<SubmoduleInfo>, TwigError> {
    let Some(workdir) = repo.workdir() else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for sm in repo.submodules()? {
        let name = sm.name().unwrap_or("").to_string();
        let path = sm.path().to_string_lossy().to_string();
        let status = repo
            .submodule_status(&name, SubmoduleIgnore::None)
            .unwrap_or(SubmoduleStatus::WD_UNINITIALIZED);
        let head = sm.head_id();
        let wd = sm.workdir_id();
        out.push(SubmoduleInfo {
            abs_path: workdir.join(sm.path()).to_string_lossy().to_string(),
            url: sm.url().map(String::from),
            branch: sm.branch().map(String::from),
            head_oid: head.map(|o| o.to_string()),
            workdir_oid: wd.map(|o| o.to_string()),
            status: status_label(status, head, wd).to_string(),
            name,
            path,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// `git submodule update --init --recursive [-- <path>]`
pub async fn submodule_update(repo_path: &Path, path: Option<&str>) -> Result<GitOutput, TwigError> {
    let args = ["submodule", "update", "--init", "--recursive"];
    match path {
        Some(p) => {
            validate_rel_path(p)?;
            run_git_paths(repo_path, &args, &[p]).await
        }
        None => run_git(repo_path, &args).await,
    }
}

/// `git submodule sync --recursive [-- <path>]`: copy URLs from .gitmodules.
pub async fn submodule_sync(repo_path: &Path, path: Option<&str>) -> Result<GitOutput, TwigError> {
    let args = ["submodule", "sync", "--recursive"];
    match path {
        Some(p) => {
            validate_rel_path(p)?;
            run_git_paths(repo_path, &args, &[p]).await
        }
        None => run_git(repo_path, &args).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git_ok(dir: &Path, args: &[&str]) {
        let mut full = vec![
            "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false",
            "-c", "protocol.file.allow=always",
        ];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    async fn repo_with_commit(dir: &PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        git_ok(dir, &["init", "-q", "-b", "main"]).await;
        std::fs::write(dir.join("f.txt"), "f\n").unwrap();
        git_ok(dir, &["add", "."]).await;
        git_ok(dir, &["commit", "-q", "-m", "init"]).await;
    }

    #[tokio::test]
    async fn list_and_update_submodules() {
        let base = std::env::temp_dir().join(format!("twig-submod-{}", std::process::id()));
        let lib = base.join("lib");
        let sup = base.join("super");
        let clone = base.join("clone");
        repo_with_commit(&lib).await;
        repo_with_commit(&sup).await;
        let lib_url = lib.to_string_lossy().to_string();
        git_ok(&sup, &["submodule", "add", "-q", &lib_url, "deps/lib"]).await;
        git_ok(&sup, &["commit", "-q", "-m", "add sub"]).await;

        let _ = std::fs::remove_dir_all(&clone);
        let sup_url = sup.to_string_lossy().to_string();
        let clone_s = clone.to_string_lossy().to_string();
        git_ok(&base, &["clone", "-q", &sup_url, &clone_s]).await;

        let repo = Repository::open(&clone).unwrap();
        let list = list_submodules(&repo).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].path, "deps/lib");
        assert_eq!(list[0].status, "uninitialized");

        // Needs file:// transport allowed for the local clone.
        git_ok(&clone, &["config", "protocol.file.allow", "always"]).await;
        let out = run_git(
            &clone,
            &["-c", "protocol.file.allow=always", "submodule", "update", "--init", "--recursive", "--", "deps/lib"],
        )
        .await
        .unwrap();
        assert!(out.success, "{}", out.stderr);
        let list = list_submodules(&Repository::open(&clone).unwrap()).unwrap();
        assert_eq!(list[0].status, "up_to_date");
        assert_eq!(list[0].head_oid, list[0].workdir_oid);

        let out = submodule_sync(&clone, Some("deps/lib")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(submodule_update(&clone, Some("../x")).await.is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
