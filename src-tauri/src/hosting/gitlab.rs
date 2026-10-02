//! GitLab REST API v4 client (gitlab.com and self-hosted).
//! Projects are addressed by their URL-encoded full path.

use reqwest::{Client, RequestBuilder};
use serde_json::Value;

use super::http::{check, encode_query, encode_segment};
use super::patch::{count_changes, parse_unified_patch};
use super::types::{rollup, CiCheck, CiStatus, PrDetail, PrFile, PrLabel, PrPage, PrSummary};
use crate::error::TwigError;
use crate::github::{GitHubOwner, GitHubPullRequest, GitHubRepo, GitHubUser, RepoListPage};

const NAME: &str = "GitLab";

pub struct GitLab<'a> {
    pub client: &'a Client,
    /// Instance base URL, e.g. `https://gitlab.com`.
    pub base: &'a str,
    pub token: &'a str,
}

fn s(v: &Value, ptr: &str) -> String {
    v.pointer(ptr).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn opt_s(v: &Value, ptr: &str) -> Option<String> {
    v.pointer(ptr).and_then(Value::as_str).map(String::from).filter(|x| !x.is_empty())
}

/// GitLab paginates with `x-next-page` (empty on the last page).
fn next_page(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("x-next-page")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
}

impl GitLab<'_> {
    fn api(&self, path: &str) -> String {
        format!("{}/api/v4{path}", self.base)
    }

    fn auth(&self, b: RequestBuilder) -> RequestBuilder {
        b.header("PRIVATE-TOKEN", self.token)
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response, TwigError> {
        let resp = self.auth(self.client.get(self.api(path))).send().await?;
        check(NAME, resp).await
    }

    pub async fn validate(&self) -> Result<GitHubUser, TwigError> {
        let v: Value = self.get("/user").await?.json().await?;
        Ok(GitHubUser {
            login: s(&v, "/username"),
            name: opt_s(&v, "/name"),
            avatar_url: s(&v, "/avatar_url"),
        })
    }

    pub async fn list_repos(&self, page: u32, per_page: u32) -> Result<RepoListPage, TwigError> {
        let resp = self
            .get(&format!(
                "/projects?membership=true&order_by=last_activity_at&sort=desc&per_page={per_page}&page={page}"
            ))
            .await?;
        let has_next = next_page(resp.headers()).is_some();
        let v: Vec<Value> = resp.json().await?;
        Ok(RepoListPage {
            repos: v.iter().map(map_project).collect(),
            has_next_page: has_next,
        })
    }

    pub async fn list_branches(&self, project: &str) -> Result<Vec<String>, TwigError> {
        let mut names = Vec::new();
        let mut page = "1".to_string();
        for _ in 0..20 {
            let resp = self
                .get(&format!(
                    "/projects/{}/repository/branches?per_page=100&page={page}",
                    encode_segment(project)
                ))
                .await?;
            let next = next_page(resp.headers());
            let v: Vec<Value> = resp.json().await?;
            names.extend(v.iter().map(|b| s(b, "/name")));
            match next {
                Some(n) => page = n,
                None => break,
            }
        }
        Ok(names)
    }

    pub async fn create_merge_request(
        &self,
        project: &str,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<GitHubPullRequest, TwigError> {
        let payload = serde_json::json!({
            "source_branch": head,
            "target_branch": base,
            "title": title,
            "description": body,
        });
        let resp = self
            .auth(self.client.post(self.api(&format!(
                "/projects/{}/merge_requests",
                encode_segment(project)
            ))))
            .json(&payload)
            .send()
            .await?;
        let v: Value = check(NAME, resp).await?.json().await?;
        Ok(GitHubPullRequest {
            number: v.get("iid").and_then(Value::as_u64).unwrap_or(0),
            html_url: s(&v, "/web_url"),
            title: s(&v, "/title"),
            state: s(&v, "/state"),
        })
    }

    pub async fn list_merge_requests(
        &self,
        project: &str,
        filter: &str,
        cursor: Option<&str>,
    ) -> Result<PrPage, TwigError> {
        let state = match filter {
            "closed" => "closed",
            "merged" => "merged",
            "all" => "all",
            _ => "opened",
        };
        let page = cursor.and_then(|c| c.parse::<u32>().ok()).unwrap_or(1);
        let resp = self
            .get(&format!(
                "/projects/{}/merge_requests?state={}&order_by=updated_at&sort=desc&per_page=30&page={page}",
                encode_segment(project),
                encode_query(state)
            ))
            .await?;
        let next = next_page(resp.headers());
        let v: Vec<Value> = resp.json().await?;
        Ok(PrPage {
            items: v.iter().map(map_mr).collect(),
            next_cursor: next,
        })
    }

    pub async fn get_merge_request(&self, project: &str, iid: u64) -> Result<PrDetail, TwigError> {
        let v: Value = self
            .get(&format!("/projects/{}/merge_requests/{iid}", encode_segment(project)))
            .await?
            .json()
            .await?;
        Ok(map_mr_detail(&v))
    }

    pub async fn merge_request_files(&self, project: &str, iid: u64) -> Result<Vec<PrFile>, TwigError> {
        let mut out = Vec::new();
        let mut page = "1".to_string();
        for _ in 0..30 {
            let resp = self
                .get(&format!(
                    "/projects/{}/merge_requests/{iid}/diffs?per_page=100&page={page}",
                    encode_segment(project)
                ))
                .await?;
            let next = next_page(resp.headers());
            let v: Vec<Value> = resp.json().await?;
            out.extend(v.iter().map(map_mr_diff));
            match next {
                Some(n) => page = n,
                None => break,
            }
        }
        Ok(out)
    }

    pub async fn ci_status(&self, project: &str, sha: &str) -> Result<CiStatus, TwigError> {
        let v: Vec<Value> = self
            .get(&format!(
                "/projects/{}/repository/commits/{}/statuses?per_page=100",
                encode_segment(project),
                encode_segment(sha)
            ))
            .await?
            .json()
            .await?;
        Ok(map_statuses(sha, &v))
    }
}

pub fn map_project(p: &Value) -> GitHubRepo {
    GitHubRepo {
        id: p.get("id").and_then(Value::as_u64).unwrap_or(0),
        full_name: s(p, "/path_with_namespace"),
        name: s(p, "/path"),
        owner: GitHubOwner {
            login: s(p, "/namespace/full_path"),
            avatar_url: opt_s(p, "/namespace/avatar_url")
                .or_else(|| opt_s(p, "/avatar_url"))
                .unwrap_or_default(),
        },
        description: opt_s(p, "/description"),
        private: s(p, "/visibility") != "public",
        html_url: s(p, "/web_url"),
        clone_url: s(p, "/http_url_to_repo"),
        ssh_url: s(p, "/ssh_url_to_repo"),
        default_branch: s(p, "/default_branch"),
        stargazers_count: p.get("star_count").and_then(Value::as_u64).unwrap_or(0) as u32,
        updated_at: s(p, "/last_activity_at"),
        fork: p.get("forked_from_project").is_some_and(|f| !f.is_null()),
    }
}

fn map_mr_state(state: &str) -> String {
    match state {
        "opened" | "locked" => "open",
        "merged" => "merged",
        _ => "closed",
    }
    .into()
}

fn map_pipeline_status(status: &str) -> &'static str {
    match status {
        "success" => "success",
        "failed" => "failure",
        "canceled" | "skipped" | "manual" => "neutral",
        _ => "pending", // created, waiting_for_resource, preparing, pending, running, scheduled
    }
}

pub fn map_mr(v: &Value) -> PrSummary {
    let cross = match (v.get("source_project_id"), v.get("target_project_id")) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };
    // Fork path isn't in the list payload; the web URL of the source is
    // only in `references.full` of the source. Show a generic marker.
    PrSummary {
        number: v.get("iid").and_then(Value::as_u64).unwrap_or(0),
        title: s(v, "/title"),
        state: map_mr_state(&s(v, "/state")),
        draft: v.get("draft").and_then(Value::as_bool).unwrap_or(false)
            || v.get("work_in_progress").and_then(Value::as_bool).unwrap_or(false),
        author: opt_s(v, "/author/username").unwrap_or_else(|| "ghost".into()),
        author_avatar: opt_s(v, "/author/avatar_url"),
        head_ref: s(v, "/source_branch"),
        head_sha: s(v, "/sha"),
        head_repo: if cross { Some("fork".into()) } else { None },
        base_ref: s(v, "/target_branch"),
        html_url: s(v, "/web_url"),
        created_at: s(v, "/created_at"),
        updated_at: s(v, "/updated_at"),
        comments: v.get("user_notes_count").and_then(Value::as_u64).unwrap_or(0) as u32,
        review_state: None,
        ci_state: v
            .pointer("/head_pipeline/status")
            .and_then(Value::as_str)
            .map(|st| map_pipeline_status(st).to_string()),
        labels: v
            .get("labels")
            .and_then(Value::as_array)
            .map(|ls| {
                ls.iter()
                    .filter_map(|l| {
                        // Plain strings, or objects with `with_labels_details`.
                        l.as_str()
                            .map(|n| PrLabel { name: n.into(), color: None })
                            .or_else(|| {
                                Some(PrLabel {
                                    name: opt_s(l, "/name")?,
                                    color: opt_s(l, "/color").map(|c| c.trim_start_matches('#').to_string()),
                                })
                            })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn map_mr_detail(v: &Value) -> PrDetail {
    let changed = v
        .get("changes_count")
        .and_then(|c| c.as_str().map(|s| s.trim_end_matches('+').parse().ok()).unwrap_or(c.as_u64().map(|n| n as u32)));
    let mergeable = opt_s(v, "/detailed_merge_status").or_else(|| opt_s(v, "/merge_status"));
    PrDetail {
        summary: map_mr(v),
        body: s(v, "/description"),
        additions: None,
        deletions: None,
        changed_files: changed,
        commits: None,
        mergeable_state: mergeable,
    }
}

pub fn map_mr_diff(d: &Value) -> PrFile {
    let flag = |k: &str| d.get(k).and_then(Value::as_bool).unwrap_or(false);
    let status = if flag("new_file") {
        "added"
    } else if flag("deleted_file") {
        "deleted"
    } else if flag("renamed_file") {
        "renamed"
    } else {
        "modified"
    };
    let hunks = parse_unified_patch(&s(d, "/diff"));
    let (additions, deletions) = count_changes(&hunks);
    let old_path = opt_s(d, "/old_path");
    let new_path = s(d, "/new_path");
    PrFile {
        patch_missing: hunks.is_empty() && (flag("too_large") || flag("collapsed") || s(d, "/diff").is_empty()),
        old_path: if status == "renamed" { old_path } else { None },
        path: new_path,
        status: status.into(),
        additions,
        deletions,
        hunks,
    }
}

pub fn map_statuses(sha: &str, v: &[Value]) -> CiStatus {
    let checks: Vec<CiCheck> = v
        .iter()
        .map(|st| {
            let allow_failure = st.get("allow_failure").and_then(Value::as_bool).unwrap_or(false);
            let mut state = map_pipeline_status(&s(st, "/status"));
            if state == "failure" && allow_failure {
                state = "neutral";
            }
            CiCheck {
                name: s(st, "/name"),
                state: state.into(),
                description: opt_s(st, "/description"),
                url: opt_s(st, "/target_url"),
            }
        })
        .collect();
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
    fn maps_project() {
        let p = map_project(&json!({
            "id": 9, "path": "proj", "path_with_namespace": "grp/sub/proj",
            "namespace": {"full_path": "grp/sub", "avatar_url": null},
            "description": "", "visibility": "internal", "web_url": "https://gl/grp/sub/proj",
            "http_url_to_repo": "https://gl/grp/sub/proj.git", "ssh_url_to_repo": "git@gl:grp/sub/proj.git",
            "default_branch": "main", "star_count": 4, "last_activity_at": "2026-01-01",
            "forked_from_project": {"id": 1}
        }));
        assert_eq!(p.full_name, "grp/sub/proj");
        assert_eq!(p.owner.login, "grp/sub");
        assert!(p.private);
        assert!(p.fork);
        assert_eq!(p.description, None);
        assert_eq!(p.stargazers_count, 4);
    }

    #[test]
    fn maps_merge_request() {
        let v = json!({
            "iid": 12, "title": "Draft: x", "state": "opened", "draft": true,
            "author": {"username": "carol", "avatar_url": "a"},
            "source_branch": "feat", "target_branch": "main", "sha": "abc",
            "source_project_id": 2, "target_project_id": 1,
            "web_url": "w", "created_at": "c", "updated_at": "u", "user_notes_count": 5,
            "labels": ["bug", "ui"], "description": "**hi**", "changes_count": "1000+",
            "head_pipeline": {"status": "running"}, "detailed_merge_status": "mergeable"
        });
        let d = map_mr_detail(&v);
        assert_eq!(d.summary.number, 12);
        assert_eq!(d.summary.state, "open");
        assert!(d.summary.draft);
        assert_eq!(d.summary.head_repo.as_deref(), Some("fork"));
        assert_eq!(d.summary.labels.len(), 2);
        assert_eq!(d.summary.ci_state.as_deref(), Some("pending"));
        assert_eq!(d.changed_files, Some(1000));
        assert_eq!(d.body, "**hi**");
        assert_eq!(d.mergeable_state.as_deref(), Some("mergeable"));
        assert_eq!(map_mr(&json!({"state": "merged"})).state, "merged");
        assert_eq!(map_mr(&json!({"state": "closed"})).state, "closed");
    }

    #[test]
    fn maps_diffs_and_statuses() {
        let f = map_mr_diff(&json!({
            "old_path": "a", "new_path": "b", "renamed_file": true,
            "diff": "@@ -1 +1 @@\n-x\n+y\n"
        }));
        assert_eq!(f.status, "renamed");
        assert_eq!(f.old_path.as_deref(), Some("a"));
        assert_eq!((f.additions, f.deletions), (1, 1));
        let big = map_mr_diff(&json!({"new_path": "x", "diff": "", "too_large": true}));
        assert!(big.patch_missing);

        let ci = map_statuses("s", &[
            json!({"name": "test", "status": "failed", "allow_failure": true}),
            json!({"name": "build", "status": "success", "target_url": "t"}),
        ]);
        assert_eq!(ci.state, "success");
        assert_eq!(ci.checks[0].state, "neutral");
        assert_eq!(map_statuses("s", &[]).state, "none");
    }
}
