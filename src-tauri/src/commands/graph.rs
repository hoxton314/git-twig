use tauri::State;

use crate::error::TwigError;
use crate::git::graph::{self, CommitSearchResult, GraphOptions, LocatedCommit};
use crate::git::reader::CommitGraph;
use crate::state::AppState;

/// Fetch one page of the commit graph: rows `[skip, skip + max_commits)`.
/// `max_commits` defaults to 5000, `skip` to 0.
#[tauri::command]
pub async fn get_commit_graph(
    state: State<'_, AppState>,
    path: String,
    max_commits: Option<usize>,
    skip: Option<usize>,
    options: Option<GraphOptions>,
) -> Result<CommitGraph, TwigError> {
    let limit = max_commits.unwrap_or(5000);
    let skip = skip.unwrap_or(0);
    let opts = options.unwrap_or_default();
    state
        .read_repo(&path, move |repo| {
            graph::read_commit_graph_page(repo, skip, limit, &opts)
        })
        .await
}

/// Search the full history (message, author name/email, SHA prefix).
#[tauri::command]
pub async fn search_commits(
    state: State<'_, AppState>,
    path: String,
    query: String,
    options: Option<GraphOptions>,
    max_results: Option<usize>,
) -> Result<CommitSearchResult, TwigError> {
    let opts = options.unwrap_or_default();
    let max = max_results.unwrap_or(5000).clamp(1, 100_000);
    state
        .read_repo(&path, move |repo| graph::search_commits(repo, &query, &opts, max))
        .await
}

/// Search the history for code changes: commits that add or remove `query`
/// (`git log -S`), or with `regex`, whose changed lines match it (`-G`).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_changes(
    state: State<'_, AppState>,
    path: String,
    query: String,
    regex: bool,
    ignore_case: bool,
    paths: Vec<String>,
    options: Option<GraphOptions>,
    max_results: Option<usize>,
) -> Result<CommitSearchResult, TwigError> {
    let opts = options.unwrap_or_default();
    let max = max_results.unwrap_or(5000).clamp(1, 100_000);
    let repo_path = state.repo_path(&path)?;
    let o = opts.clone();
    let tips = state.read_repo(&path, move |repo| Ok(graph::graph_tip_args(repo, &o))).await?;
    let (oids, truncated) =
        crate::git::pickaxe::pickaxe(&repo_path, &query, regex, ignore_case, &tips, &paths, max).await?;
    state
        .read_repo(&path, move |repo| graph::matches_in_graph(repo, &oids, &opts, max, truncated))
        .await
}

/// Resolve a revision (branch, tag, HEAD, SHA) and find its graph row.
#[tauri::command]
pub async fn locate_commit(
    state: State<'_, AppState>,
    path: String,
    rev: String,
    options: Option<GraphOptions>,
) -> Result<LocatedCommit, TwigError> {
    let opts = options.unwrap_or_default();
    state
        .read_repo(&path, move |repo| graph::locate_commit(repo, &rev, &opts))
        .await
}
