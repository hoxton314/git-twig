//! Provider-agnostic pull/merge request and CI types sent to the UI.
//! Mirrored in `src/lib/types/hosting.ts`.

use serde::Serialize;

use crate::git::reader::DiffHunk;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PrLabel {
    pub name: String,
    /// Hex color without `#`, if the provider has one.
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PrSummary {
    pub number: u64,
    pub title: String,
    /// `open`, `closed` or `merged`.
    pub state: String,
    pub draft: bool,
    pub author: String,
    pub author_avatar: Option<String>,
    pub head_ref: String,
    pub head_sha: String,
    /// `owner/repo` of the head when it lives in a fork, else `None`.
    pub head_repo: Option<String>,
    pub base_ref: String,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub comments: u32,
    /// `approved`, `changes_requested`, `review_required`, or `None`.
    pub review_state: Option<String>,
    /// CI rollup if the list API provides it cheaply (see `CiStatus::state`).
    pub ci_state: Option<String>,
    pub labels: Vec<PrLabel>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrPage {
    pub items: Vec<PrSummary>,
    /// Opaque cursor for the next page (`None` = last page).
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrDetail {
    pub summary: PrSummary,
    /// Raw markdown description (rendered safely by the UI).
    pub body: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub changed_files: Option<u32>,
    pub commits: Option<u32>,
    /// Provider's mergeability hint, e.g. `clean`, `dirty`, `blocked`.
    pub mergeable_state: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrFile {
    pub path: String,
    pub old_path: Option<String>,
    /// `added`, `deleted`, `modified`, `renamed`.
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub hunks: Vec<DiffHunk>,
    /// No textual patch available (binary or too large for the API).
    pub patch_missing: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CiCheck {
    pub name: String,
    /// `success`, `failure`, `pending`, `neutral` (skipped/cancelled-neutral).
    pub state: String,
    pub description: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CiStatus {
    pub sha: String,
    /// Rollup: `success`, `failure`, `pending`, `neutral`, or `none` (no CI).
    pub state: String,
    pub checks: Vec<CiCheck>,
}

/// Combine individual check states: any failure → failure, else any
/// pending → pending, else any success → success, else neutral/none.
pub fn rollup(checks: &[CiCheck]) -> String {
    if checks.is_empty() {
        return "none".into();
    }
    let has = |s: &str| checks.iter().any(|c| c.state == s);
    if has("failure") {
        "failure"
    } else if has("pending") {
        "pending"
    } else if has("success") {
        "success"
    } else {
        "neutral"
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(state: &str) -> CiCheck {
        CiCheck {
            name: "x".into(),
            state: state.into(),
            description: None,
            url: None,
        }
    }

    #[test]
    fn rollup_precedence() {
        assert_eq!(rollup(&[]), "none");
        assert_eq!(rollup(&[c("success"), c("pending")]), "pending");
        assert_eq!(rollup(&[c("pending"), c("failure"), c("success")]), "failure");
        assert_eq!(rollup(&[c("success"), c("neutral")]), "success");
        assert_eq!(rollup(&[c("neutral")]), "neutral");
    }
}
