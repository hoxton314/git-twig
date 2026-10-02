//! Tauri commands for saving and applying patch files.
use tauri::State;

use crate::error::TwigError;
use crate::git::patches::{self, ApplyPatchResult, PatchInfo};
use crate::state::AppState;

/// Save commits (oldest first) as patches: one file each in the folder
/// `target`, or all in the mbox file `target` with `single_file`.
#[tauri::command]
pub async fn format_patches(
    state: State<'_, AppState>,
    path: String,
    oids: Vec<String>,
    target: String,
    single_file: bool,
) -> Result<Vec<String>, TwigError> {
    let repo_path = state.repo_path(&path)?;
    patches::format_patches(&repo_path, &oids, &target, single_file).await
}

/// Save uncommitted changes to tracked files as a patch file.
#[tauri::command]
pub async fn save_working_patch(
    state: State<'_, AppState>,
    path: String,
    target: String,
) -> Result<(), TwigError> {
    let repo_path = state.repo_path(&path)?;
    patches::save_working_patch(&repo_path, &target).await
}

/// Summarise a patch file (kind, commits, stat, whether it applies).
#[tauri::command]
pub async fn inspect_patch(
    state: State<'_, AppState>,
    path: String,
    file: String,
) -> Result<PatchInfo, TwigError> {
    let repo_path = state.repo_path(&path)?;
    patches::inspect_patch(&repo_path, &file).await
}

/// Apply a patch file (`git am --3way` for mbox, else `git apply`).
#[tauri::command]
pub async fn apply_patch(
    state: State<'_, AppState>,
    path: String,
    file: String,
) -> Result<ApplyPatchResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    patches::apply_patch(&repo_path, &file).await
}
