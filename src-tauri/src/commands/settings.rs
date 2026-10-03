use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::credentials;
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
    #[serde(default = "default_theme")]
    pub theme: String,
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
    /// Show whitespace-only changes in diffs (git's default); off means `diff -w`.
    #[serde(default = "default_true")]
    pub show_whitespace_changes: bool,
    #[serde(default)]
    pub word_wrap_in_diffs: bool,
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,
    #[serde(default)]
    pub external_diff_tool: Option<String>,
    #[serde(default)]
    pub external_merge_tool: Option<String>,
    /// Command for "Open in terminal" (empty = platform default).
    #[serde(default)]
    pub terminal_command: Option<String>,
    /// Command for "Open in editor"; `{path}` is replaced by the target,
    /// otherwise it is appended (empty = `code`, else the system opener).
    #[serde(default)]
    pub editor_command: Option<String>,

    // ── Keybindings ─────────────────────────────────────────────────
    /// Custom keybinding overrides: action_id -> shortcut string (e.g. "Ctrl+Enter")
    #[serde(default)]
    pub keybinding_overrides: HashMap<String, String>,

    // ── GitHub ──────────────────────────────────────────────────────
    /// Plaintext token from settings files written before the token moved
    /// to the OS keyring. Read only for migration; never serialized, so it
    /// is neither sent to the webview nor written back to disk.
    #[serde(default, rename = "github_token", skip_serializing)]
    pub legacy_github_token: Option<String>,

    // ── Staging panel ───────────────────────────────────────────────
    /// Show the staged/unstaged file lists as a folder tree instead of flat.
    #[serde(default)]
    pub staging_tree_view: bool,
    // ── Commit graph view ───────────────────────────────────────────
    #[serde(default = "default_true")]
    pub graph_show_author: bool,
    #[serde(default = "default_true")]
    pub graph_show_date: bool,
    #[serde(default = "default_true")]
    pub graph_show_sha: bool,
    #[serde(default = "default_graph_author_width")]
    pub graph_author_width: u32,
    #[serde(default = "default_graph_sha_width")]
    pub graph_sha_width: u32,
    #[serde(default = "default_graph_date_width")]
    pub graph_date_width: u32,
    /// "relative" | "iso" | "locale"
    #[serde(default = "default_graph_date_format")]
    pub graph_date_format: String,
    /// "compact" | "normal" | "comfortable"
    #[serde(default = "default_graph_row_density")]
    pub graph_row_density: String,
    #[serde(default)]
    pub graph_hide_remotes: bool,
    #[serde(default)]
    pub graph_current_branch_only: bool,
    // ── App shell: fonts & updater ──────────────────────────────────
    /// Interface font family (CSS font-family list); empty = system UI font.
    #[serde(default)]
    pub ui_font_family: String,
    /// Monospace font family for code/diffs/hashes; empty = system monospace.
    #[serde(default)]
    pub mono_font_family: String,
    #[serde(default = "default_true")]
    pub check_updates_on_startup: bool,
    /// Release version the user chose to skip in the update prompt.
    #[serde(default)]
    pub skipped_update_version: Option<String>,
    // ── Hosting integrations ────────────────────────────────────────
    /// Authenticate HTTPS fetch/pull/push to the GitHub host with the
    /// keyring token (passed via environment-only git config).
    #[serde(default = "default_true")]
    pub github_https_auth: bool,
    /// GitHub host: `github.com` or a GitHub Enterprise Server hostname.
    #[serde(default = "default_github_host")]
    pub github_host: String,
    /// Optional REST API base override (default: derived from the host).
    #[serde(default)]
    pub github_api_url: String,
    /// GitLab instance URL (gitlab.com or self-hosted).
    #[serde(default = "default_gitlab_url")]
    pub gitlab_base_url: String,
    /// Gitea / Forgejo instance URL (empty = not configured).
    #[serde(default)]
    pub gitea_base_url: String,
    // ── Diff viewer: syntax highlighting ─────────────────────────────
    #[serde(default = "default_true")]
    pub syntax_highlighting: bool,
    // ── Notifications ────────────────────────────────────────────────
    /// New upstream commits after an auto-fetch: "off" | "current" (the
    /// checked-out branch) | "all" (every branch with an upstream).
    #[serde(default = "default_notify_new_commits")]
    pub notify_new_commits: String,
    /// CI finished on the checked-out commit of an open repository:
    /// "off" | "failures" | "all".
    #[serde(default = "default_notify_ci")]
    pub notify_ci: String,
}

fn default_notify_ci() -> String {
    "all".to_string()
}

fn default_notify_new_commits() -> String {
    "current".to_string()
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
fn default_theme() -> String {
    "dark".to_string()
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
// Commit graph view defaults
fn default_graph_author_width() -> u32 {
    120
}
fn default_graph_sha_width() -> u32 {
    64
}
fn default_graph_date_width() -> u32 {
    90
}
fn default_graph_date_format() -> String {
    "relative".to_string()
}
fn default_graph_row_density() -> String {
    "normal".to_string()
}

fn default_github_host() -> String {
    "github.com".to_string()
}
fn default_gitlab_url() -> String {
    "https://gitlab.com".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_repo_dir: None,
            auto_fetch_interval: default_auto_fetch_interval(),
            max_commits: default_max_commits(),
            confirm_destructive_ops: true,
            restore_tabs_on_startup: true,
            theme: default_theme(),
            accent_color: default_accent_color(),
            font_size: default_font_size(),
            diff_font_size: default_font_size(),
            diff_view_mode: default_diff_view(),
            tab_size: default_tab_size(),
            show_whitespace_changes: true,
            word_wrap_in_diffs: false,
            context_lines: default_context_lines(),
            external_diff_tool: None,
            external_merge_tool: None,
            terminal_command: None,
            editor_command: None,
            keybinding_overrides: HashMap::new(),
            legacy_github_token: None,
            staging_tree_view: false,
            // Commit graph view
            graph_show_author: true,
            graph_show_date: true,
            graph_show_sha: true,
            graph_author_width: default_graph_author_width(),
            graph_sha_width: default_graph_sha_width(),
            graph_date_width: default_graph_date_width(),
            graph_date_format: default_graph_date_format(),
            graph_row_density: default_graph_row_density(),
            graph_hide_remotes: false,
            graph_current_branch_only: false,
            ui_font_family: String::new(),
            mono_font_family: String::new(),
            check_updates_on_startup: true,
            skipped_update_version: None,
            // Hosting integrations
            github_https_auth: true,
            github_host: default_github_host(),
            github_api_url: String::new(),
            gitlab_base_url: default_gitlab_url(),
            gitea_base_url: String::new(),
            syntax_highlighting: true,
            notify_new_commits: default_notify_new_commits(),
            notify_ci: default_notify_ci(),
        }
    }
}

pub(crate) fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, TwigError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| TwigError::Config(e.to_string()))?;
    Ok(dir.join("settings.json"))
}

/// Serialises writers of the app-data JSON files so concurrent saves can't
/// interleave on the shared temp file.
static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Atomically replace `path` with `contents`: write a sibling temp file,
/// fsync it, then rename over the target. A crash mid-write can therefore
/// never leave a truncated/corrupt file behind. When `private` is set the
/// file is created owner-read/write only (it may contain secrets).
pub(crate) fn write_atomic(path: &Path, contents: &str, private: bool) -> Result<(), TwigError> {
    let _guard = WRITE_LOCK.lock().map_err(|_| TwigError::Lock)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp_name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;

    // Remove any stale temp file so `mode` applies to a freshly created file.
    let _ = fs::remove_file(&tmp);
    let result = (|| -> Result<(), TwigError> {
        let mut f = opts.open(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Move an unreadable JSON file aside (so it is not silently overwritten and
/// can be inspected/recovered) and log why.
pub(crate) fn quarantine_corrupt(path: &Path, err: &serde_json::Error) {
    let mut bad_name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    bad_name.push(".corrupt");
    let bad = path.with_file_name(bad_name);
    log::warn!(
        "{} is not valid ({err}); moved to {} and using defaults",
        path.display(),
        bad.display()
    );
    let _ = fs::rename(path, bad);
}

#[tauri::command]
pub async fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, TwigError> {
    let file = settings_file(&app)?;
    if !file.exists() {
        return Ok(AppSettings::default());
    }
    let json = fs::read_to_string(&file)?;
    let mut settings = match serde_json::from_str::<AppSettings>(&json) {
        Ok(settings) => settings,
        Err(e) => {
            quarantine_corrupt(&file, &e);
            return Ok(AppSettings::default());
        }
    };

    // One-time move of a plaintext token into the OS keyring. On failure the
    // file is left untouched (and `save_settings` keeps the token) so it is
    // not lost; `github::get_token` falls back to it.
    if let Some(token) = settings.legacy_github_token.take().filter(|t| !t.is_empty()) {
        match credentials::set_github_token(Some(token)).await {
            Ok(()) => write_atomic(&file, &serde_json::to_string_pretty(&settings)?, true)?,
            Err(e) => log::warn!("could not migrate GitHub token to the OS keyring: {e}"),
        }
    }
    Ok(settings)
}

/// Current settings from disk (defaults when missing or unreadable).
pub(crate) fn load_settings_from_disk(app: &tauri::AppHandle) -> AppSettings {
    settings_file(app)
        .ok()
        .and_then(|f| fs::read_to_string(f).ok())
        .and_then(|json| serde_json::from_str::<AppSettings>(&json).ok())
        .unwrap_or_default()
}

/// The not-yet-migrated plaintext token in `settings.json`, if any.
pub(crate) fn read_legacy_token(app: &tauri::AppHandle) -> Option<String> {
    let json = fs::read_to_string(settings_file(app).ok()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    value
        .get("github_token")?
        .as_str()
        .filter(|t| !t.is_empty())
        .map(String::from)
}

#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<(), TwigError> {
    let file = settings_file(&app)?;
    let mut value = serde_json::to_value(&settings)?;
    // Keep an unmigrated legacy token rather than silently dropping it.
    if let (Some(token), Some(obj)) = (read_legacy_token(&app), value.as_object_mut()) {
        obj.insert("github_token".into(), token.into());
    }
    let json = serde_json::to_string_pretty(&value)?;
    // Owner-only: older files may still hold a plaintext token.
    write_atomic(&file, &json, true)?;
    Ok(())
}

/// Per-repository overrides (repo path → setting overrides), kept next to
/// `settings.json`. The frontend owns which keys are allowed; this only
/// guarantees the file stays a map of objects.
pub type RepoSettingsMap = std::collections::BTreeMap<String, serde_json::Map<String, serde_json::Value>>;

fn repo_settings_file(app: &tauri::AppHandle) -> Result<PathBuf, TwigError> {
    Ok(settings_file(app)?.with_file_name("repo_settings.json"))
}

/// Parse the overrides file; anything that isn't a map of objects is invalid.
pub(crate) fn parse_repo_settings(json: &str) -> Result<RepoSettingsMap, serde_json::Error> {
    serde_json::from_str(json)
}

#[tauri::command]
pub async fn load_repo_settings(app: tauri::AppHandle) -> Result<RepoSettingsMap, TwigError> {
    let file = repo_settings_file(&app)?;
    if !file.exists() {
        return Ok(RepoSettingsMap::new());
    }
    let json = fs::read_to_string(&file)?;
    match parse_repo_settings(&json) {
        Ok(map) => Ok(map),
        Err(e) => {
            quarantine_corrupt(&file, &e);
            Ok(RepoSettingsMap::new())
        }
    }
}

#[tauri::command]
pub async fn save_repo_settings(app: tauri::AppHandle, overrides: RepoSettingsMap) -> Result<(), TwigError> {
    // Repositories without overrides are dropped rather than kept as `{}`.
    let overrides: RepoSettingsMap = overrides.into_iter().filter(|(_, v)| !v.is_empty()).collect();
    let json = serde_json::to_string_pretty(&overrides)?;
    write_atomic(&repo_settings_file(&app)?, &json, false)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_settings_must_be_a_map_of_objects() {
        let map = parse_repo_settings(r#"{"/a":{"context_lines":10},"/b":{}}"#).unwrap();
        assert_eq!(map["/a"]["context_lines"], 10);
        assert!(parse_repo_settings(r#"{"/a":5}"#).is_err());
        assert!(parse_repo_settings("[]").is_err());
    }

    #[test]
    fn missing_fields_take_the_documented_defaults() -> Result<(), serde_json::Error> {
        let parsed: AppSettings = serde_json::from_str("{}")?;
        let default = AppSettings::default();
        assert!(parsed.show_whitespace_changes);
        assert!(default.show_whitespace_changes);
        assert_eq!(
            serde_json::to_value(&parsed)?,
            serde_json::to_value(&default)?
        );
        Ok(())
    }

    #[test]
    fn explicit_whitespace_choice_is_kept() -> Result<(), serde_json::Error> {
        let parsed: AppSettings = serde_json::from_str(r#"{"show_whitespace_changes": false}"#)?;
        assert!(!parsed.show_whitespace_changes);
        Ok(())
    }
}
