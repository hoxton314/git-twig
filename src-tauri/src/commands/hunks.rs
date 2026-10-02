use tauri::State;

use crate::commands::staging::CommandResult;
use crate::error::TwigError;
use crate::git::hunks::{self, DiffArea, HunkAction, HunkRange, SelectedLine};
use crate::state::AppState;

/// Stage, unstage or discard a subset of one file's changes.
///
/// `lines` are explicitly selected changed lines (verified against the
/// current diff); every change inside `ranges` (whole hunks) is included too.
#[tauri::command]
pub async fn apply_diff_selection(
    state: State<'_, AppState>,
    path: String,
    file_path: String,
    area: DiffArea,
    action: HunkAction,
    lines: Vec<SelectedLine>,
    ranges: Vec<HunkRange>,
) -> Result<CommandResult, TwigError> {
    let (cached, reverse) = hunks::apply_mode(area, action)?;
    let repo_path = state.repo_path(&path)?;
    let patch = state
        .read_repo(&path, move |repo| {
            let src = hunks::read_patch_source(repo, area, &file_path)?;
            let selected = hunks::resolve_selection(&src, &lines, &ranges)?;
            Ok(hunks::build_patch(&src, &selected, reverse))
        })
        .await?;
    let Some(patch) = patch else {
        return Ok(CommandResult {
            success: false,
            message: "No changes selected".to_string(),
        });
    };
    let output = hunks::apply_patch(&repo_path, &patch, cached, reverse).await?;
    Ok(output.into())
}
