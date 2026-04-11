use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::error::TwigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // ── General ──────────────────────────────────────────────────────
    #[serde(default)]
    pub default_repo_dir: Option<String>,
    #[serde(default = "default_auto_fetch_interval")]
    pub auto_fetch_interval: u32,
    #[serde(default = "default_max_commits")]
    pub max_commits: u32,
    #[serde(default = "default_true")]
    pub confirm_destructive_ops: bool,
    #[serde(default = "default_true")]
    pub restore_tabs_on_startup: bool,

    // ── Appearance ───────────────────────────────────────────────────
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_font_size")]
    pub diff_font_size: u32,

    // ── Editor & Diff ────────────────────────────────────────────────
    #[serde(default = "default_diff_view")]
    pub diff_view_mode: String,
    #[serde(default = "default_tab_size")]
    pub tab_size: u32,
    #[serde(default)]
    pub show_whitespace_changes: bool,
    #[serde(default)]
    pub word_wrap_in_diffs: bool,
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,
    #[serde(default)]
    pub external_diff_tool: Option<String>,
    #[serde(default)]
    pub external_merge_tool: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_auto_fetch_interval() -> u32 {
    0
}
fn default_max_commits() -> u32 {
    5000
}
fn default_accent_color() -> String {
    "#7aa2f7".to_string()
}
fn default_font_size() -> u32 {
    13
}
fn default_diff_view() -> String {
    "unified".to_string()
}
fn default_tab_size() -> u32 {
    4
}
fn default_context_lines() -> u32 {
    3
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_repo_dir: None,
            auto_fetch_interval: default_auto_fetch_interval(),
            max_commits: default_max_commits(),
            confirm_destructive_ops: true,
            restore_tabs_on_startup: true,
            accent_color: default_accent_color(),
            font_size: default_font_size(),
            diff_font_size: default_font_size(),
            diff_view_mode: default_diff_view(),
            tab_size: default_tab_size(),
            show_whitespace_changes: false,
            word_wrap_in_diffs: false,
            context_lines: default_context_lines(),
            external_diff_tool: None,
            external_merge_tool: None,
        }
    }
}

fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, TwigError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| TwigError::Config(e.to_string()))?;
    Ok(dir.join("settings.json"))
}

#[tauri::command]
pub async fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, TwigError> {
    let file = settings_file(&app)?;
    if !file.exists() {
        return Ok(AppSettings::default());
    }
    let json = fs::read_to_string(&file)?;
    let settings: AppSettings = serde_json::from_str(&json)?;
    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<(), TwigError> {
    let file = settings_file(&app)?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&settings)?;
    fs::write(&file, json)?;
    Ok(())
}
