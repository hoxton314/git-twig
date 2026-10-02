use std::path::PathBuf;

use base64::Engine;
use git2::Repository;
use tauri::State;
use tokio::process::Command;

use crate::commands::repo::{build_repo_info, RepoInfo};
use crate::commands::settings::read_legacy_token;
use crate::credentials;
use crate::error::TwigError;
use crate::github::{self, GitHubPullRequest, GitHubRemoteInfo, GitHubRepo, GitHubUser, RepoListPage};
use crate::state::{AppState, OpenRepo};

// ── Helpers ──────────────────────────────────────────────────────────

async fn get_token(app: &tauri::AppHandle) -> Result<String, TwigError> {
    let stored = credentials::get_github_token().await;
    if let Ok(Some(token)) = stored {
        return Ok(token);
    }
    // Fall back to a token not yet migrated out of settings.json
    // (e.g. the keyring was unavailable at startup).
    if let Some(token) = read_legacy_token(app) {
        return Ok(token);
    }
    match stored {
        Err(e) => Err(e),
        _ => Err(TwigError::GitHub(
            "No GitHub token configured. Set one in Settings > GitHub.".into(),
        )),
    }
}

/// Store (or with `None`/empty, remove) the GitHub token in the OS keyring.
#[tauri::command]
pub async fn github_set_token(token: Option<String>) -> Result<(), TwigError> {
    credentials::set_github_token(token.map(|t| t.trim().to_string())).await
}

/// Whether a GitHub token is configured. The token itself never leaves Rust.
#[tauri::command]
pub async fn github_has_token(app: tauri::AppHandle) -> Result<bool, TwigError> {
    Ok(get_token(&app).await.is_ok())
}

fn build_client() -> Result<reqwest::Client, TwigError> {
    reqwest::Client::builder()
        .user_agent(concat!("Twig/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| TwigError::Http(e.to_string()))
}

// ── Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn github_validate_token(app: tauri::AppHandle) -> Result<GitHubUser, TwigError> {
    let token = get_token(&app).await?;
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
    let token = get_token(&app).await?;
    let client = build_client()?;
    github::list_repos(&client, &token, page, per_page, &sort).await
}

#[tauri::command]
pub async fn github_clone_repo(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    clone_url: String,
    destination: String,
) -> Result<RepoInfo, TwigError> {
    let dest = PathBuf::from(&destination);

    // Guard against arguments git would treat as flags (`--` ends option parsing).
    if clone_url.starts_with('-') || destination.starts_with('-') {
        return Err(TwigError::GitCli(
            "clone URL and destination must not start with '-'".to_string(),
        ));
    }
    // A relative destination would resolve against the app's working
    // directory (wherever Twig was launched from), not anywhere meaningful.
    if !dest.is_absolute() {
        return Err(TwigError::GitCli(
            "Clone destination must be an absolute path".to_string(),
        ));
    }

    let mut cmd = Command::new("git");
    cmd.args(["clone", "--", &clone_url, &destination])
        // Never block on an interactive username/password prompt.
        .env("GIT_TERMINAL_PROMPT", "0");

    // Authenticate HTTPS clones of GitHub repos (needed for private repos)
    // with the configured PAT. It is passed as a one-off http.extraHeader via
    // GIT_CONFIG_* env vars so it is neither visible in the process list nor
    // persisted into the clone's .git/config remote URL.
    if clone_url.to_ascii_lowercase().starts_with("https://github.com/") {
        if let Ok(token) = get_token(&app).await {
            let basic = base64::engine::general_purpose::STANDARD
                .encode(format!("x-access-token:{token}"));
            cmd.env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "http.https://github.com/.extraheader")
                .env("GIT_CONFIG_VALUE_0", format!("AUTHORIZATION: basic {basic}"));
        }
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git clone: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(TwigError::GitCli(format!("Clone failed: {stderr}")));
    }

    // Open the cloned repo in state (same logic as open_repo)
    let repo = Repository::open(&dest).map_err(|_| TwigError::NotARepo(destination.clone()))?;

    let workdir = repo.workdir().unwrap_or(repo.path()).to_path_buf();
    let canonical = workdir.canonicalize().unwrap_or_else(|_| workdir.clone());
    let key = canonical.to_string_lossy().to_string();
    let info = build_repo_info(&repo, key.clone(), &canonical);

    let mut repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    repos.insert(key, OpenRepo { path: canonical });

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
    let token = get_token(&app).await?;
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
    state.read_repo(&path, |repo| {
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
    })
    .await
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
    let token = get_token(&app).await?;
    let client = build_client()?;
    github::create_pull_request(&client, &token, &owner, &repo, &title, &body, &head, &base).await
}

#[tauri::command]
pub async fn github_list_branches(
    app: tauri::AppHandle,
    owner: String,
    repo: String,
) -> Result<Vec<String>, TwigError> {
    let token = get_token(&app).await?;
    let client = build_client()?;
    github::list_branches(&client, &token, &owner, &repo).await
}
