use serde::Serialize;
use tauri::State;

use crate::error::TwigError;
use crate::git::writer;
use crate::state::AppState;

use super::staging::CommandResult;

#[derive(Debug, Serialize)]
pub struct StashEntry {
    pub index: u32,
    pub reference: String,
    pub message: String,
    pub timestamp: String,
}

/// List all stash entries.
#[tauri::command]
pub async fn stash_list(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<StashEntry>, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::stash_list(&repo_path).await?;
    if !output.success || output.stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    for line in output.stdout.lines() {
        // Format: stash@{N}\0message\0ISO-timestamp
        let parts: Vec<&str> = line.splitn(3, '\0').collect();
        if parts.len() < 3 {
            continue;
        }
        let reference = parts[0].to_string();
        let index = reference
            .trim_start_matches("stash@{")
            .trim_end_matches('}')
            .parse::<u32>()
            .unwrap_or(0);
        entries.push(StashEntry {
            index,
            reference,
            message: parts[1].to_string(),
            timestamp: parts[2].to_string(),
        });
    }

    Ok(entries)
}

/// Push current changes to the stash.
#[tauri::command]
pub async fn stash_push(
    state: State<'_, AppState>,
    path: String,
    message: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::stash_push(&repo_path, message.as_deref()).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Pop a stash entry (apply + remove).
#[tauri::command]
pub async fn stash_pop(
    state: State<'_, AppState>,
    path: String,
    index: u32,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::stash_pop(&repo_path, index).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Apply a stash entry without removing it.
#[tauri::command]
pub async fn stash_apply(
    state: State<'_, AppState>,
    path: String,
    index: u32,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::stash_apply(&repo_path, index).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Drop (delete) a stash entry.
#[tauri::command]
pub async fn stash_drop(
    state: State<'_, AppState>,
    path: String,
    index: u32,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::stash_drop(&repo_path, index).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}
