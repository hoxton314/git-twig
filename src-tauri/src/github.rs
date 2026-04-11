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

const API_BASE: &str = "https://api.github.com";

fn auth_headers(
    builder: reqwest::RequestBuilder,
    token: &str,
) -> reqwest::RequestBuilder {
    builder
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
}

async fn check_response(response: reqwest::Response) -> Result<reqwest::Response, TwigError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    match status.as_u16() {
        401 => Err(TwigError::GitHub(
            "Invalid or expired GitHub token. Please update it in Settings.".into(),
        )),
        403 => Err(TwigError::GitHub(format!("GitHub access denied: {body}"))),
        404 => Err(TwigError::GitHub(format!(
            "GitHub resource not found: {body}"
        ))),
        422 => Err(TwigError::GitHub(format!("Validation failed: {body}"))),
        _ => Err(TwigError::GitHub(format!(
            "GitHub API error ({status}): {body}"
        ))),
    }
}

/// Check whether the `Link` header contains a `rel="next"` entry.
fn has_next_link(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get("link")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("rel=\"next\""))
        .unwrap_or(false)
}

// ── API functions ────────────────────────────────────────────────────

pub async fn validate_token(client: &Client, token: &str) -> Result<GitHubUser, TwigError> {
    let resp = auth_headers(client.get(format!("{API_BASE}/user")), token)
        .send()
        .await?;
    let resp = check_response(resp).await?;
    Ok(resp.json().await?)
}

pub async fn list_repos(
    client: &Client,
    token: &str,
    page: u32,
    per_page: u32,
    sort: &str,
) -> Result<RepoListPage, TwigError> {
    let resp = auth_headers(
        client.get(format!(
            "{API_BASE}/user/repos?page={page}&per_page={per_page}&sort={sort}&affiliation=owner,collaborator,organization_member"
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
    let resp = auth_headers(client.post(format!("{API_BASE}/user/repos")), token)
        .json(&body)
        .send()
        .await?;
    let resp = check_response(resp).await?;
    Ok(resp.json().await?)
}

pub async fn create_pull_request(
    client: &Client,
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
        client.post(format!("{API_BASE}/repos/{owner}/{repo}/pulls")),
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

pub async fn list_branches(
    client: &Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<String>, TwigError> {
    let resp = auth_headers(
        client.get(format!(
            "{API_BASE}/repos/{owner}/{repo}/branches?per_page=100"
        )),
        token,
    )
    .send()
    .await?;
    let resp = check_response(resp).await?;
    let branches: Vec<BranchEntry> = resp.json().await?;
    Ok(branches.into_iter().map(|b| b.name).collect())
}

// ── Remote URL parsing ───────────────────────────────────────────────

/// Parse a GitHub owner/repo from a remote URL.
/// Supports HTTPS (`https://github.com/owner/repo.git`) and
/// SSH (`git@github.com:owner/repo.git`).
pub fn parse_github_remote(url: &str) -> Option<(String, String)> {
    // HTTPS
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let rest = rest.trim_end_matches(".git").trim_end_matches('/');
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    // SSH
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let rest = rest.trim_end_matches(".git").trim_end_matches('/');
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}
