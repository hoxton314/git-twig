//! Tauri commands for `git bisect`.
use tauri::State;

use crate::commands::staging::CommandResult;
use crate::error::TwigError;
use crate::git::bisect::{self, BisectInfo};
use crate::git::conflicts::read_operation_state;
use crate::git::writer::GitOutput;
use crate::state::AppState;

/// git prints bisect progress ("Bisecting: …", "… is the first bad commit")
/// on stdout, and errors on stderr.
fn result(out: GitOutput) -> CommandResult {
    CommandResult {
        success: out.success,
        message: if out.success { out.stdout } else { format!("{}{}", out.stderr, out.stdout) },
    }
}

/// The bisect in progress (marks, candidates left, first bad commit), if any.
#[tauri::command]
pub async fn get_bisect_state(
    state: State<'_, AppState>,
    path: String,
) -> Result<Option<BisectInfo>, TwigError> {
    let repo_path = state.repo_path(&path)?;
    bisect::bisect_info(&repo_path).await
}

/// Start a bisect from a bad and/or good commit.
#[tauri::command]
pub async fn bisect_start(
    state: State<'_, AppState>,
    path: String,
    bad: Option<String>,
    good: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let kind = state.read_repo(&path, |repo| Ok(read_operation_state(repo)?.kind)).await?;
    if kind != "none" {
        return Ok(CommandResult {
            success: false,
            message: format!("Finish the {} in progress first.", kind.replace('_', "-")),
        });
    }
    Ok(result(bisect::start(&repo_path, bad.as_deref(), good.as_deref()).await?))
}

/// Mark a commit (default HEAD) as good, bad or skip.
#[tauri::command]
pub async fn bisect_mark(
    state: State<'_, AppState>,
    path: String,
    verdict: String,
    rev: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(result(bisect::mark(&repo_path, &verdict, rev.as_deref()).await?))
}

/// End the bisect and check out where it started.
#[tauri::command]
pub async fn bisect_reset(state: State<'_, AppState>, path: String) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(result(bisect::reset(&repo_path).await?))
}
