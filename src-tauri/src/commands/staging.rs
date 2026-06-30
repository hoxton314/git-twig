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

    if files.is_empty() {
        return Ok(CommandResult { success: true, message: String::new() });
    }
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

    if files.is_empty() {
        return Ok(CommandResult { success: true, message: String::new() });
    }
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
        // Replace the cached handle so git2 sees the new HEAD. Insert rather than
        // only-update-if-present so a concurrently-removed entry doesn't leave a
        // stale repo behind (which would make later reads report the old HEAD).
        repos.insert(
            path.clone(),
            OpenRepo {
                repository: fresh,
                path: repo_path,
            },
        );
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

    // Auto-stash if the working tree is dirty. Capture the created stash's
    // commit SHA so we can pop exactly that entry later — popping by positional
    // index ("stash@{0}") is unsafe because indices shift if another stash
    // appears (e.g. a concurrent operation or rebase autostash).
    let mut autostash_sha: Option<String> = None;
    if dirty {
        let stash = writer::stash_push(&repo_path, Some("autostash before pull")).await?;
        if !stash.success {
            return Ok(CommandResult {
                success: false,
                message: format!("Failed to stash changes before pull: {}", stash.stderr),
            });
        }
        autostash_sha = writer::stash_top_sha(&repo_path).await?;
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
    if dirty {
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
