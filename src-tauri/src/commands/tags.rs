//! Tag commands: list, create, delete, push, delete on remote.
use tauri::State;

use super::staging::CommandResult;
use crate::error::TwigError;
use crate::git::tags::{self, TagInfo};
use crate::state::AppState;

#[tauri::command]
pub async fn get_tags(state: State<'_, AppState>, path: String) -> Result<Vec<TagInfo>, TwigError> {
    state.read_repo(&path, tags::read_tags).await
}

/// Create a tag at `target`; a non-empty `message` makes it annotated.
#[tauri::command]
pub async fn create_tag(
    state: State<'_, AppState>,
    path: String,
    name: String,
    target: String,
    message: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(tags::create_tag(&repo_path, &name, &target, message.as_deref()).await?.into())
}

#[tauri::command]
pub async fn delete_tag(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(tags::delete_tag(&repo_path, &name).await?.into())
}

/// Push one tag (or all tags when `name` is None). `remote` defaults to
/// `origin` or the only remote.
#[tauri::command]
pub async fn push_tag(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
    remote: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let remote = tags::resolve_remote(&repo_path, remote.as_deref()).await?;
    let out = tags::push_tags(&repo_path, &remote, name.as_deref()).await?;
    Ok(CommandResult {
        success: out.success,
        message: if out.success {
            format!("Pushed to {remote}")
        } else {
            out.stderr
        },
    })
}

#[tauri::command]
pub async fn delete_remote_tag(
    state: State<'_, AppState>,
    path: String,
    name: String,
    remote: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let remote = tags::resolve_remote(&repo_path, remote.as_deref()).await?;
    let out = tags::delete_remote_tag(&repo_path, &remote, &name).await?;
    Ok(CommandResult {
        success: out.success,
        message: if out.success {
            format!("Deleted tag '{name}' from {remote}")
        } else {
            out.stderr
        },
    })
}
