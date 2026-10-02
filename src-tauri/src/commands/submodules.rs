//! Submodule commands.

use tauri::State;

use crate::error::TwigError;
use crate::hosting::net_auth::with_network_auth;
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    sub_path: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    // The token header is host-scoped, so it only reaches submodules hosted
    // on the same provider host as the superproject.
    let out = with_network_auth(
        &app,
        &repo_path,
        submodules::submodule_update(&repo_path, sub_path.as_deref()),
    )
    .await?;
    Ok(out.into())
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
