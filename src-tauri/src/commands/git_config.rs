use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::error::TwigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub user_name: String,
    pub user_email: String,
    /// "false" = merge (default), "true" = rebase, "ff-only" = fast-forward only
    pub pull_rebase: String,
    pub fetch_prune: bool,
    pub gpg_sign: bool,
    pub signing_key: String,
    pub lfs_installed: bool,
}

async fn git_config_get(key: &str) -> String {
    let output = Command::new("git")
        .args(["config", "--global", "--get", key])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}

async fn git_config_get_bool(key: &str) -> bool {
    let val = git_config_get(key).await;
    matches!(val.as_str(), "true" | "1" | "yes")
}

async fn git_config_set(key: &str, value: &str) -> Result<(), TwigError> {
    let output = Command::new("git")
        .args(["config", "--global", key, value])
        .output()
        .await
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git config: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(TwigError::GitCli(format!(
            "git config --global {key} failed: {stderr}"
        )));
    }
    Ok(())
}

async fn git_config_set_bool(key: &str, value: bool) -> Result<(), TwigError> {
    // Explicitly set "true"/"false" rather than unsetting, because unsetting
    // from ~/.gitconfig won't override values in ~/.config/git/config (XDG).
    git_config_set(key, if value { "true" } else { "false" }).await
}

async fn detect_lfs() -> bool {
    let output = Command::new("git")
        .args(["lfs", "version"])
        .output()
        .await;

    matches!(output, Ok(o) if o.status.success())
}

#[tauri::command]
pub async fn get_git_config() -> Result<GitConfig, TwigError> {
    let (user_name, user_email, pull_rebase_raw, fetch_prune, gpg_sign, signing_key, lfs_installed) =
        tokio::join!(
            git_config_get("user.name"),
            git_config_get("user.email"),
            git_config_get("pull.rebase"),
            git_config_get_bool("fetch.prune"),
            git_config_get_bool("commit.gpgsign"),
            git_config_get("user.signingkey"),
            detect_lfs(),
        );

    // Normalize pull.rebase: empty means "false" (merge)
    let pull_rebase = if pull_rebase_raw.is_empty() {
        "false".to_string()
    } else {
        pull_rebase_raw
    };

    Ok(GitConfig {
        user_name,
        user_email,
        pull_rebase,
        fetch_prune,
        gpg_sign,
        signing_key,
        lfs_installed,
    })
}

#[tauri::command]
pub async fn set_git_config(config: GitConfig) -> Result<(), TwigError> {
    // Only set non-empty values; empty means unset / use default
    if !config.user_name.is_empty() {
        git_config_set("user.name", &config.user_name).await?;
    }
    if !config.user_email.is_empty() {
        git_config_set("user.email", &config.user_email).await?;
    }

    // pull.rebase — explicitly set all values to override any XDG config
    match config.pull_rebase.as_str() {
        "true" => {
            git_config_set("pull.rebase", "true").await?;
            git_config_set("pull.ff", "false").await?;
        }
        "ff-only" => {
            git_config_set("pull.rebase", "false").await?;
            git_config_set("pull.ff", "only").await?;
        }
        _ => {
            git_config_set("pull.rebase", "false").await?;
            git_config_set("pull.ff", "false").await?;
        }
    }

    git_config_set_bool("fetch.prune", config.fetch_prune).await?;
    git_config_set_bool("commit.gpgsign", config.gpg_sign).await?;

    if config.gpg_sign && !config.signing_key.is_empty() {
        git_config_set("user.signingkey", &config.signing_key).await?;
    }

    Ok(())
}
