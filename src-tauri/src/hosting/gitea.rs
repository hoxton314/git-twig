//! Gitea / Forgejo REST API v1 client. The pull request and status
//! payloads are GitHub-compatible, so their mapping is shared.

use reqwest::{Client, RequestBuilder};
use serde_json::Value;

use super::github_api::map_rest_pr;
use super::http::{check, encode_segment};
use super::patch::{count_changes, parse_unified_patch, split_git_diff};
use super::types::{rollup, CiCheck, CiStatus, PrDetail, PrFile, PrPage};
use crate::error::TwigError;
use crate::github::{GitHubOwner, GitHubPullRequest, GitHubRepo, GitHubUser, RepoListPage};

const NAME: &str = "Gitea";

pub struct Gitea<'a> {
    pub client: &'a Client,
    /// Instance base URL, e.g. `https://codeberg.org`.
    pub base: &'a str,
    pub token: &'a str,
}

fn s(v: &Value, ptr: &str) -> String {
    v.pointer(ptr).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn opt_s(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr).and_then(Value::as_str).map(String::from).filter(|x| !x.is_empty())
}

impl Gitea<'_> {
    fn api(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.base)
    }

    fn auth(&self, b: RequestBuilder) -> RequestBuilder {
        b.header("Authorization", format!("token {}", self.token))
            .header("Accept", "application/json")
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response, TwigError> {
        let resp = self.auth(self.client.get(self.api(path))).send().await?;
        check(NAME, resp).await
    }

    fn repo_path(owner: &str, repo: &str) -> String {
        format!("/repos/{}/{}", encode_segment(owner), encode_segment(repo))
    }

    pub async fn validate(&self) -> Result<GitHubUser, TwigError> {
        let v: Value = self.get("/user").await?.json().await?;
        Ok(GitHubUser {
            login: s(&v, "/login"),
            name: opt_s(&v, "/full_name"),
            avatar_url: s(&v, "/avatar_url"),
        })
    }

    pub async fn list_repos(&self, page: u32, per_page: u32) -> Result<RepoListPage, TwigError> {
        let resp = self.get(&format!("/user/repos?page={page}&limit={per_page}")).await?;
        let has_link_next = crate::github::has_next_link(resp.headers());
        let v: Vec<Value> = resp.json().await?;
        let full_page = v.len() as u32 >= per_page;
        Ok(RepoListPage {
            repos: v.iter().map(map_repo).collect(),
            has_next_page: has_link_next || full_page,
        })
    }

    pub async fn list_branches(&self, owner: &str, repo: &str) -> Result<Vec<String>, TwigError> {
        let mut names = Vec::new();
        for page in 1..=20 {
            let v: Vec<Value> = self
                .get(&format!("{}/branches?page={page}&limit=50", Self::repo_path(owner, repo)))
                .await?
                .json()
                .await?;
            let n = v.len();
            names.extend(v.iter().map(|b| s(b, "/name")));
            if n < 50 {
                break;
            }
        }
        Ok(names)
    }

    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<GitHubPullRequest, TwigError> {
        let payload = serde_json::json!({ "title": title, "body": body, "head": head, "base": base });
        let resp = self
            .auth(self.client.post(self.api(&format!("{}/pulls", Self::repo_path(owner, repo)))))
            .json(&payload)
            .send()
            .await?;
        let v: Value = check(NAME, resp).await?.json().await?;
        Ok(GitHubPullRequest {
            number: v.get("number").and_then(Value::as_u64).unwrap_or(0),
            html_url: s(&v, "/html_url"),
            title: s(&v, "/title"),
            state: s(&v, "/state"),
        })
    }

    pub async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        filter: &str,
        cursor: Option<&str>,
    ) -> Result<PrPage, TwigError> {
        // Gitea has no "merged" state filter; merged PRs are closed ones.
        let state = match filter {
            "closed" | "merged" => "closed",
            "all" => "all",
            _ => "open",
        };
        let page = cursor.and_then(|c| c.parse::<u32>().ok()).unwrap_or(1);
        let v: Vec<Value> = self
            .get(&format!(
                "{}/pulls?state={state}&sort=recentupdate&page={page}&limit=30",
                Self::repo_path(owner, repo)
            ))
            .await?
            .json()
            .await?;
        let full = v.len() >= 30;
        let mut items: Vec<_> = v.iter().map(|p| map_rest_pr(p).summary).collect();
        if filter == "merged" {
            items.retain(|p| p.state == "merged");
        }
        Ok(PrPage {
            items,
            next_cursor: full.then(|| (page + 1).to_string()),
        })
    }

    pub async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PrDetail, TwigError> {
        let v: Value = self
            .get(&format!("{}/pulls/{number}", Self::repo_path(owner, repo)))
            .await?
            .json()
            .await?;
        Ok(map_rest_pr(&v))
    }

    pub async fn pull_request_files(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<PrFile>, TwigError> {
        let text = self
            .get(&format!("{}/pulls/{number}.diff", Self::repo_path(owner, repo)))
            .await?
            .text()
            .await?;
        Ok(map_diff_text(&text))
    }

    pub async fn ci_status(&self, owner: &str, repo: &str, sha: &str) -> Result<CiStatus, TwigError> {
        let v: Value = self
            .get(&format!(
                "{}/commits/{}/status",
                Self::repo_path(owner, repo),
                encode_segment(sha)
            ))
            .await?
            .json()
            .await?;
        Ok(map_combined_status(sha, &v))
    }
}

pub fn map_repo(r: &Value) -> GitHubRepo {
    GitHubRepo {
        id: r.get("id").and_then(Value::as_u64).unwrap_or(0),
        full_name: s(r, "/full_name"),
        name: s(r, "/name"),
        owner: GitHubOwner {
            login: s(r, "/owner/login"),
            avatar_url: s(r, "/owner/avatar_url"),
        },
        description: opt_s(r, "/description"),
        private: r.get("private").and_then(Value::as_bool).unwrap_or(false),
        html_url: s(r, "/html_url"),
        clone_url: s(r, "/clone_url"),
        ssh_url: s(r, "/ssh_url"),
        default_branch: s(r, "/default_branch"),
        stargazers_count: r.get("stars_count").and_then(Value::as_u64).unwrap_or(0) as u32,
        updated_at: s(r, "/updated_at"),
        fork: r.get("fork").and_then(Value::as_bool).unwrap_or(false),
    }
}

pub fn map_diff_text(text: &str) -> Vec<PrFile> {
    split_git_diff(text)
        .into_iter()
        .map(|sec| {
            let hunks = parse_unified_patch(&sec.body);
            let (additions, deletions) = count_changes(&hunks);
            PrFile {
                path: sec.new_path.clone().or_else(|| sec.old_path.clone()).unwrap_or_default(),
                old_path: if sec.status == "renamed" { sec.old_path } else { None },
                status: sec.status,
                additions,
                deletions,
                patch_missing: sec.is_binary || (hunks.is_empty() && !sec.body.is_empty()),
                hunks,
            }
        })
        .collect()
}

/// Gitea statuses use `status` (newer: also `state`) with values
/// pending / success / error / failure / warning.
pub fn map_combined_status(sha: &str, v: &Value) -> CiStatus {
    let checks: Vec<CiCheck> = v
        .get("statuses")
        .and_then(Value::as_array)
        .map(|sts| {
            sts.iter()
                .map(|st| {
                    let raw = opt_s(st, "/status").or_else(|| opt_s(st, "/state")).unwrap_or_default();
                    let state = match raw.as_str() {
                        "success" => "success",
                        "failure" | "error" => "failure",
                        "warning" => "neutral",
                        _ => "pending",
                    };
                    CiCheck {
                        name: s(st, "/context"),
                        state: state.into(),
                        description: opt_s(st, "/description"),
                        url: opt_s(st, "/target_url"),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    CiStatus {
        sha: sha.to_string(),
        state: rollup(&checks),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_repo_and_status() {
        let r = map_repo(&json!({"id": 3, "full_name": "o/r", "name": "r",
            "owner": {"login": "o", "avatar_url": "a"}, "description": "d", "private": true,
            "html_url": "h", "clone_url": "c", "ssh_url": "s", "default_branch": "main",
            "stars_count": 2, "updated_at": "u", "fork": false}));
        assert_eq!(r.full_name, "o/r");
        assert_eq!(r.stargazers_count, 2);
        assert!(r.private);

        let ci = map_combined_status("x", &json!({"state": "pending", "statuses": [
            {"context": "ci", "status": "warning"},
            {"context": "woodpecker", "state": "error", "target_url": "t"}
        ]}));
        assert_eq!(ci.state, "failure");
        assert_eq!(ci.checks[0].state, "neutral");
        assert_eq!(map_combined_status("x", &json!({})).state, "none");
    }

    #[test]
    fn maps_pr_and_diff() {
        let d = map_rest_pr(&json!({"number": 3, "title": "t", "state": "closed", "merged": true,
            "user": {"login": "u"}, "head": {"ref": "f", "sha": "s", "repo": {"full_name": "o/r"}},
            "base": {"ref": "main", "repo": {"full_name": "o/r"}}, "body": "b"}));
        assert_eq!(d.summary.state, "merged");
        let files = map_diff_text("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1,2 @@\n x\n+y\n");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "x");
        assert_eq!((files[0].additions, files[0].deletions), (1, 0));
    }
}
