//! Tauri commands for Git LFS (patterns, locks, fetch / prune). Commands that
//! talk to the LFS server run with the app's network auth for the remote.
use tauri::State;

use crate::error::TwigError;
use crate::git::lfs::{self, LfsLock, LfsStatus};
use crate::hosting::net_auth::with_network_auth;
use crate::state::AppState;

/// git-lfs version (null when not installed) and the tracked patterns.
#[tauri::command]
pub async fn get_lfs_status(state: State<'_, AppState>, path: String) -> Result<LfsStatus, TwigError> {
    let repo_path = state.repo_path(&path)?;
    lfs::status(&repo_path).await
}

#[tauri::command]
pub async fn lfs_track(
    state: State<'_, AppState>,
    path: String,
    pattern: String,
    lockable: bool,
) -> Result<String, TwigError> {
    let repo_path = state.repo_path(&path)?;
    lfs::track(&repo_path, &pattern, lockable).await
}

#[tauri::command]
pub async fn lfs_untrack(state: State<'_, AppState>, path: String, pattern: String) -> Result<String, TwigError> {
    let repo_path = state.repo_path(&path)?;
    lfs::untrack(&repo_path, &pattern).await
}

/// Locks on the LFS server, marked ours / theirs.
#[tauri::command]
pub async fn get_lfs_locks(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<LfsLock>, TwigError> {
    let repo_path = state.repo_path(&path)?;
    with_network_auth(&app, &repo_path, lfs::locks(&repo_path)).await
}

#[tauri::command]
pub async fn lfs_lock(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    file: String,
) -> Result<(), TwigError> {
    let repo_path = state.repo_path(&path)?;
    with_network_auth(&app, &repo_path, lfs::lock(&repo_path, &file)).await
}

#[tauri::command]
pub async fn lfs_unlock(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    id: String,
    force: bool,
) -> Result<(), TwigError> {
    let repo_path = state.repo_path(&path)?;
    with_network_auth(&app, &repo_path, lfs::unlock(&repo_path, &id, force)).await
}

#[tauri::command]
pub async fn lfs_fetch(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    all: bool,
) -> Result<String, TwigError> {
    let repo_path = state.repo_path(&path)?;
    with_network_auth(&app, &repo_path, lfs::fetch(&repo_path, all)).await
}

/// Delete old local LFS objects (`dry_run`: only report). With
/// `lfs.pruneverifyremotealways` prune checks the server, hence the auth.
#[tauri::command]
pub async fn lfs_prune(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    dry_run: bool,
) -> Result<String, TwigError> {
    let repo_path = state.repo_path(&path)?;
    with_network_auth(&app, &repo_path, lfs::prune(&repo_path, dry_run)).await
}
