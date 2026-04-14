use serde::Serialize;
use tauri::State;

use git2::Repository;

use crate::error::TwigError;
use crate::git::{reader, writer};
use crate::state::{AppState, OpenRepo};

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
}

/// Get the working directory status (staged + unstaged file lists).
#[tauri::command]
pub async fn get_working_status(
    state: State<'_, AppState>,
    path: String,
) -> Result<reader::WorkingStatus, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_working_status(&open.repository)
}

/// Get the staged diff, optionally for a single file.
#[tauri::command]
pub async fn get_staged_diff(
    state: State<'_, AppState>,
    path: String,
    file_path: Option<String>,
) -> Result<Vec<reader::DiffFile>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_staged_diff(&open.repository, file_path.as_deref())
}

/// Get the unstaged diff, optionally for a single file.
#[tauri::command]
pub async fn get_unstaged_diff(
    state: State<'_, AppState>,
    path: String,
    file_path: Option<String>,
) -> Result<Vec<reader::DiffFile>, TwigError> {
    let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
    let open = repos
        .get(&path)
        .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;

    reader::read_unstaged_diff(&open.repository, file_path.as_deref())
}

/// Stage files by path.
#[tauri::command]
pub async fn stage_files(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    let output = writer::stage_files(&repo_path, &file_refs).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Unstage files by path.
#[tauri::command]
pub async fn unstage_files(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    let output = writer::unstage_files(&repo_path, &file_refs).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Create a commit with the given message.
#[tauri::command]
pub async fn create_commit(
    state: State<'_, AppState>,
    path: String,
    message: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::commit(&repo_path, &message).await?;
    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Undo the last commit, keeping changes staged.
#[tauri::command]
pub async fn undo_commit(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let output = writer::undo_last_commit(&repo_path).await?;

    if output.success {
        // Re-open the repository so git2 sees the new HEAD
        let fresh = Repository::discover(&repo_path)
            .map_err(|_| TwigError::NotARepo(path.clone()))?;
        let mut repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        if let Some(open) = repos.get_mut(&path) {
            *open = OpenRepo {
                repository: fresh,
                path: repo_path,
            };
        }
    }

    Ok(CommandResult {
        success: output.success,
        message: if output.success {
            output.stdout
        } else {
            output.stderr
        },
    })
}

/// Discard unstaged changes. Tracked files are restored, untracked files are deleted.
#[tauri::command]
pub async fn discard_files(
    state: State<'_, AppState>,
    path: String,
    tracked: Vec<String>,
    untracked: Vec<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

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
    let repo_path = {
        let repos = state.repos.lock().map_err(|_| TwigError::Lock)?;
        let open = repos
            .get(&path)
            .ok_or_else(|| TwigError::RepoNotFound(path.clone()))?;
        open.path.clone()
    };

    let dirty = writer::has_uncommitted_changes(&repo_path).await?;

    // Auto-stash if the working tree is dirty
    if dirty {
        let stash = writer::stash_push(&repo_path, Some("autostash before pull")).await?;
        if !stash.success {
            return Ok(CommandResult {
                success: false,
                message: format!("Failed to stash changes before pull: {}", stash.stderr),
            });
        }
    }

    let remote_name = remote.as_deref().unwrap_or("origin");
    let output = writer::pull(&repo_path, remote_name, branch.as_deref()).await?;

    if !output.success {
        // Pull failed — restore stash if we created one
        if dirty {
            let pop = writer::stash_pop(&repo_path, 0).await?;
            if !pop.success {
                return Ok(CommandResult {
                    success: false,
                    message: format!(
                        "Pull failed: {}\nAlso failed to restore stashed changes: {}\nYour changes are still in the stash.",
                        output.stderr, pop.stderr
                    ),
                });
            }
        }
        return Ok(CommandResult {
            success: false,
            message: output.stderr,
        });
    }

    // Pull succeeded — pop stash if we created one
    if dirty {
        let pop = writer::stash_pop(&repo_path, 0).await?;
        if !pop.success {
            return Ok(CommandResult {
                success: true,
                message: format!(
                    "Pulled successfully, but conflicts when restoring local changes.\nYour changes are saved in the stash.\n{}",
                    pop.stderr
                ),
            });
        }
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
