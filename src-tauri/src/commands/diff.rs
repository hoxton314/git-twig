use tauri::State;

use crate::error::TwigError;
use crate::git::reader::{self, DiffFile};
use crate::state::AppState;

/// Get the diff for a specific commit.
#[tauri::command]
pub async fn get_commit_diff(
    state: State<'_, AppState>,
    path: String,
    oid: String,
) -> Result<Vec<DiffFile>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_commit_diff(&open.repository, &oid)
}

/// Get the working directory diff (staged + unstaged changes).
#[tauri::command]
pub async fn get_working_diff(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<DiffFile>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_working_diff(&open.repository)
}
