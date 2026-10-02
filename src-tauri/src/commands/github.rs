use std::path::PathBuf;

use tauri::State;

use crate::commands::repo::RepoInfo;
use crate::commands::settings::read_legacy_token;
use crate::credentials;
use crate::error::TwigError;
use crate::github::{self, GitHubPullRequest, GitHubRemoteInfo, GitHubRepo, GitHubUser, RepoListPage};
use crate::hosting::config::{self as hosting_config, GitHubEndpoint};
use crate::hosting::{net_auth, remote as hosting_remote};
use crate::state::AppState;

// ── Helpers ──────────────────────────────────────────────────────────

pub(crate) async fn get_token(app: &tauri::AppHandle) -> Result<String, TwigError> {
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

pub(crate) fn build_client() -> Result<reqwest::Client, TwigError> {
    crate::hosting::http::build_client()
}

/// The configured GitHub / GitHub Enterprise endpoint.
pub(crate) fn endpoint(app: &tauri::AppHandle) -> GitHubEndpoint {
    hosting_config::load(app).github
}

// ── Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn github_validate_token(app: tauri::AppHandle) -> Result<GitHubUser, TwigError> {
    let token = get_token(&app).await?;
    let client = build_client()?;
    github::validate_token(&client, &endpoint(&app).api_base, &token).await
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
    github::list_repos(&client, &endpoint(&app).api_base, &token, page, per_page, &sort).await
}

/// Auth environment for cloning `url`: the GitHub token as a host-scoped
/// `http.extraHeader` (via `GIT_CONFIG_*` env vars, never persisted into the
/// clone's config) when `url` is HTTPS on the configured GitHub host.
/// `respect_setting`: only when "Use token for HTTPS git operations" is on
/// (the generic clone); the GitHub picker clones repos the API listed for
/// this token, so it always authenticates.
pub(crate) async fn clone_auth_env(
    app: &tauri::AppHandle,
    url: &str,
    respect_setting: bool,
) -> Vec<(String, String)> {
    if respect_setting && !hosting_config::load(app).github_https_auth {
        return Vec::new();
    }
    let host = endpoint(app).host;
    if hosting_remote::is_https_on_host(url, &host) {
        if let Ok(token) = get_token(app).await {
            return net_auth::auth_env_for(&host, &token);
        }
    }
    Vec::new()
}

#[tauri::command]
pub async fn github_clone_repo(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    clone_url: String,
    destination: String,
) -> Result<RepoInfo, TwigError> {
    let env = clone_auth_env(&app, clone_url.trim(), false).await;
    let out = crate::git::create::clone_repo(&clone_url, &destination, &env, |_| {}).await?;
    if !out.success {
        return Err(TwigError::GitCli(format!("Clone failed: {}", out.stderr.trim())));
    }
    crate::commands::repo::register_repo(&state, PathBuf::from(destination)).await
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
        &endpoint(&app).api_base,
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Option<GitHubRemoteInfo>, TwigError> {
    let host = endpoint(&app).host;
    state.read_repo(&path, move |repo| {
        let remotes = repo.remotes().map_err(TwigError::Git)?;

        // Prefer "origin", fall back to first GitHub remote found
        let mut result: Option<GitHubRemoteInfo> = None;

        for remote_name in remotes.iter().flatten() {
            if let Ok(remote) = repo.find_remote(remote_name) {
                if let Some(url) = remote.url() {
                    if let Some((owner, repo_name)) = hosting_remote::parse_github_remote(url, &host) {
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
    github::create_pull_request(&client, &endpoint(&app).api_base, &token, &owner, &repo, &title, &body, &head, &base).await
}

#[tauri::command]
pub async fn github_list_branches(
    app: tauri::AppHandle,
    owner: String,
    repo: String,
) -> Result<Vec<String>, TwigError> {
    let token = get_token(&app).await?;
    let client = build_client()?;
    github::list_branches(&client, &endpoint(&app).api_base, &token, &owner, &repo).await
}
