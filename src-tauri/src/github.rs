//! GitHub REST API v3 client.
//! Pure HTTP layer — no Tauri dependencies so it stays testable in isolation.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::TwigError;

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubOwner {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub id: u64,
    pub full_name: String,
    pub name: String,
    pub owner: GitHubOwner,
    pub description: Option<String>,
    pub private: bool,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    pub default_branch: String,
    pub stargazers_count: u32,
    pub updated_at: String,
    pub fork: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPullRequest {
    pub number: u64,
    pub html_url: String,
    pub title: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoListPage {
    pub repos: Vec<GitHubRepo>,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitHubRemoteInfo {
    pub owner: String,
    pub repo: String,
    pub remote_name: String,
}

// ── Helpers ──────────────────────────────────────────────────────────

pub(crate) fn auth_headers(
    builder: reqwest::RequestBuilder,
    token: &str,
) -> reqwest::RequestBuilder {
    builder
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
}

pub(crate) async fn check_response(response: reqwest::Response) -> Result<reqwest::Response, TwigError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let headers = response.headers().clone();
    let raw = response.text().await.unwrap_or_default();
    // GitHub error bodies are JSON like {"message": "...", "errors": [...]}.
    // Surface the human-readable message instead of the raw JSON blob.
    let body = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| {
            let msg = v.get("message")?.as_str()?.to_string();
            let details: Vec<String> = v
                .get("errors")
                .and_then(|e| e.as_array())
                .map(|errs| {
                    errs.iter()
                        .filter_map(|e| {
                            e.get("message")
                                .and_then(|m| m.as_str())
                                .map(String::from)
                                .or_else(|| e.as_str().map(String::from))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(if details.is_empty() {
                msg
            } else {
                format!("{msg} ({})", details.join("; "))
            })
        })
        .unwrap_or(raw);

    let rate_limited = headers
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim() == "0");
    if (status.as_u16() == 403 || status.as_u16() == 429) && rate_limited {
        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .and_then(|reset| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_secs();
                Some(reset.saturating_sub(now).div_ceil(60).max(1))
            });
        return Err(TwigError::GitHub(match reset {
            Some(mins) => format!("GitHub API rate limit exceeded. Try again in ~{mins} min."),
            None => "GitHub API rate limit exceeded. Try again later.".into(),
        }));
    }

    match status.as_u16() {
        401 => Err(TwigError::GitHub(
            "Invalid or expired GitHub token. Please update it in Settings.".into(),
        )),
        403 => Err(TwigError::GitHub(format!("GitHub access denied: {body}"))),
        404 => Err(TwigError::GitHub(format!(
            "GitHub resource not found (or token lacks access): {body}"
        ))),
        422 => Err(TwigError::GitHub(format!("Validation failed: {body}"))),
        429 => Err(TwigError::GitHub(format!(
            "GitHub rate limit hit, try again later: {body}"
        ))),
        _ => Err(TwigError::GitHub(format!(
            "GitHub API error ({status}): {body}"
        ))),
    }
}

/// Check whether the `Link` header contains a `rel="next"` entry.
pub(crate) fn has_next_link(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get("link")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("rel=\"next\""))
        .unwrap_or(false)
}

// ── API functions ────────────────────────────────────────────────────

pub async fn validate_token(client: &Client, api: &str, token: &str) -> Result<GitHubUser, TwigError> {
    let resp = auth_headers(client.get(format!("{api}/user")), token)
        .send()
        .await?;
    let resp = check_response(resp).await?;
    Ok(resp.json().await?)
}

pub async fn list_repos(
    client: &Client,
    api: &str,
    token: &str,
    page: u32,
    per_page: u32,
    sort: &str,
) -> Result<RepoListPage, TwigError> {
    let resp = auth_headers(
        client.get(format!(
            "{api}/user/repos?page={page}&per_page={per_page}&sort={sort}&affiliation=owner,collaborator,organization_member"
        )),
        token,
    )
    .send()
    .await?;
    let resp = check_response(resp).await?;
    let has_next = has_next_link(resp.headers());
    let repos: Vec<GitHubRepo> = resp.json().await?;
    Ok(RepoListPage {
        repos,
        has_next_page: has_next,
    })
}

pub async fn create_repo(
    client: &Client,
    api: &str,
    token: &str,
    name: &str,
    description: Option<&str>,
    private: bool,
    auto_init: bool,
) -> Result<GitHubRepo, TwigError> {
    let body = serde_json::json!({
        "name": name,
        "description": description.unwrap_or(""),
        "private": private,
        "auto_init": auto_init,
    });
    let resp = auth_headers(client.post(format!("{api}/user/repos")), token)
        .json(&body)
        .send()
        .await?;
    let resp = check_response(resp).await?;
    Ok(resp.json().await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_pull_request(
    client: &Client,
    api: &str,
    token: &str,
    owner: &str,
    repo: &str,
    title: &str,
    body: &str,
    head: &str,
    base: &str,
) -> Result<GitHubPullRequest, TwigError> {
    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "head": head,
        "base": base,
    });
    let resp = auth_headers(
        client.post(format!("{api}/repos/{owner}/{repo}/pulls")),
        token,
    )
    .json(&payload)
    .send()
    .await?;
    let resp = check_response(resp).await?;
    Ok(resp.json().await?)
}

#[derive(Debug, Deserialize)]
struct BranchEntry {
    name: String,
}

/// Upper bound on pages fetched for branch listings (100 per page).
const MAX_BRANCH_PAGES: u32 = 20;

pub async fn list_branches(
    client: &Client,
    api: &str,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<String>, TwigError> {
    let mut names = Vec::new();
    for page in 1..=MAX_BRANCH_PAGES {
        let resp = auth_headers(
            client.get(format!(
                "{api}/repos/{owner}/{repo}/branches?per_page=100&page={page}"
            )),
            token,
        )
        .send()
        .await?;
        let resp = check_response(resp).await?;
        let has_next = has_next_link(resp.headers());
        let branches: Vec<BranchEntry> = resp.json().await?;
        names.extend(branches.into_iter().map(|b| b.name));
        if !has_next {
            break;
        }
    }
    Ok(names)
}

// Remote URL parsing lives in `hosting::remote` (it also handles GitHub
// Enterprise hosts, GitLab and Gitea).
