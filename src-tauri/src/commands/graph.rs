use tauri::State;

use crate::error::TwigError;
use crate::git::reader::{self, CommitGraph};
use crate::state::AppState;

/// Fetch the commit graph for a repository.
/// `max_commits` limits how many commits to walk (default 5000).
#[tauri::command]
pub async fn get_commit_graph(
    state: State<'_, AppState>,
    path: String,
    max_commits: Option<usize>,
) -> Result<CommitGraph, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    let limit = max_commits.unwrap_or(5000);
    reader::read_commit_graph(&open.repository, limit)
}
