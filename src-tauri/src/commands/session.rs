use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::commands::settings::{quarantine_corrupt, write_atomic};
use crate::error::TwigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub sidebar_width: Option<f64>,
    #[serde(default)]
    pub staging_width: Option<f64>,
    #[serde(default)]
    pub diff_panel_ratio: Option<f64>,
}

fn session_file(app: &tauri::AppHandle) -> Result<PathBuf, TwigError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| TwigError::Config(e.to_string()))?;
    Ok(dir.join("session.json"))
}

#[tauri::command]
pub async fn save_session(
    app: tauri::AppHandle,
    paths: Vec<String>,
    active: Option<String>,
    sidebar_width: Option<f64>,
    staging_width: Option<f64>,
    diff_panel_ratio: Option<f64>,
) -> Result<(), TwigError> {
    let file = session_file(&app)?;
    let session = Session {
        paths,
        active,
        sidebar_width,
        staging_width,
        diff_panel_ratio,
    };
    let json = serde_json::to_string_pretty(&session)?;
    write_atomic(&file, &json, false)?;
    Ok(())
}

#[tauri::command]
pub async fn load_session(app: tauri::AppHandle) -> Result<Session, TwigError> {
    let file = session_file(&app)?;
    let empty = Session {
        paths: vec![],
        active: None,
        sidebar_width: None,
        staging_width: None,
        diff_panel_ratio: None,
    };
    if !file.exists() {
        return Ok(empty);
    }
    let json = fs::read_to_string(&file)?;
    match serde_json::from_str::<Session>(&json) {
        Ok(session) => Ok(session),
        Err(e) => {
            // A corrupt session must not block startup; fall back to no tabs.
            quarantine_corrupt(&file, &e);
            Ok(empty)
        }
    }
}
