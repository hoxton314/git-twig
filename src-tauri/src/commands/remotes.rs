//! Remote management commands: list, add, remove, rename, set URLs, fetch, prune.
use tauri::State;

use crate::error::TwigError;
use crate::git::remotes::{self, RemoteInfo};
use crate::git::writer::GitOutput;
use crate::state::AppState;

use super::staging::CommandResult;

fn result(out: GitOutput, ok_message: &str) -> CommandResult {
    if out.success {
        CommandResult { success: true, message: ok_message.to_string() }
    } else {
        let err = out.stderr.trim();
        CommandResult {
            success: false,
            message: if err.is_empty() { out.stdout.trim().to_string() } else { err.to_string() },
        }
    }
}

#[tauri::command]
pub async fn list_remotes(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<RemoteInfo>, TwigError> {
    state.read_repo(&path, remotes::list_remotes).await
}

#[tauri::command]
pub async fn add_remote(
    state: State<'_, AppState>,
    path: String,
    name: String,
    url: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = remotes::add_remote(&repo_path, name.trim(), &url).await?;
    Ok(result(out, &format!("Added remote '{}'", name.trim())))
}

#[tauri::command]
pub async fn remove_remote(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = remotes::remove_remote(&repo_path, &name).await?;
    Ok(result(out, &format!("Removed remote '{name}'")))
}

#[tauri::command]
pub async fn rename_remote(
    state: State<'_, AppState>,
    path: String,
    old_name: String,
    new_name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = remotes::rename_remote(&repo_path, &old_name, new_name.trim()).await?;
    Ok(result(out, &format!("Renamed remote '{old_name}' to '{}'", new_name.trim())))
}

#[tauri::command]
pub async fn set_remote_urls(
    state: State<'_, AppState>,
    path: String,
    name: String,
    fetch_url: String,
    push_url: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out =
        remotes::set_remote_urls(&repo_path, &name, &fetch_url, push_url.as_deref()).await?;
    Ok(result(out, &format!("Updated URLs of '{name}'")))
}

#[tauri::command]
pub async fn fetch_remote(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = remotes::fetch_remote(&repo_path, &name).await?;
    Ok(result(out, &format!("Fetched '{name}'")))
}

#[tauri::command]
pub async fn prune_remote(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = remotes::prune_remote(&repo_path, &name).await?;
    let pruned = out.stdout.lines().filter(|l| l.contains("[pruned]")).count();
    let msg = match pruned {
        0 => format!("Nothing to prune on '{name}'"),
        1 => format!("Pruned 1 stale branch from '{name}'"),
        n => format!("Pruned {n} stale branches from '{name}'"),
    };
    Ok(result(out, &msg))
}
