use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::error::TwigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub paths: Vec<String>,
    pub active: Option<String>,
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
) -> Result<(), TwigError> {
    let file = session_file(&app)?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let session = Session { paths, active };
    let json = serde_json::to_string_pretty(&session)?;
    fs::write(&file, json)?;
    Ok(())
}

#[tauri::command]
pub async fn load_session(app: tauri::AppHandle) -> Result<Session, TwigError> {
    let file = session_file(&app)?;
    if !file.exists() {
        return Ok(Session {
            paths: vec![],
            active: None,
        });
    }
    let json = fs::read_to_string(&file)?;
    let session: Session = serde_json::from_str(&json)?;
    Ok(session)
}
