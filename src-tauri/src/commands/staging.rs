use serde::Serialize;
use tauri::State;

use crate::error::TwigError;
use crate::git::writer::GitOutput;
use crate::git::{reader, writer};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
}

impl From<GitOutput> for CommandResult {
    /// Surface stdout on success and stderr on failure.
    fn from(output: GitOutput) -> Self {
        CommandResult {
            success: output.success,
            message: if output.success {
                output.stdout
            } else {
                output.stderr
            },
        }
    }
}

/// Get the working directory status (staged + unstaged file lists).
#[tauri::command]
pub async fn get_working_status(
    state: State<'_, AppState>,
    path: String,
) -> Result<reader::WorkingStatus, TwigError> {
    state.read_repo(&path, reader::read_working_status).await
}

/// Get the staged diff, optionally for a single file.
#[tauri::command]
pub async fn get_staged_diff(
    state: State<'_, AppState>,
    path: String,
    file_path: Option<String>,
) -> Result<Vec<reader::DiffFile>, TwigError> {
    state
        .read_repo(&path, move |repo| {
            reader::read_staged_diff(repo, file_path.as_deref())
        })
        .await
}

/// Get the unstaged diff, optionally for a single file.
#[tauri::command]
pub async fn get_unstaged_diff(
    state: State<'_, AppState>,
    path: String,
    file_path: Option<String>,
) -> Result<Vec<reader::DiffFile>, TwigError> {
    state
        .read_repo(&path, move |repo| {
            reader::read_unstaged_diff(repo, file_path.as_deref())
        })
        .await
}

/// Stage files by path.
#[tauri::command]
pub async fn stage_files(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    if files.is_empty() {
        return Ok(CommandResult { success: true, message: String::new() });
    }
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    let output = writer::stage_files(&repo_path, &file_refs).await?;
    Ok(output.into())
}

/// Unstage files by path.
#[tauri::command]
pub async fn unstage_files(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    if files.is_empty() {
        return Ok(CommandResult { success: true, message: String::new() });
    }
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    let output = writer::unstage_files(&repo_path, &file_refs).await?;
    Ok(output.into())
}

/// Create a commit with the given message.
#[tauri::command]
pub async fn create_commit(
    state: State<'_, AppState>,
    path: String,
    message: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let output = writer::commit(&repo_path, &message).await?;
    Ok(output.into())
}

/// Undo the last commit, keeping changes staged.
#[tauri::command]
pub async fn undo_commit(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    // Reads open a fresh repository handle per call (see
    // `AppState::read_repo`), so no cached handle needs refreshing here.
    let output = writer::undo_last_commit(&repo_path).await?;
    Ok(output.into())
}

/// Discard unstaged changes. Tracked files are restored, untracked files are deleted.
#[tauri::command]
pub async fn discard_files(
    state: State<'_, AppState>,
    path: String,
    tracked: Vec<String>,
    untracked: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    if !tracked.is_empty() {
        let refs: Vec<&str> = tracked.iter().map(|s| s.as_str()).collect();
        let output = writer::restore_files(&repo_path, &refs).await?;
        if !output.success {
            return Ok(CommandResult {
                success: false,
                message: output.stderr,
            });
        }
    }

    if !untracked.is_empty() {
        let refs: Vec<&str> = untracked.iter().map(|s| s.as_str()).collect();
        let output = writer::clean_files(&repo_path, &refs).await?;
        if !output.success {
            return Ok(CommandResult {
                success: false,
                message: output.stderr,
            });
        }
    }

    Ok(CommandResult {
        success: true,
        message: String::new(),
    })
}

/// Pull from remote, auto-stashing uncommitted changes if present.
#[tauri::command]
pub async fn pull(
    state: State<'_, AppState>,
    path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;

    let dirty = writer::has_uncommitted_changes(&repo_path).await?;

    // Auto-stash if the working tree is dirty. Capture the created stash's
    // commit SHA so we can pop exactly that entry later — popping by positional
    // index ("stash@{0}") is unsafe because indices shift if another stash
    // appears (e.g. a concurrent operation or rebase autostash).
    let mut autostash_sha: Option<String> = None;
    if dirty {
        let before = writer::stash_top_sha(&repo_path).await?;
        let stash = writer::stash_push(&repo_path, Some("autostash before pull")).await?;
        if !stash.success {
            return Ok(CommandResult {
                success: false,
                message: format!("Failed to stash changes before pull: {}", stash.stderr),
            });
        }
        // `git stash push` exits 0 with "No local changes to save" when there
        // is nothing it can stash. Only treat the top entry as ours if it is
        // new — otherwise we would later pop the user's unrelated stash.
        let after = writer::stash_top_sha(&repo_path).await?;
        if after != before {
            autostash_sha = after;
        }
    }

    let remote_name = remote.as_deref().unwrap_or("origin");
    let output = writer::pull(&repo_path, remote_name, branch.as_deref()).await?;

    if !output.success {
        // Pull failed — restore stash if we created one
        if let Some(pop_msg) = restore_autostash(&repo_path, autostash_sha.as_deref()).await? {
            return Ok(CommandResult {
                success: false,
                message: format!(
                    "Pull failed: {}\nAlso failed to restore stashed changes: {}\nYour changes are still in the stash.",
                    output.stderr, pop_msg
                ),
            });
        }
        return Ok(CommandResult {
            success: false,
            message: output.stderr,
        });
    }

    // Pull succeeded — pop stash if we created one
    if let Some(pop_msg) = restore_autostash(&repo_path, autostash_sha.as_deref()).await? {
        return Ok(CommandResult {
            success: true,
            message: format!(
                "Pulled successfully, but conflicts when restoring local changes.\nYour changes are saved in the stash.\n{}",
                pop_msg
            ),
        });
    }
    if autostash_sha.is_some() {
        return Ok(CommandResult {
            success: true,
            message: format!("Pulled successfully (local changes auto-stashed and restored)\n{}", output.stdout),
        });
    }

    Ok(CommandResult {
        success: true,
        message: format!("Pulled successfully\n{}", output.stdout),
    })
}

/// Pop the auto-stash created before a pull, identified by its commit SHA so the
/// correct entry is popped even if stash indices have shifted. Returns
/// `Some(stderr)` if the pop failed, `None` on success (or if there was nothing
/// to restore).
async fn restore_autostash(
    repo_path: &std::path::Path,
    sha: Option<&str>,
) -> Result<Option<String>, TwigError> {
    let Some(sha) = sha else {
        return Ok(None);
    };
    match writer::stash_index_for_sha(repo_path, sha).await? {
        Some(index) => {
            let pop = writer::stash_pop(repo_path, index).await?;
            if pop.success {
                Ok(None)
            } else {
                Ok(Some(pop.stderr))
            }
        }
        // Stash entry not found — nothing to restore (already applied/dropped).
        None => Ok(None),
    }
}
