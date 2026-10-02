use std::path::{Path, PathBuf};

use git2::Repository;
use serde::Serialize;
use tauri::State;

use crate::error::TwigError;
use crate::state::{AppState, OpenRepo};

#[derive(Debug, Clone, Serialize)]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub head_name: Option<String>,
    pub is_bare: bool,
    pub is_empty: bool,
    /// Unix timestamp of the HEAD commit (0 if unavailable).
    pub last_commit_time: i64,
}

/// Display name for HEAD: the branch name, or a short OID when detached.
/// `None` for an unborn branch (empty repo).
fn head_display_name(repo: &Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if head.is_branch() {
        head.shorthand().map(String::from)
    } else {
        head.target().map(|o| {
            let mut s = o.to_string();
            s.truncate(7);
            s
        })
    }
}

/// Build `RepoInfo` for an opened repository.
pub(crate) fn build_repo_info(repo: &Repository, key: String, dir: &Path) -> RepoInfo {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    let last_commit_time = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.time().seconds())
        .unwrap_or(0);

    RepoInfo {
        path: key,
        name,
        head_name: head_display_name(repo),
        is_bare: repo.is_bare(),
        is_empty: repo.is_empty().unwrap_or(false),
        last_commit_time,
    }
}

/// Open a repository by path and add it to the app state.
#[tauri::command]
pub async fn open_repo(
    state: State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, TwigError> {
    let requested = path.clone();
    let (canonical, info) = tauri::async_runtime::spawn_blocking(move || {
        // Discover the git repository (handles opening from subdirectories)
        let repo = Repository::discover(PathBuf::from(&requested))
            .map_err(|_| TwigError::NotARepo(requested.clone()))?;

        let workdir = repo.workdir().unwrap_or(repo.path()).to_path_buf();
        let canonical = workdir.canonicalize().unwrap_or(workdir);
        let key = canonical.to_string_lossy().to_string();
        let info = build_repo_info(&repo, key, &canonical);
        Ok::<_, TwigError>((canonical, info))
    })
    .await
    .map_err(|e| TwigError::Task(e.to_string()))??;

    let mut repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    repos.insert(info.path.clone(), OpenRepo { path: canonical });

    Ok(info)
}

/// Open the repository at `dir` (just created or cloned) and track it.
pub(crate) async fn register_repo(state: &AppState, dir: PathBuf) -> Result<RepoInfo, TwigError> {
    let (canonical, info) = tauri::async_runtime::spawn_blocking(move || {
        let shown = dir.to_string_lossy().to_string();
        let repo = Repository::open(&dir).map_err(|_| TwigError::NotARepo(shown))?;
        let workdir = repo.workdir().unwrap_or(repo.path()).to_path_buf();
        let canonical = workdir.canonicalize().unwrap_or(workdir);
        let key = canonical.to_string_lossy().to_string();
        let info = build_repo_info(&repo, key, &canonical);
        Ok::<_, TwigError>((canonical, info))
    })
    .await
    .map_err(|e| TwigError::Task(e.to_string()))??;
    let mut repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    repos.insert(info.path.clone(), OpenRepo { path: canonical });
    Ok(info)
}

/// `git init` a folder (created if missing) and open it.
#[tauri::command]
pub async fn init_repository(
    state: State<'_, AppState>,
    path: String,
    initial_branch: Option<String>,
) -> Result<RepoInfo, TwigError> {
    let out = crate::git::create::init_repo(&path, initial_branch.as_deref()).await?;
    if !out.success {
        return Err(TwigError::GitCli(format!("git init failed: {}", out.stderr.trim())));
    }
    register_repo(&state, PathBuf::from(path)).await
}

/// Progress line of a running clone, emitted as `clone-progress`.
#[derive(Debug, Clone, Serialize)]
pub struct CloneProgress {
    pub op_id: u64,
    pub line: String,
}

/// Clone any URL into `destination` (absolute; missing or empty) and open it.
/// Progress lines are emitted as `clone-progress` events tagged with `op_id`.
/// HTTPS clones from the configured GitHub host use the stored token.
#[tauri::command]
pub async fn clone_repository(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    url: String,
    destination: String,
    op_id: u64,
) -> Result<RepoInfo, TwigError> {
    use tauri::Emitter;
    let env = crate::commands::github::clone_auth_env(&app, url.trim(), true).await;
    let emitter = app.clone();
    let out = crate::git::create::clone_repo(&url, &destination, &env, move |line| {
        let _ = emitter.emit("clone-progress", CloneProgress { op_id, line: line.to_string() });
    })
    .await?;
    if !out.success {
        return Err(TwigError::GitCli(format!("Clone failed: {}", out.stderr.trim())));
    }
    register_repo(&state, PathBuf::from(destination)).await
}

/// Close a repository and remove it from state.
#[tauri::command]
pub async fn close_repo(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), TwigError> {
    let mut repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    repos.remove(&path);
    Ok(())
}

/// Get info about an already-open repo (e.g. refresh head name after checkout).
#[tauri::command]
pub async fn get_repo_info(
    state: State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, TwigError> {
    let dir = state.repo_path(&path)?;
    state
        .read_repo(&path.clone(), move |repo| Ok(build_repo_info(repo, path, &dir)))
        .await
}

/// List all currently open repository paths.
#[tauri::command]
pub async fn list_open_repos(
    state: State<'_, AppState>,
) -> Result<Vec<String>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    Ok(repos.keys().cloned().collect())
}

/// Scan a directory for git repositories (immediate children only).
/// Returns RepoInfo for each discovered repo, sorted by most recent commit (newest first).
#[tauri::command]
pub async fn list_repos_in_dir(dir: String) -> Result<Vec<RepoInfo>, TwigError> {
    tauri::async_runtime::spawn_blocking(move || scan_repos(&PathBuf::from(dir)))
        .await
        .map_err(|e| TwigError::Task(e.to_string()))?
}

fn scan_repos(dir_path: &Path) -> Result<Vec<RepoInfo>, TwigError> {
    if !dir_path.is_dir() {
        return Ok(vec![]);
    }

    let mut repos: Vec<RepoInfo> = std::fs::read_dir(dir_path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .filter_map(|entry| {
            let path = entry.path();
            let repo = Repository::open(&path).ok()?;
            let canonical = path.canonicalize().unwrap_or(path);
            let key = canonical.to_string_lossy().to_string();
            Some(build_repo_info(&repo, key, &canonical))
        })
        .collect();

    repos.sort_by_key(|a| std::cmp::Reverse(a.last_commit_time));

    Ok(repos)
}
