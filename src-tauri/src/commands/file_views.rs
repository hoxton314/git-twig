//! File history & blame commands.

use tauri::State;

use crate::error::TwigError;
use crate::git::file_history::{self, BlameResult, FileHistoryPage};
use crate::git::reader::DiffFile;
use crate::state::AppState;

/// Upper bound on one page of file history.
const MAX_HISTORY_PAGE: usize = 500;

/// One page of commits that touched `file_path` (following renames).
#[tauri::command]
pub async fn get_file_history(
    state: State<'_, AppState>,
    path: String,
    file_path: String,
    rev: Option<String>,
    skip: usize,
    limit: usize,
) -> Result<FileHistoryPage, TwigError> {
    let limit = limit.clamp(1, MAX_HISTORY_PAGE);
    state
        .read_repo(&path, move |repo| {
            file_history::read_file_history(repo, &file_path, rev.as_deref(), skip, limit)
        })
        .await
}

/// Diff of one file in one commit (pass `old_path` for renames).
#[tauri::command]
pub async fn get_file_diff_at_commit(
    state: State<'_, AppState>,
    path: String,
    oid: String,
    file_path: String,
    old_path: Option<String>,
) -> Result<Vec<DiffFile>, TwigError> {
    state
        .read_repo(&path, move |repo| {
            file_history::read_file_diff_at(repo, &oid, &file_path, old_path.as_deref())
        })
        .await
}

/// Line-by-line blame of `file_path` at `rev` (default HEAD).
#[tauri::command]
pub async fn get_blame(
    state: State<'_, AppState>,
    path: String,
    file_path: String,
    rev: Option<String>,
) -> Result<BlameResult, TwigError> {
    state
        .read_repo(&path, move |repo| file_history::read_blame(repo, &file_path, rev.as_deref()))
        .await
}

/// All tracked file paths (for the file picker).
#[tauri::command]
pub async fn list_tracked_files(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<String>, TwigError> {
    state.read_repo(&path, file_history::list_tracked_files).await
}
