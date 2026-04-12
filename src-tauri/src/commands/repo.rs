use std::path::PathBuf;

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
}

/// Open a repository by path and add it to the app state.
#[tauri::command]
pub async fn open_repo(
    state: State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, TwigError> {
    let repo_path = PathBuf::from(&path);

    // Discover the git repository (handles opening from subdirectories)
    let repo = Repository::discover(&repo_path)
        .map_err(|_| TwigError::NotARepo(path.clone()))?;

    let workdir = repo
        .workdir()
        .unwrap_or(repo.path())
        .to_path_buf();

    let canonical = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.clone());

    let key = canonical.to_string_lossy().to_string();

    let head_name = repo
        .head()
        .ok()
        .and_then(|h| {
            if h.is_branch() {
                h.shorthand().map(String::from)
            } else {
                // Detached HEAD — show short OID
                h.target().map(|o| o.to_string()[..7].to_string())
            }
        });

    let name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    let info = RepoInfo {
        path: key.clone(),
        name,
        head_name,
        is_bare: repo.is_bare(),
        is_empty: repo.is_empty().unwrap_or(true),
    };

    let mut repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    repos.insert(
        key,
        OpenRepo {
            repository: repo,
            path: canonical,
        },
    );

    Ok(info)
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
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    let repo = &open.repository;

    let head_name = repo
        .head()
        .ok()
        .and_then(|h| {
            if h.is_branch() {
                h.shorthand().map(String::from)
            } else {
                h.target().map(|o| o.to_string()[..7].to_string())
            }
        });

    let name = open
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    Ok(RepoInfo {
        path,
        name,
        head_name,
        is_bare: repo.is_bare(),
        is_empty: repo.is_empty().unwrap_or(true),
    })
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
    let dir_path = PathBuf::from(&dir);
    if !dir_path.is_dir() {
        return Ok(vec![]);
    }

    let entries: Vec<_> = std::fs::read_dir(&dir_path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect();

    let mut repos: Vec<(i64, RepoInfo)> = entries
        .into_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let repo = Repository::open(&path).ok()?;
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

            let head_time = repo
                .head()
                .ok()
                .and_then(|h| h.peel_to_commit().ok())
                .map(|c| c.time().seconds())
                .unwrap_or(0);

            let head_name = repo.head().ok().and_then(|h| {
                if h.is_branch() {
                    h.shorthand().map(String::from)
                } else {
                    h.target().map(|o| o.to_string()[..7].to_string())
                }
            });

            let name = canonical
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "repo".to_string());

            Some((head_time, RepoInfo {
                path: canonical.to_string_lossy().to_string(),
                name,
                head_name,
                is_bare: repo.is_bare(),
                is_empty: repo.is_empty().unwrap_or(true),
            }))
        })
        .collect();

    repos.sort_by(|a, b| b.0.cmp(&a.0));

    Ok(repos.into_iter().map(|(_, info)| info).collect())
}
