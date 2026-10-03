//! Saved tabs per window (`session.json`), and opening extra windows.
//!
//! The file holds one session per window label (`"main"`, `"win-2"`, …).
//! Files written before multiple windows (a single top-level session) are
//! read as the main window's session.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::commands::settings::{quarantine_corrupt, write_atomic};
use crate::error::TwigError;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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

/// On-disk layout: per-window sessions, plus the pre-multi-window fields
/// (read only, as the main window's session).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct SessionFile {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub windows: BTreeMap<String, Session>,
    /// Only read: `set` moves it into `windows`.
    #[serde(flatten, skip_serializing)]
    pub legacy: Session,
}

pub const MAIN_WINDOW: &str = "main";

impl SessionFile {
    pub(crate) fn session_for(&self, label: &str) -> Session {
        match self.windows.get(label) {
            Some(s) => s.clone(),
            None if label == MAIN_WINDOW => self.legacy.clone(),
            None => Session::default(),
        }
    }

    pub(crate) fn set(&mut self, label: &str, session: Session) {
        // Once per-window sessions are written, the old fields are moved over.
        if self.windows.is_empty() && label != MAIN_WINDOW && self.legacy != Session::default() {
            self.windows.insert(MAIN_WINDOW.to_string(), std::mem::take(&mut self.legacy));
        }
        self.legacy = Session::default();
        self.windows.insert(label.to_string(), session);
    }

    pub(crate) fn forget(&mut self, label: &str) {
        self.windows.remove(label);
        if label == MAIN_WINDOW {
            self.legacy = Session::default();
        }
    }

    /// Windows to reopen at startup besides the main one.
    pub(crate) fn extra_windows(&self) -> Vec<String> {
        self.windows.keys().filter(|l| l.as_str() != MAIN_WINDOW).cloned().collect()
    }
}

/// Serialises read-modify-write of the file across windows.
static SESSION_LOCK: Mutex<()> = Mutex::new(());

fn session_file(app: &tauri::AppHandle) -> Result<PathBuf, TwigError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| TwigError::Config(e.to_string()))?;
    Ok(dir.join("session.json"))
}

fn read_file(file: &PathBuf) -> SessionFile {
    let Ok(json) = fs::read_to_string(file) else {
        return SessionFile::default();
    };
    match serde_json::from_str::<SessionFile>(&json) {
        Ok(s) => s,
        Err(e) => {
            // A corrupt session must not block startup; fall back to no tabs.
            quarantine_corrupt(file, &e);
            SessionFile::default()
        }
    }
}

fn update(app: &tauri::AppHandle, f: impl FnOnce(&mut SessionFile)) -> Result<(), TwigError> {
    let _guard = SESSION_LOCK.lock().map_err(|_| TwigError::Lock)?;
    let file = session_file(app)?;
    let mut data = read_file(&file);
    f(&mut data);
    write_atomic(&file, &serde_json::to_string_pretty(&data)?, false)
}

/// Save the calling window's tabs.
#[tauri::command]
pub async fn save_session(
    app: tauri::AppHandle,
    window: tauri::Window,
    paths: Vec<String>,
    active: Option<String>,
    sidebar_width: Option<f64>,
    staging_width: Option<f64>,
    diff_panel_ratio: Option<f64>,
) -> Result<(), TwigError> {
    let session = Session { paths, active, sidebar_width, staging_width, diff_panel_ratio };
    update(&app, |data| data.set(window.label(), session))
}

/// The calling window's saved tabs.
#[tauri::command]
pub async fn load_session(app: tauri::AppHandle, window: tauri::Window) -> Result<Session, TwigError> {
    let _guard = SESSION_LOCK.lock().map_err(|_| TwigError::Lock)?;
    Ok(read_file(&session_file(&app)?).session_for(window.label()))
}

/// The calling window is being closed while others stay open: drop its
/// saved tabs so it isn't reopened next time.
#[tauri::command]
pub async fn forget_window_session(app: tauri::AppHandle, window: tauri::Window) -> Result<(), TwigError> {
    update(&app, |data| data.forget(window.label()))
}

/// Labels of the extra windows saved last time (reopened by the main window).
#[tauri::command]
pub async fn saved_windows(app: tauri::AppHandle) -> Result<Vec<String>, TwigError> {
    let _guard = SESSION_LOCK.lock().map_err(|_| TwigError::Lock)?;
    Ok(read_file(&session_file(&app)?).extra_windows())
}

fn valid_label(label: &str) -> bool {
    label.starts_with("win-") && label.len() <= 32 && label[4..].bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Open another app window. `label` reopens a saved one; without it a new
/// label is chosen. Returns the label.
pub(crate) fn create_window(app: &tauri::AppHandle, label: Option<String>) -> Result<String, TwigError> {
    let label = match label {
        Some(l) if valid_label(&l) => l,
        Some(l) => return Err(TwigError::InvalidArgument(format!("'{l}' is not a window label"))),
        None => {
            // Not open and not saved by a closed-but-remembered window.
            let saved = {
                let _guard = SESSION_LOCK.lock().map_err(|_| TwigError::Lock)?;
                read_file(&session_file(app)?).windows
            };
            (2..)
                .map(|n| format!("win-{n}"))
                .find(|l| app.get_webview_window(l).is_none() && !saved.contains_key(l))
                .unwrap_or_else(|| "win-x".into())
        }
    };
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        return Ok(label);
    }
    tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::default())
        .title("Twig")
        .inner_size(1280.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .decorations(false)
        .build()
        .map_err(|e| TwigError::Config(format!("could not open a window: {e}")))?;
    Ok(label)
}

#[tauri::command]
pub async fn open_new_window(app: tauri::AppHandle, label: Option<String>) -> Result<String, TwigError> {
    create_window(&app, label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(paths: &[&str]) -> Session {
        Session { paths: paths.iter().map(|p| p.to_string()).collect(), ..Default::default() }
    }

    #[test]
    fn old_files_become_the_main_window() {
        let old: SessionFile = serde_json::from_str(r#"{"paths":["/a"],"active":"/a"}"#).unwrap();
        assert_eq!(old.session_for("main").paths, ["/a"]);
        assert!(old.session_for("win-2").paths.is_empty());
        assert!(old.extra_windows().is_empty());
    }

    #[test]
    fn windows_are_saved_independently() {
        let mut f: SessionFile = serde_json::from_str(r#"{"paths":["/a"]}"#).unwrap();
        // A second window saving first keeps the main window's old tabs.
        f.set("win-2", s(&["/b"]));
        assert_eq!(f.session_for("main").paths, ["/a"]);
        f.set("main", s(&["/a", "/c"]));
        assert_eq!(f.extra_windows(), ["win-2"]);
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.starts_with("{\"windows\":") && !json.contains("},\"paths\""), "only per-window sessions are written: {json}");
        let back: SessionFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_for("win-2").paths, ["/b"]);
        assert_eq!(back.session_for("main").paths, ["/a", "/c"]);
        f.forget("win-2");
        assert!(f.extra_windows().is_empty());
    }

    #[test]
    fn window_labels_are_checked() {
        assert!(valid_label("win-2") && valid_label("win-abc9"));
        assert!(!valid_label("main") && !valid_label("win-../x") && !valid_label("win-a b"));
    }
}
