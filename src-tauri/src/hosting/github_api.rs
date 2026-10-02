//! GitHub pull requests, CI status and OAuth device flow.
//! Works against github.com and GitHub Enterprise Server via `GitHubEndpoint`.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::config::GitHubEndpoint;
use super::patch::parse_unified_patch;
use super::types::{rollup, CiCheck, CiStatus, PrDetail, PrFile, PrLabel, PrPage, PrSummary};
use crate::error::TwigError;
use crate::github::{auth_headers, check_response, has_next_link};

// ── Pull request list (GraphQL: one round trip incl. reviews + CI) ────

const PR_LIST_QUERY: &str = r#"
query($owner: String!, $name: String!, $states: [PullRequestState!], $first: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequests(states: $states, first: $first, after: $after, orderBy: {field: UPDATED_AT, direction: DESC}) {
      pageInfo { hasNextPage endCursor }
      nodes {
        number title state isDraft url createdAt updatedAt
        author { login avatarUrl }
        headRefName headRefOid baseRefName isCrossRepository
        headRepository { nameWithOwner }
        reviewDecision
        comments { totalCount }
        labels(first: 10) { nodes { name color } }
        commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
      }
    }
  }
}"#;

/// GraphQL `PullRequestState` list for a UI filter.
fn states_for_filter(filter: &str) -> Value {
    match filter {
        "closed" => serde_json::json!(["CLOSED", "MERGED"]),
        "merged" => serde_json::json!(["MERGED"]),
        "all" => Value::Null,
        _ => serde_json::json!(["OPEN"]),
    }
}

fn s(v: &Value, ptr: &str) -> String {
    v.pointer(ptr).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn opt_s(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr).and_then(Value::as_str).map(String::from)
}

fn map_rollup_state(state: &str) -> String {
    match state {
        "SUCCESS" => "success",
        "FAILURE" | "ERROR" => "failure",
        "PENDING" | "EXPECTED" => "pending",
        _ => "none",
    }
    .to_string()
}

fn map_review_decision(d: &str) -> Option<String> {
    match d {
        "APPROVED" => Some("approved".into()),
        "CHANGES_REQUESTED" => Some("changes_requested".into()),
        "REVIEW_REQUIRED" => Some("review_required".into()),
        _ => None,
    }
}

/// Map a GraphQL `pullRequests` response into a page of summaries.
pub fn map_graphql_prs(resp: &Value) -> Result<PrPage, TwigError> {
    if let Some(errors) = resp.get("errors").and_then(Value::as_array) {
        let msg = errors
            .iter()
            .filter_map(|e| e.get("message").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("; ");
        if !msg.is_empty() {
            return Err(TwigError::GitHub(msg));
        }
    }
    let conn = resp
        .pointer("/data/repository/pullRequests")
        .ok_or_else(|| TwigError::GitHub("Repository not found or not accessible".into()))?;
    let items = conn
        .get("nodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter(|n| !n.is_null())
                .map(|n| {
                    let cross = n.get("isCrossRepository").and_then(Value::as_bool).unwrap_or(false);
                    PrSummary {
                        number: n.get("number").and_then(Value::as_u64).unwrap_or(0),
                        title: s(n, "/title"),
                        state: s(n, "/state").to_ascii_lowercase(),
                        draft: n.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
                        // Deleted accounts come back as `author: null`.
                        author: opt_s(n, "/author/login").unwrap_or_else(|| "ghost".into()),
                        author_avatar: opt_s(n, "/author/avatarUrl"),
                        head_ref: s(n, "/headRefName"),
                        head_sha: s(n, "/headRefOid"),
                        head_repo: if cross {
                            Some(opt_s(n, "/headRepository/nameWithOwner").unwrap_or_else(|| "(deleted fork)".into()))
                        } else {
                            None
                        },
                        base_ref: s(n, "/baseRefName"),
                        html_url: s(n, "/url"),
                        created_at: s(n, "/createdAt"),
                        updated_at: s(n, "/updatedAt"),
                        comments: n
                            .pointer("/comments/totalCount")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as u32,
                        review_state: opt_s(n, "/reviewDecision").and_then(|d| map_review_decision(&d)),
                        ci_state: n
                            .pointer("/commits/nodes/0/commit/statusCheckRollup/state")
                            .and_then(Value::as_str)
                            .map(map_rollup_state)
                            .or_else(|| Some("none".into())),
                        labels: n
                            .pointer("/labels/nodes")
                            .and_then(Value::as_array)
                            .map(|ls| {
                                ls.iter()
                                    .map(|l| PrLabel {
                                        name: s(l, "/name"),
                                        color: opt_s(l, "/color"),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let has_next = conn
        .pointer("/pageInfo/hasNextPage")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(PrPage {
        items,
        next_cursor: if has_next {
            opt_s(conn, "/pageInfo/endCursor")
        } else {
            None
        },
    })
}

pub async fn list_pull_requests(
    client: &Client,
    ep: &GitHubEndpoint,
    token: &str,
    owner: &str,
    repo: &str,
    filter: &str,
    cursor: Option<&str>,
) -> Result<PrPage, TwigError> {
    let body = serde_json::json!({
        "query": PR_LIST_QUERY,
        "variables": {
            "owner": owner,
            "name": repo,
            "states": states_for_filter(filter),
            "first": 30,
            "after": cursor,
        }
    });
    let resp = auth_headers(client.post(&ep.graphql_url), token)
        .json(&body)
        .send()
        .await?;
    let resp = check_response(resp).await?;
    let value: Value = resp.json().await?;
    map_graphql_prs(&value)
}

// ── Pull request detail + files (REST) ──────────────────────────────

/// Map a REST `pulls/{n}` object.
pub fn map_rest_pr(v: &Value) -> PrDetail {
    let merged = v.get("merged").and_then(Value::as_bool).unwrap_or(false)
        || v.get("merged_at").is_some_and(|m| !m.is_null());
    let state = if merged { "merged".to_string() } else { s(v, "/state") };
    let base_repo = opt_s(v, "/base/repo/full_name");
    let head_repo = opt_s(v, "/head/repo/full_name");
    let cross = match (&head_repo, &base_repo) {
        (Some(h), Some(b)) => !h.eq_ignore_ascii_case(b),
        (None, _) => true, // fork deleted
        _ => false,
    };
    let u32_at = |key: &str| v.get(key).and_then(Value::as_u64).map(|n| n as u32);
    PrDetail {
        summary: PrSummary {
            number: v.get("number").and_then(Value::as_u64).unwrap_or(0),
            title: s(v, "/title"),
            state,
            draft: v.get("draft").and_then(Value::as_bool).unwrap_or(false),
            author: opt_s(v, "/user/login").unwrap_or_else(|| "ghost".into()),
            author_avatar: opt_s(v, "/user/avatar_url"),
            head_ref: s(v, "/head/ref"),
            head_sha: s(v, "/head/sha"),
            head_repo: if cross {
                Some(head_repo.unwrap_or_else(|| "(deleted fork)".into()))
            } else {
                None
            },
            base_ref: s(v, "/base/ref"),
            html_url: s(v, "/html_url"),
            created_at: s(v, "/created_at"),
            updated_at: s(v, "/updated_at"),
            comments: u32_at("comments").unwrap_or(0) + u32_at("review_comments").unwrap_or(0),
            review_state: None,
            ci_state: None,
            labels: v
                .get("labels")
                .and_then(Value::as_array)
                .map(|ls| {
                    ls.iter()
                        .map(|l| PrLabel {
                            name: s(l, "/name"),
                            color: opt_s(l, "/color"),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        body: opt_s(v, "/body").unwrap_or_default(),
        additions: u32_at("additions"),
        deletions: u32_at("deletions"),
        changed_files: u32_at("changed_files"),
        commits: u32_at("commits"),
        mergeable_state: opt_s(v, "/mergeable_state"),
    }
}

pub async fn get_pull_request(
    client: &Client,
    ep: &GitHubEndpoint,
    token: &str,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PrDetail, TwigError> {
    let url = format!("{}/repos/{owner}/{repo}/pulls/{number}", ep.api_base);
    let resp = auth_headers(client.get(url), token).send().await?;
    let resp = check_response(resp).await?;
    let v: Value = resp.json().await?;
    Ok(map_rest_pr(&v))
}

#[derive(Debug, Deserialize)]
pub struct RestPrFile {
    pub filename: String,
    #[serde(default)]
    pub previous_filename: Option<String>,
    pub status: String,
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(default)]
    pub patch: Option<String>,
}

pub fn map_pr_file(f: RestPrFile) -> PrFile {
    let status = match f.status.as_str() {
        "removed" => "deleted",
        "renamed" | "copied" => "renamed",
        "added" => "added",
        _ => "modified",
    }
    .to_string();
    let hunks = f.patch.as_deref().map(parse_unified_patch).unwrap_or_default();
    PrFile {
        patch_missing: hunks.is_empty() && (f.additions + f.deletions > 0 || f.patch.is_none()),
        path: f.filename,
        old_path: f.previous_filename,
        status,
        additions: f.additions,
        deletions: f.deletions,
        hunks,
    }
}

/// GitHub caps the files listing at 3000 entries (30 pages of 100).
const MAX_FILE_PAGES: u32 = 30;

pub async fn list_pull_request_files(
    client: &Client,
    ep: &GitHubEndpoint,
    token: &str,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<Vec<PrFile>, TwigError> {
    let mut out = Vec::new();
    for page in 1..=MAX_FILE_PAGES {
        let url = format!(
            "{}/repos/{owner}/{repo}/pulls/{number}/files?per_page=100&page={page}",
            ep.api_base
        );
        let resp = auth_headers(client.get(url), token).send().await?;
        let resp = check_response(resp).await?;
        let has_next = has_next_link(resp.headers());
        let files: Vec<RestPrFile> = resp.json().await?;
        out.extend(files.into_iter().map(map_pr_file));
        if !has_next {
            break;
        }
    }
    Ok(out)
}

// ── CI status (combined status + check runs) ────────────────────────

fn map_status_state(state: &str) -> &'static str {
    match state {
        "success" => "success",
        "failure" | "error" => "failure",
        _ => "pending",
    }
}

fn map_check_run(run: &Value) -> CiCheck {
    let status = s(run, "/status");
    let state = if status != "completed" {
        "pending"
    } else {
        match s(run, "/conclusion").as_str() {
            "success" => "success",
            "failure" | "timed_out" | "action_required" | "cancelled" | "startup_failure" => "failure",
            _ => "neutral", // neutral, skipped, stale
        }
    };
    CiCheck {
        name: s(run, "/name"),
        state: state.into(),
        description: opt_s(run, "/output/title").filter(|t| !t.is_empty()),
        url: opt_s(run, "/html_url").or_else(|| opt_s(run, "/details_url")),
    }
}

/// Combine a combined-status response and a check-runs response (either
/// may be absent, e.g. when a fine-grained token lacks the permission).
pub fn map_ci(sha: &str, combined: Option<&Value>, check_runs: Option<&Value>) -> CiStatus {
    let mut checks: Vec<CiCheck> = Vec::new();
    if let Some(statuses) = combined.and_then(|c| c.get("statuses")).and_then(Value::as_array) {
        for st in statuses {
            checks.push(CiCheck {
                name: s(st, "/context"),
                state: map_status_state(&s(st, "/state")).into(),
                description: opt_s(st, "/description").filter(|d| !d.is_empty()),
                url: opt_s(st, "/target_url").filter(|u| !u.is_empty()),
            });
        }
    }
    if let Some(runs) = check_runs.and_then(|c| c.get("check_runs")).and_then(Value::as_array) {
        checks.extend(runs.iter().map(map_check_run));
    }
    CiStatus {
        sha: sha.to_string(),
        state: rollup(&checks),
        checks,
    }
}

async fn get_json(client: &Client, url: String, token: &str) -> Result<Value, TwigError> {
    let resp = auth_headers(client.get(url), token).send().await?;
    let resp = check_response(resp).await?;
    Ok(resp.json().await?)
}

pub async fn ci_status(
    client: &Client,
    ep: &GitHubEndpoint,
    token: &str,
    owner: &str,
    repo: &str,
    sha: &str,
) -> Result<CiStatus, TwigError> {
    let base = format!("{}/repos/{owner}/{repo}/commits/{sha}", ep.api_base);
    let (combined, runs) = tokio::join!(
        get_json(client, format!("{base}/status?per_page=100"), token),
        get_json(client, format!("{base}/check-runs?per_page=100"), token),
    );
    match (combined, runs) {
        (Err(e), Err(_)) => Err(e),
        (c, r) => Ok(map_ci(sha, c.ok().as_ref(), r.ok().as_ref())),
    }
}

// ── OAuth device flow ───────────────────────────────────────────────

/// Client ID of the GitHub OAuth App used for "Sign in with GitHub".
///
/// TODO(maintainer): register an OAuth App at
/// https://github.com/settings/applications/new (any homepage/callback
/// URL works), tick "Enable Device Flow", and paste its Client ID here.
/// The device flow needs no client secret. While this is empty the
/// sign-in button is hidden and only the personal-access-token flow is
/// offered.
pub const GITHUB_OAUTH_CLIENT_ID: &str = "";

/// Scopes requested by the device flow (private repos + org membership).
pub const OAUTH_SCOPES: &str = "repo read:org";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

/// What the UI sees of a started flow (the device code stays in Rust).
#[derive(Debug, Clone, Serialize)]
pub struct DeviceFlowStart {
    pub flow_id: u64,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PollOutcome {
    Token(String),
    Pending,
    SlowDown(Option<u64>),
    Failed(String),
}

pub fn map_poll_response(v: &Value) -> PollOutcome {
    if let Some(tok) = v.get("access_token").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        return PollOutcome::Token(tok.to_string());
    }
    match v.get("error").and_then(Value::as_str).unwrap_or("") {
        "authorization_pending" => PollOutcome::Pending,
        "slow_down" => PollOutcome::SlowDown(v.get("interval").and_then(Value::as_u64)),
        "expired_token" => PollOutcome::Failed("The sign-in code expired. Please start again.".into()),
        "access_denied" => PollOutcome::Failed("Sign-in was cancelled on GitHub.".into()),
        "device_flow_disabled" => PollOutcome::Failed(
            "Device flow is not enabled for Twig's OAuth app.".into(),
        ),
        other => PollOutcome::Failed(
            opt_s(v, "/error_description").unwrap_or_else(|| {
                if other.is_empty() {
                    "Unexpected response from GitHub".into()
                } else {
                    other.to_string()
                }
            }),
        ),
    }
}

pub async fn request_device_code(client: &Client, ep: &GitHubEndpoint) -> Result<DeviceCode, TwigError> {
    let resp = client
        .post(format!("{}/login/device/code", ep.web_base))
        .header("Accept", "application/json")
        .form(&[("client_id", GITHUB_OAUTH_CLIENT_ID), ("scope", OAUTH_SCOPES)])
        .send()
        .await?;
    let resp = check_response(resp).await?;
    let v: Value = resp.json().await?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(TwigError::GitHub(
            opt_s(&v, "/error_description").unwrap_or_else(|| err.to_string()),
        ));
    }
    Ok(serde_json::from_value(v)?)
}

pub async fn poll_device_token(
    client: &Client,
    ep: &GitHubEndpoint,
    device_code: &str,
) -> Result<PollOutcome, TwigError> {
    let resp = client
        .post(format!("{}/login/oauth/access_token", ep.web_base))
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_OAUTH_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await?;
    let resp = check_response(resp).await?;
    let v: Value = resp.json().await?;
    Ok(map_poll_response(&v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_graphql_pr_list() {
        let resp = json!({"data": {"repository": {"pullRequests": {
            "pageInfo": {"hasNextPage": true, "endCursor": "Y3Vy"},
            "nodes": [{
                "number": 42, "title": "Add thing", "state": "OPEN", "isDraft": true,
                "url": "https://github.com/o/r/pull/42",
                "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-02T00:00:00Z",
                "author": {"login": "alice", "avatarUrl": "https://a/1"},
                "headRefName": "feat", "headRefOid": "abc123", "baseRefName": "main",
                "isCrossRepository": true, "headRepository": {"nameWithOwner": "alice/r"},
                "reviewDecision": "CHANGES_REQUESTED",
                "comments": {"totalCount": 3},
                "labels": {"nodes": [{"name": "bug", "color": "d73a4a"}]},
                "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "ERROR"}}}]}
            }, {
                "number": 7, "title": "Old", "state": "MERGED", "isDraft": false,
                "url": "u", "createdAt": "c", "updatedAt": "u",
                "author": null, "headRefName": "x", "headRefOid": "def",
                "baseRefName": "main", "isCrossRepository": false, "headRepository": null,
                "reviewDecision": null, "comments": {"totalCount": 0},
                "labels": {"nodes": []},
                "commits": {"nodes": [{"commit": {"statusCheckRollup": null}}]}
            }]
        }}}});
        let page = map_graphql_prs(&resp).unwrap();
        assert_eq!(page.next_cursor.as_deref(), Some("Y3Vy"));
        let a = &page.items[0];
        assert_eq!(a.number, 42);
        assert_eq!(a.state, "open");
        assert!(a.draft);
        assert_eq!(a.author, "alice");
        assert_eq!(a.head_repo.as_deref(), Some("alice/r"));
        assert_eq!(a.review_state.as_deref(), Some("changes_requested"));
        assert_eq!(a.ci_state.as_deref(), Some("failure"));
        assert_eq!(a.comments, 3);
        assert_eq!(a.labels[0].color.as_deref(), Some("d73a4a"));
        let b = &page.items[1];
        assert_eq!(b.state, "merged");
        assert_eq!(b.author, "ghost");
        assert_eq!(b.head_repo, None);
        assert_eq!(b.review_state, None);
        assert_eq!(b.ci_state.as_deref(), Some("none"));
    }

    #[test]
    fn graphql_errors_and_last_page() {
        let err = map_graphql_prs(&json!({"errors": [{"message": "Could not resolve"}]}));
        assert!(err.unwrap_err().to_string().contains("Could not resolve"));
        let page = map_graphql_prs(&json!({"data": {"repository": {"pullRequests": {
            "pageInfo": {"hasNextPage": false, "endCursor": "x"}, "nodes": []}}}}))
        .unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.next_cursor, None);
        assert!(map_graphql_prs(&json!({"data": {"repository": null}})).is_err());
    }

    #[test]
    fn filter_states() {
        assert_eq!(states_for_filter("open"), json!(["OPEN"]));
        assert_eq!(states_for_filter("closed"), json!(["CLOSED", "MERGED"]));
        assert_eq!(states_for_filter("all"), Value::Null);
    }

    #[test]
    fn maps_rest_pr_detail() {
        let v = json!({
            "number": 5, "title": "T", "state": "closed", "merged": true, "draft": false,
            "user": {"login": "bob", "avatar_url": "av"},
            "head": {"ref": "fix", "sha": "s1", "repo": {"full_name": "bob/r"}},
            "base": {"ref": "main", "repo": {"full_name": "o/r"}},
            "html_url": "h", "created_at": "c", "updated_at": "u",
            "comments": 2, "review_comments": 3, "body": null,
            "additions": 10, "deletions": 4, "changed_files": 2, "commits": 1,
            "mergeable_state": "clean", "labels": []
        });
        let d = map_rest_pr(&v);
        assert_eq!(d.summary.state, "merged");
        assert_eq!(d.summary.head_repo.as_deref(), Some("bob/r"));
        assert_eq!(d.summary.comments, 5);
        assert_eq!(d.body, "");
        assert_eq!(d.additions, Some(10));
        assert_eq!(d.mergeable_state.as_deref(), Some("clean"));

        let same = json!({"number": 1, "state": "open",
            "head": {"ref": "a", "sha": "s", "repo": {"full_name": "O/R"}},
            "base": {"ref": "main", "repo": {"full_name": "o/r"}}});
        assert_eq!(map_rest_pr(&same).summary.head_repo, None);
        assert_eq!(map_rest_pr(&same).summary.state, "open");
    }

    #[test]
    fn maps_pr_files() {
        let f = map_pr_file(RestPrFile {
            filename: "b.rs".into(),
            previous_filename: Some("a.rs".into()),
            status: "renamed".into(),
            additions: 1,
            deletions: 0,
            patch: Some("@@ -1 +1,2 @@\n x\n+y".into()),
        });
        assert_eq!(f.status, "renamed");
        assert_eq!(f.hunks.len(), 1);
        assert!(!f.patch_missing);
        let bin = map_pr_file(RestPrFile {
            filename: "img.png".into(),
            previous_filename: None,
            status: "removed".into(),
            additions: 0,
            deletions: 0,
            patch: None,
        });
        assert_eq!(bin.status, "deleted");
        assert!(bin.patch_missing);
    }

    #[test]
    fn maps_ci() {
        let combined = json!({"state": "pending", "statuses": [
            {"context": "ci/legacy", "state": "success", "description": "ok", "target_url": "t"}
        ]});
        let runs = json!({"check_runs": [
            {"name": "build", "status": "completed", "conclusion": "failure", "html_url": "h", "output": {"title": "2 errors"}},
            {"name": "lint", "status": "in_progress", "conclusion": null},
            {"name": "docs", "status": "completed", "conclusion": "skipped"}
        ]});
        let ci = map_ci("sha", Some(&combined), Some(&runs));
        assert_eq!(ci.state, "failure");
        assert_eq!(ci.checks.len(), 4);
        assert_eq!(ci.checks[1].description.as_deref(), Some("2 errors"));
        assert_eq!(ci.checks[2].state, "pending");
        assert_eq!(ci.checks[3].state, "neutral");

        let none = map_ci("sha", Some(&json!({"statuses": []})), None);
        assert_eq!(none.state, "none");
        let only_runs = map_ci("sha", None, Some(&json!({"check_runs": [
            {"name": "x", "status": "completed", "conclusion": "success"}]})));
        assert_eq!(only_runs.state, "success");
    }

    #[test]
    fn maps_device_poll() {
        assert_eq!(
            map_poll_response(&json!({"access_token": "gho_x", "token_type": "bearer"})),
            PollOutcome::Token("gho_x".into())
        );
        assert_eq!(map_poll_response(&json!({"error": "authorization_pending"})), PollOutcome::Pending);
        assert_eq!(
            map_poll_response(&json!({"error": "slow_down", "interval": 10})),
            PollOutcome::SlowDown(Some(10))
        );
        assert!(matches!(map_poll_response(&json!({"error": "expired_token"})), PollOutcome::Failed(_)));
        assert_eq!(
            map_poll_response(&json!({"error": "weird", "error_description": "Weird thing"})),
            PollOutcome::Failed("Weird thing".into())
        );
    }

    /// Read-only calls against github.com/hoxton314/git-twig. Opt-in:
    /// `TWIG_LIVE_GITHUB_TOKEN=$(gh auth token) cargo test live_github -- --ignored`
    #[tokio::test]
    #[ignore = "needs network and TWIG_LIVE_GITHUB_TOKEN"]
    async fn live_github_read_only() {
        let Ok(token) = std::env::var("TWIG_LIVE_GITHUB_TOKEN") else {
            return;
        };
        let client = crate::hosting::http::build_client().unwrap();
        let ep = GitHubEndpoint::new("github.com", "");
        let (owner, repo) = ("hoxton314", "git-twig");

        let page = list_pull_requests(&client, &ep, &token, owner, repo, "merged", None)
            .await
            .unwrap();
        let pr = page.items.iter().find(|p| p.number == 16).expect("PR #16 in merged list");
        assert_eq!(pr.state, "merged");
        assert_eq!(pr.base_ref, "main");

        let detail = get_pull_request(&client, &ep, &token, owner, repo, 16).await.unwrap();
        assert_eq!(detail.summary.number, 16);
        assert!(detail.body.contains("Closes #3"));
        assert!(detail.changed_files.unwrap_or(0) >= 1);

        let files = list_pull_request_files(&client, &ep, &token, owner, repo, 16).await.unwrap();
        let f = files.iter().find(|f| f.path == "src-tauri/src/git/history.rs").expect("history.rs");
        assert!(!f.patch_missing && !f.hunks.is_empty());
        assert!(f.additions > 0);

        // CI on main's tip: the CI workflow exists, so there is a check.
        let sha = std::process::Command::new("git")
            .args(["ls-remote", "https://github.com/hoxton314/git-twig", "refs/heads/main"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.split_whitespace().next().map(String::from))
            .expect("main sha");
        let ci = ci_status(&client, &ep, &token, owner, repo, &sha).await.unwrap();
        assert_ne!(ci.state, "none", "{ci:?}");
        assert!(ci.checks.iter().any(|c| c.name.contains("check")), "{ci:?}");
    }
}
