//! App-shell commands: status bar summary, recent/favorite repositories,
//! settings folder & import/export, and revealing folders in the OS file
//! manager.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::commands::settings::{quarantine_corrupt, write_atomic, AppSettings};
use crate::error::TwigError;
use crate::git::repo_state::{repo_status_summary, RepoStatusSummary};
use crate::state::AppState;

// ── Status bar ───────────────────────────────────────────────────────

/// Branch / detached HEAD, upstream ahead-behind and in-progress operation
/// (merge, rebase, ...) of an open repository.
#[tauri::command]
pub async fn get_repo_status_summary(
    state: State<'_, AppState>,
    path: String,
) -> Result<RepoStatusSummary, TwigError> {
    state.read_repo(&path, repo_status_summary).await
}

// ── Recent & favorite repositories ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentRepo {
    pub path: String,
    pub name: String,
    /// Unix time in milliseconds when the repo was last opened.
    #[serde(default)]
    pub last_opened: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RepoHistory {
    #[serde(default)]
    pub recent: Vec<RecentRepo>,
    /// Pinned repository paths, in display order.
    #[serde(default)]
    pub favorites: Vec<String>,
}

/// Recent list is capped so the file stays small.
const MAX_RECENT: usize = 30;

fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, TwigError> {
    app.path()
        .app_data_dir()
        .map_err(|e| TwigError::Config(e.to_string()))
}

fn history_file(app: &tauri::AppHandle) -> Result<PathBuf, TwigError> {
    Ok(app_data_dir(app)?.join("repo_history.json"))
}

#[tauri::command]
pub async fn load_repo_history(app: tauri::AppHandle) -> Result<RepoHistory, TwigError> {
    let file = history_file(&app)?;
    if !file.exists() {
        return Ok(RepoHistory::default());
    }
    let json = fs::read_to_string(&file)?;
    match serde_json::from_str::<RepoHistory>(&json) {
        Ok(h) => Ok(h),
        Err(e) => {
            quarantine_corrupt(&file, &e);
            Ok(RepoHistory::default())
        }
    }
}

#[tauri::command]
pub async fn save_repo_history(
    app: tauri::AppHandle,
    mut history: RepoHistory,
) -> Result<(), TwigError> {
    history.recent.truncate(MAX_RECENT);
    let file = history_file(&app)?;
    let json = serde_json::to_string_pretty(&history)?;
    write_atomic(&file, &json, false)?;
    Ok(())
}

/// For each path, whether it still exists as a directory on disk.
#[tauri::command]
pub async fn repo_paths_exist(paths: Vec<String>) -> Result<Vec<bool>, TwigError> {
    Ok(paths.iter().map(|p| Path::new(p).is_dir()).collect())
}

// ── Opening folders ──────────────────────────────────────────────────

/// Open a directory in the platform file manager.
async fn open_dir(dir: &Path) -> Result<(), TwigError> {
    if !dir.is_dir() {
        return Err(TwigError::InvalidArgument(format!(
            "not a directory: {}",
            dir.display()
        )));
    }
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let opener = "xdg-open";

    let child = tokio::process::Command::new(opener)
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| TwigError::Config(format!("could not run {opener}: {e}")))?;
    // Reap the launcher in the background; it may outlive this call.
    tauri::async_runtime::spawn(async move {
        let mut child = child;
        let _ = child.wait().await;
    });
    Ok(())
}

/// Reveal an open repository's working directory in the file manager.
#[tauri::command]
pub async fn open_in_file_manager(path: String) -> Result<(), TwigError> {
    open_dir(Path::new(&path)).await
}

/// Open the folder holding settings.json / session.json.
#[tauri::command]
pub async fn open_settings_folder(app: tauri::AppHandle) -> Result<String, TwigError> {
    let dir = app_data_dir(&app)?;
    fs::create_dir_all(&dir)?;
    open_dir(&dir).await?;
    Ok(dir.to_string_lossy().to_string())
}

// ── Settings import / export ─────────────────────────────────────────

/// Write `settings` to a user-chosen JSON file. Secrets are never part of
/// `AppSettings` serialization (the GitHub token lives in the OS keyring
/// and the legacy field is `skip_serializing`).
#[tauri::command]
pub async fn export_settings(path: String, settings: AppSettings) -> Result<(), TwigError> {
    let json = serde_json::to_string_pretty(&settings)?;
    write_atomic(Path::new(&path), &json, false)?;
    Ok(())
}

/// Read settings from a JSON file. Missing fields take their defaults; a
/// plaintext token in the file is ignored (never returned to the webview).
#[tauri::command]
pub async fn import_settings(path: String) -> Result<AppSettings, TwigError> {
    let json = fs::read_to_string(&path)?;
    let mut settings: AppSettings = serde_json::from_str(&json)
        .map_err(|e| TwigError::InvalidArgument(format!("not a Twig settings file: {e}")))?;
    settings.legacy_github_token = None;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_roundtrip_and_defaults() {
        let h: RepoHistory = serde_json::from_str("{}").unwrap_or_default();
        assert!(h.recent.is_empty() && h.favorites.is_empty());
        let h = RepoHistory {
            recent: vec![RecentRepo { path: "/a".into(), name: "a".into(), last_opened: 1.0 }],
            favorites: vec!["/a".into()],
        };
        let json = serde_json::to_string(&h).unwrap_or_default();
        let back: RepoHistory = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(back, h);
    }

    #[test]
    fn exported_settings_never_contain_token() {
        let s = AppSettings {
            legacy_github_token: Some("secret".into()),
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&s).unwrap_or_default();
        assert!(!json.contains("secret"));
        assert!(!json.contains("github_token"));
    }
}
