//! Submodule commands.

use tauri::State;

use crate::error::TwigError;
use crate::git::submodules::{self, SubmoduleInfo};
use crate::state::AppState;

use super::staging::CommandResult;

#[tauri::command]
pub async fn list_submodules(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<SubmoduleInfo>, TwigError> {
    state.read_repo(&path, submodules::list_submodules).await
}

/// Init + update (recursive). `sub_path` limits it to one submodule.
#[tauri::command]
pub async fn submodule_update(
    state: State<'_, AppState>,
    path: String,
    sub_path: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(submodules::submodule_update(&repo_path, sub_path.as_deref()).await?.into())
}

#[tauri::command]
pub async fn submodule_sync(
    state: State<'_, AppState>,
    path: String,
    sub_path: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    Ok(submodules::submodule_sync(&repo_path, sub_path.as_deref()).await?.into())
}
