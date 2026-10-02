use base64::Engine;
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
    options: Option<reader::DiffReadOptions>,
) -> Result<Vec<DiffFile>, TwigError> {
    let options = options.unwrap_or_default();
    state
        .read_repo(&path, move |repo| {
            reader::read_commit_diff_with(repo, &oid, &options)
        })
        .await
}

/// Diff between two commits (`from` → `to`), for comparing graph selections.
#[tauri::command]
pub async fn get_compare_diff(
    state: State<'_, AppState>,
    path: String,
    from: String,
    to: String,
    options: Option<reader::DiffReadOptions>,
) -> Result<Vec<DiffFile>, TwigError> {
    let options = options.unwrap_or_default();
    state
        .read_repo(&path, move |repo| {
            reader::read_compare_diff_with(repo, &from, &to, &options)
        })
        .await
}

/// Get raw file content as base64 from a given source (workdir, index, head, or commit OID).
/// Returns null if the file doesn't exist in that source.
/// Largest blob `get_file_blob` will return for image/audio previews.
const MAX_PREVIEW_BLOB_BYTES: usize = 25 * 1024 * 1024;

#[tauri::command]
pub async fn get_file_blob(
    state: State<'_, AppState>,
    path: String,
    file_path: String,
    source: String,
) -> Result<Option<String>, TwigError> {
    state
        .read_repo(&path, move |repo| {
            let data = reader::read_file_blob(repo, &file_path, &source)?;
            // Previews are base64-encoded across IPC; skip huge files rather
            // than freezing the webview. The frontend shows "no preview".
            Ok(data
                .filter(|bytes| bytes.len() <= MAX_PREVIEW_BLOB_BYTES)
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)))
        })
        .await
}

/// Get the working directory diff (staged + unstaged changes).
#[tauri::command]
pub async fn get_working_diff(
    state: State<'_, AppState>,
    path: String,
    options: Option<reader::DiffReadOptions>,
) -> Result<Vec<DiffFile>, TwigError> {
    let options = options.unwrap_or_default();
    state
        .read_repo(&path, move |repo| reader::read_working_diff_with(repo, &options))
        .await
}
