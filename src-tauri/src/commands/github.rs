use std::fs;
use std::path::PathBuf;

use git2::Repository;
use tauri::{Manager, State};
use tokio::process::Command;

use crate::commands::repo::RepoInfo;
use crate::commands::settings::AppSettings;
use crate::error::TwigError;
use crate::github::{self, GitHubPullRequest, GitHubRemoteInfo, GitHubRepo, GitHubUser, RepoListPage};
use crate::state::{AppState, OpenRepo};

// ── Helpers ──────────────────────────────────────────────────────────

fn get_token(app: &tauri::AppHandle) -> Result<String, TwigError> {
    let file = app
        .path()
        .app_data_dir()
        .map_err(|e| TwigError::Config(e.to_string()))?
        .join("settings.json");

    if !file.exists() {
        return Err(TwigError::GitHub(
            "No GitHub token configured. Set one in Settings > GitHub.".into(),
        ));
    }
    let json = fs::read_to_string(&file)?;
    let settings: AppSettings = serde_json::from_str(&json)?;
    match settings.github_token {
        Some(t) if !t.is_empty() => Ok(t),
        _ => Err(TwigError::GitHub(
            "No GitHub token configured. Set one in Settings > GitHub.".into(),
        )),
    }
}

fn build_client() -> Result<reqwest::Client, TwigError> {
    reqwest::Client::builder()
        .user_agent("Twig/0.1.0")
        .build()
        .map_err(|e| TwigError::Http(e.to_string()))
}

// ── Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn github_validate_token(app: tauri::AppHandle) -> Result<GitHubUser, TwigError> {
    let token = get_token(&app)?;
    let client = build_client()?;
    github::validate_token(&client, &token).await
}

#[tauri::command]
pub async fn github_list_repos(
    app: tauri::AppHandle,
    page: u32,
    per_page: u32,
    sort: String,
) -> Result<RepoListPage, TwigError> {
    let token = get_token(&app)?;
    let client = build_client()?;
    github::list_repos(&client, &token, page, per_page, &sort).await
}

#[tauri::command]
pub async fn github_clone_repo(
    state: State<'_, AppState>,
    clone_url: String,
    destination: String,
) -> Result<RepoInfo, TwigError> {
    let dest = PathBuf::from(&destination);

    let output = Command::new("git")
        .args(["clone", &clone_url, &destination])
        .output()
        .await
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git clone: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(TwigError::GitCli(format!("Clone failed: {stderr}")));
    }

    // Open the cloned repo in state (same logic as open_repo)
    let repo =
        Repository::discover(&dest).map_err(|_| TwigError::NotARepo(destination.clone()))?;

    let workdir = repo.workdir().unwrap_or(repo.path()).to_path_buf();
    let canonical = workdir.canonicalize().unwrap_or_else(|_| workdir.clone());
    let key = canonical.to_string_lossy().to_string();

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

    let last_commit_time = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.time().seconds())
        .unwrap_or(0);

    let info = RepoInfo {
        path: key.clone(),
        name,
        head_name,
        is_bare: repo.is_bare(),
        is_empty: repo.is_empty().unwrap_or(true),
        last_commit_time,
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

#[tauri::command]
pub async fn github_create_repo(
    app: tauri::AppHandle,
    name: String,
    description: Option<String>,
    private: bool,
    auto_init: bool,
) -> Result<GitHubRepo, TwigError> {
    let token = get_token(&app)?;
    let client = build_client()?;
    github::create_repo(
        &client,
        &token,
        &name,
        description.as_deref(),
        private,
        auto_init,
    )
    .await
}

#[tauri::command]
pub async fn github_detect_remote(
    state: State<'_, AppState>,
    path: String,
) -> Result<Option<GitHubRemoteInfo>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    let repo = &open.repository;
    let remotes = repo.remotes().map_err(TwigError::Git)?;

    // Prefer "origin", fall back to first GitHub remote found
    let mut result: Option<GitHubRemoteInfo> = None;

    for remote_name in remotes.iter().flatten() {
        if let Ok(remote) = repo.find_remote(remote_name) {
            if let Some(url) = remote.url() {
                if let Some((owner, repo_name)) = github::parse_github_remote(url) {
                    let info = GitHubRemoteInfo {
                        owner,
                        repo: repo_name,
                        remote_name: remote_name.to_string(),
                    };
                    if remote_name == "origin" {
                        return Ok(Some(info));
                    }
                    if result.is_none() {
                        result = Some(info);
                    }
                }
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn github_create_pull_request(
    app: tauri::AppHandle,
    owner: String,
    repo: String,
    title: String,
    body: String,
    head: String,
    base: String,
) -> Result<GitHubPullRequest, TwigError> {
    let token = get_token(&app)?;
    let client = build_client()?;
    github::create_pull_request(&client, &token, &owner, &repo, &title, &body, &head, &base).await
}

#[tauri::command]
pub async fn github_list_branches(
    app: tauri::AppHandle,
    owner: String,
    repo: String,
) -> Result<Vec<String>, TwigError> {
    let token = get_token(&app)?;
    let client = build_client()?;
    github::list_branches(&client, &token, &owner, &repo).await
}
