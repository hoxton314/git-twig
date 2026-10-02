//! Tauri commands for in-progress operations and conflict resolution.
use tauri::State;

use crate::error::TwigError;
use crate::git::conflicts::{self, ConflictVersions, RepoOperationState};
use crate::state::AppState;

use super::staging::CommandResult;

/// Current operation (merge/rebase/cherry-pick/revert) and its conflicts.
#[tauri::command]
pub async fn get_operation_state(
    state: State<'_, AppState>,
    path: String,
) -> Result<RepoOperationState, TwigError> {
    state.read_repo(&path, conflicts::read_operation_state).await
}

/// Base / ours / theirs / working-tree contents of a conflicted file.
#[tauri::command]
pub async fn get_conflict_versions(
    state: State<'_, AppState>,
    path: String,
    file_path: String,
) -> Result<ConflictVersions, TwigError> {
    state
        .read_repo(&path, move |repo| conflicts::read_conflict_versions(repo, &file_path))
        .await
}

/// Continue the in-progress operation, optionally with an edited message.
#[tauri::command]
pub async fn continue_operation(
    state: State<'_, AppState>,
    path: String,
    message: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let (op, gitdir) = state
        .read_repo(&path, |repo| {
            Ok((conflicts::read_operation_state(repo)?, repo.path().to_path_buf()))
        })
        .await?;
    if !op.conflicts.is_empty() {
        return Ok(CommandResult {
            success: false,
            message: format!(
                "{} file(s) still have conflicts. Resolve them first.",
                op.conflicts.len()
            ),
        });
    }
    let out =
        conflicts::continue_operation(&repo_path, &gitdir, &op.kind, message.as_deref()).await?;
    Ok(merge_output(out))
}

/// Abort the in-progress operation, restoring the pre-operation state.
#[tauri::command]
pub async fn abort_operation(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let kind = state
        .read_repo(&path, |repo| Ok(conflicts::read_operation_state(repo)?.kind))
        .await?;
    let out = conflicts::abort_operation(&repo_path, &kind).await?;
    Ok(merge_output(out))
}

/// Skip the commit currently being applied (rebase / cherry-pick / revert).
#[tauri::command]
pub async fn skip_operation(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let kind = state
        .read_repo(&path, |repo| Ok(conflicts::read_operation_state(repo)?.kind))
        .await?;
    let out = conflicts::skip_operation(&repo_path, &kind).await?;
    Ok(merge_output(out))
}

/// Resolve conflicted files by taking "ours" or "theirs" wholesale.
#[tauri::command]
pub async fn resolve_take_side(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
    side: String,
) -> Result<CommandResult, TwigError> {
    if files.is_empty() {
        return Ok(CommandResult { success: true, message: String::new() });
    }
    let repo_path = state.repo_path(&path)?;
    let want_ours = side == "ours";
    let all = state.read_repo(&path, conflicts::read_conflicts).await?;
    let mut targets = Vec::with_capacity(files.len());
    for f in &files {
        let Some(c) = all.iter().find(|c| &c.path == f) else {
            return Ok(CommandResult {
                success: false,
                message: format!("'{f}' is not conflicted"),
            });
        };
        targets.push((f.clone(), if want_ours { c.has_ours } else { c.has_theirs }));
    }
    let out = conflicts::take_side(&repo_path, &targets, &side).await?;
    Ok(out.into())
}

/// Stage files to mark their conflicts as resolved.
#[tauri::command]
pub async fn mark_resolved(
    state: State<'_, AppState>,
    path: String,
    files: Vec<String>,
) -> Result<CommandResult, TwigError> {
    if files.is_empty() {
        return Ok(CommandResult { success: true, message: String::new() });
    }
    let repo_path = state.repo_path(&path)?;
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    let out = conflicts::mark_resolved(&repo_path, &refs).await?;
    Ok(out.into())
}

/// Save hand-resolved content to the working tree (optionally staging it).
#[tauri::command]
pub async fn save_resolved_file(
    state: State<'_, AppState>,
    path: String,
    file_path: String,
    content: String,
    stage: bool,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let rp = repo_path.clone();
    let fp = file_path.clone();
    tauri::async_runtime::spawn_blocking(move || conflicts::write_worktree_file(&rp, &fp, &content))
        .await
        .map_err(|e| TwigError::Task(e.to_string()))??;
    if stage {
        let out = conflicts::mark_resolved(&repo_path, &[file_path.as_str()]).await?;
        return Ok(out.into());
    }
    Ok(CommandResult { success: true, message: String::new() })
}

/// Open the configured external merge tool for a conflicted file and wait
/// for it to close.
#[tauri::command]
pub async fn open_merge_tool(
    state: State<'_, AppState>,
    path: String,
    file_path: String,
    tool: Option<String>,
) -> Result<CommandResult, TwigError> {
    let repo_path = state.repo_path(&path)?;
    let out = conflicts::open_merge_tool(&repo_path, &file_path, tool.as_deref()).await?;
    Ok(merge_output(out))
}

/// Many sequencer commands print their useful message on stdout even on
/// failure (e.g. "CONFLICT (content): ..."), so include both streams.
pub(crate) fn merge_output(out: crate::git::writer::GitOutput) -> CommandResult {
    if out.success {
        return CommandResult { success: true, message: out.stdout };
    }
    let mut msg = out.stderr.trim().to_string();
    let stdout = out.stdout.trim();
    if !stdout.is_empty() {
        if !msg.is_empty() {
            msg.push('\n');
        }
        msg.push_str(stdout);
    }
    CommandResult { success: false, message: msg }
}
