//! Tauri command for code search (`git grep`).
use tauri::State;

use crate::error::TwigError;
use crate::git::grep::{self, GrepOptions, GrepResult};
use crate::state::AppState;

/// Search tracked files in the working tree, or a commit (`options.rev`).
#[tauri::command]
pub async fn search_code(
    state: State<'_, AppState>,
    path: String,
    options: GrepOptions,
) -> Result<GrepResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    grep::grep(&repo_path, &options).await
}
