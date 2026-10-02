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
    /// `gpg.format`: "openpgp" (default), "ssh" or "x509".
    pub gpg_format: String,
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
    // `--type=bool` makes git canonicalise every accepted spelling
    // (yes/on/1/True/...) to "true"/"false".
    let output = Command::new("git")
        .args(["config", "--global", "--type=bool", "--get", key])
        .output()
        .await;
    matches!(output, Ok(o) if o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
}

/// Map raw `pull.rebase` / `pull.ff` values onto the three modes the UI offers.
fn normalize_pull_mode(rebase: &str, ff: &str) -> String {
    match rebase.to_ascii_lowercase().as_str() {
        // Any rebase flavour (true, merges, interactive, ...) except false.
        "" | "false" | "no" | "off" | "0" => {
            if ff.eq_ignore_ascii_case("only") {
                "ff-only".to_string()
            } else {
                "false".to_string()
            }
        }
        _ => "true".to_string(),
    }
}

async fn current_pull_mode() -> String {
    let (rebase, ff) = tokio::join!(git_config_get("pull.rebase"), git_config_get("pull.ff"));
    normalize_pull_mode(&rebase, &ff)
}

async fn git_config_set(key: &str, value: &str) -> Result<(), TwigError> {
    let output = Command::new("git")
        .args(["config", "--global", "--", key, value])
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

async fn git_config_unset(key: &str) -> Result<(), TwigError> {
    let output = Command::new("git")
        .args(["config", "--global", "--unset-all", "--", key])
        .output()
        .await
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git config: {e}")))?;
    // Exit code 5: the key was not set — nothing to do.
    if !output.status.success() && output.status.code() != Some(5) {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(TwigError::GitCli(format!("git config --global --unset {key} failed: {stderr}")));
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
    let (user_name, user_email, pull_rebase, fetch_prune, gpg_sign, signing_key, gpg_format, lfs_installed) =
        tokio::join!(
            git_config_get("user.name"),
            git_config_get("user.email"),
            current_pull_mode(),
            git_config_get_bool("fetch.prune"),
            git_config_get_bool("commit.gpgsign"),
            git_config_get("user.signingkey"),
            git_config_get("gpg.format"),
            detect_lfs(),
        );

    Ok(GitConfig {
        user_name,
        user_email,
        pull_rebase,
        fetch_prune,
        gpg_sign,
        signing_key,
        gpg_format: if gpg_format.is_empty() { "openpgp".to_string() } else { gpg_format },
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

    // pull.rebase / pull.ff — only touch them when the mode actually changed,
    // so saving an unrelated field doesn't clobber e.g. `pull.rebase=merges`.
    // When written, values are set explicitly to override any XDG config.
    // Note: `pull.ff=false` would force a merge commit on *every* pull (--no-ff),
    // so merge mode uses `pull.ff=true` (fast-forward when possible), git's default.
    if current_pull_mode().await != config.pull_rebase {
        match config.pull_rebase.as_str() {
            "true" => {
                git_config_set("pull.rebase", "true").await?;
                git_config_set("pull.ff", "true").await?;
            }
            "ff-only" => {
                git_config_set("pull.rebase", "false").await?;
                git_config_set("pull.ff", "only").await?;
            }
            _ => {
                git_config_set("pull.rebase", "false").await?;
                git_config_set("pull.ff", "true").await?;
            }
        }
    }

    git_config_set_bool("fetch.prune", config.fetch_prune).await?;
    git_config_set_bool("commit.gpgsign", config.gpg_sign).await?;

    // Only touch gpg.format when it changed (an unset value means openpgp);
    // values Twig doesn't offer are passed through untouched.
    let format = config.gpg_format.trim();
    if matches!(format, "openpgp" | "ssh" | "x509") {
        let current = git_config_get("gpg.format").await;
        let current = if current.is_empty() { "openpgp" } else { current.as_str() };
        if current != format {
            git_config_set("gpg.format", format).await?;
        }
    }
    // An emptied key is unset (e.g. after switching format), so git doesn't
    // keep signing with a key of the other kind.
    if !config.signing_key.is_empty() {
        git_config_set("user.signingkey", &config.signing_key).await?;
    } else if !git_config_get("user.signingkey").await.is_empty() {
        git_config_unset("user.signingkey").await?;
    }

    Ok(())
}
