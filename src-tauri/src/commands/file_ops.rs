//! Per-file actions from the staging panel's context menu.
use tauri::State;

use crate::error::TwigError;
use crate::git::file_ops::{self, IgnoreKind, IgnoreResult};
use crate::state::AppState;

/// Open a repo file with the system's default application.
#[tauri::command]
pub async fn open_repo_file(
    state: State<'_, AppState>,
    path: String,
    file: String,
) -> Result<(), TwigError> {
    let repo_path = state.repo_path(&path)?;
    let target = file_ops::resolve_in_repo(&repo_path, &file)?;
    file_ops::open_with_default_app(&target)
}

/// Show a repo file in the platform file manager.
#[tauri::command]
pub async fn reveal_repo_file(
    state: State<'_, AppState>,
    path: String,
    file: String,
) -> Result<(), TwigError> {
    let repo_path = state.repo_path(&path)?;
    let target = file_ops::resolve_in_repo(&repo_path, &file)?;
    file_ops::reveal_in_file_manager(&target).await
}

/// Add an ignore rule for `file` to the repository's root `.gitignore`.
/// `kind` is `"path"`, `"extension"` or `"folder"`.
#[tauri::command]
pub async fn add_to_gitignore(
    state: State<'_, AppState>,
    path: String,
    file: String,
    kind: String,
) -> Result<IgnoreResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let kind = IgnoreKind::parse(&kind)?;
    file_ops::add_to_gitignore(&repo_path, &file, kind).await
}

/// Launch an external diff tool for one file (`staged` compares HEAD with
/// the index, otherwise the index with the working tree).
#[tauri::command]
pub async fn open_external_diff(
    state: State<'_, AppState>,
    path: String,
    file: String,
    staged: bool,
    tool: Option<String>,
) -> Result<(), TwigError> {
    let repo_path = state.repo_path(&path)?;
    file_ops::open_external_diff(&repo_path, &file, staged, tool.as_deref()).await
}
