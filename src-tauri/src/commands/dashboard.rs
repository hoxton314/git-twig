//! Tauri commands for the repository dashboard. They take any repository
//! path (not only open tabs); each is validated as a git working tree.
use crate::commands::staging::CommandResult;
use crate::error::TwigError;
use crate::git::dashboard::{self, RepoStatusRow};
use crate::hosting::net_auth::with_network_auth;

/// Branch, ahead/behind, changes and last fetch for each path.
#[tauri::command]
pub async fn get_dashboard_status(paths: Vec<String>) -> Result<Vec<RepoStatusRow>, TwigError> {
    tauri::async_runtime::spawn_blocking(move || dashboard::status_rows(&paths))
        .await
        .map_err(|e| TwigError::Task(e.to_string()))
}

fn result(out: crate::git::writer::GitOutput) -> CommandResult {
    CommandResult {
        success: out.success,
        message: if out.success { out.stdout } else { out.stderr },
    }
}

#[tauri::command]
pub async fn dashboard_fetch(app: tauri::AppHandle, path: String) -> Result<CommandResult, TwigError> {
    let dir = dashboard::validate_workdir(&path)?;
    Ok(result(with_network_auth(&app, &dir, dashboard::fetch(&dir)).await?))
}

/// Fast-forward the current branch (`git pull --ff-only`).
#[tauri::command]
pub async fn dashboard_pull(app: tauri::AppHandle, path: String) -> Result<CommandResult, TwigError> {
    let dir = dashboard::validate_workdir(&path)?;
    Ok(result(with_network_auth(&app, &dir, dashboard::pull_ff(&dir)).await?))
}
