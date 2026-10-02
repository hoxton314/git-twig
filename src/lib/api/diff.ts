/** Diffs, staging, working-tree file actions and partial (hunk/line) staging. */
import { invoke } from "@tauri-apps/api/core";
import type {
  AuthorInfo,
  CommandResult,
  CommitResult,
  DiffArea,
  DiffFile,
  DiffReadOptions,
  HeadCommitInfo,
  HunkAction,
  HunkRange,
  IgnoreKind,
  IgnoreResult,
  SelectedLine,
  WorkingStatus,
} from "../types/git";

// ── Diffs ─────────────────────────────────────────────────────────────

export function getCommitDiff(
  path: string,
  oid: string,
  options?: DiffReadOptions,
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_commit_diff", {
    path,
    oid,
    options: options ?? diffReadDefaults,
  });
}

/** Diff between two commits (`from` → `to`). */
export function getCompareDiff(
  path: string,
  from: string,
  to: string,
  options?: DiffReadOptions,
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_compare_diff", {
    path,
    from,
    to,
    options: options ?? diffReadDefaults,
  });
}

export function getWorkingDiff(
  path: string,
  options?: DiffReadOptions,
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_working_diff", {
    path,
    options: options ?? diffReadDefaults,
  });
}

export function getFileBlob(
  path: string,
  filePath: string,
  source: string,
): Promise<string | null> {
  return invoke<string | null>("get_file_blob", { path, filePath, source });
}

// ── Staging & working directory ───────────────────────────────────────

export function getWorkingStatus(path: string): Promise<WorkingStatus> {
  return invoke<WorkingStatus>("get_working_status", { path });
}

export function getStagedDiff(
  path: string,
  filePath?: string,
  options?: DiffReadOptions,
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_staged_diff", {
    path,
    filePath: filePath ?? null,
    options: options ?? diffReadDefaults,
  });
}

export function getUnstagedDiff(
  path: string,
  filePath?: string,
  options?: DiffReadOptions,
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_unstaged_diff", {
    path,
    filePath: filePath ?? null,
    options: options ?? diffReadDefaults,
  });
}

export function stageFiles(
  path: string,
  files: string[]
): Promise<CommandResult> {
  return invoke<CommandResult>("stage_files", { path, files });
}

export function unstageFiles(
  path: string,
  files: string[]
): Promise<CommandResult> {
  return invoke<CommandResult>("unstage_files", { path, files });
}

export function discardFiles(
  path: string,
  tracked: string[],
  untracked: string[]
): Promise<CommandResult> {
  return invoke<CommandResult>("discard_files", { path, tracked, untracked });
}

export function undoCommit(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("undo_commit", { path });
}

export function createCommit(
  path: string,
  message: string
): Promise<CommandResult> {
  return invoke<CommandResult>("create_commit", { path, message });
}

export function pull(
  path: string,
  remote?: string,
  branch?: string
): Promise<CommandResult> {
  return invoke<CommandResult>("pull", {
    path,
    remote: remote ?? null,
    branch: branch ?? null,
  });
}

// ── Staging panel: commit helpers & file actions ─────────────────────

export function createCommitWithOptions(
  path: string,
  message: string,
  amend: boolean,
  signoff: boolean,
  noVerify = false,
): Promise<CommitResult> {
  return invoke<CommitResult>("create_commit_with_options", { path, message, amend, signoff, noVerify });
}

export function getHeadCommitInfo(path: string): Promise<HeadCommitInfo> {
  return invoke<HeadCommitInfo>("get_head_commit_info", { path });
}

export function getRecentAuthors(path: string, maxCommits?: number): Promise<AuthorInfo[]> {
  return invoke<AuthorInfo[]>("get_recent_authors", { path, maxCommits: maxCommits ?? null });
}

export function getCommitTemplate(path: string): Promise<string | null> {
  return invoke<string | null>("get_commit_template", { path });
}

export function openRepoFile(path: string, file: string): Promise<void> {
  return invoke<void>("open_repo_file", { path, file });
}

export function revealRepoFile(path: string, file: string): Promise<void> {
  return invoke<void>("reveal_repo_file", { path, file });
}

export function addToGitignore(path: string, file: string, kind: IgnoreKind): Promise<IgnoreResult> {
  return invoke<IgnoreResult>("add_to_gitignore", { path, file, kind });
}

export function openExternalDiff(
  path: string,
  file: string,
  staged: boolean,
  tool: string | null,
): Promise<void> {
  return invoke<void>("open_external_diff", { path, file, staged, tool });
}

// ── Diff viewer: read options & partial staging ──────────────────────


/**
 * Display options applied to every diff read that doesn't pass its own.
 * Kept in sync with the "Editor & Diff" settings by the settings store, so
 * all callers (staging refreshes, commit diffs, WIP view) honour them.
 */
let diffReadDefaults: DiffReadOptions = {};

export function setDiffReadDefaults(options: DiffReadOptions) {
  diffReadDefaults = { ...options };
}

/**
 * Stage / unstage / discard selected lines of one file. `lines` are verified
 * against the current diff; every change inside `ranges` is included too.
 */
export function applyDiffSelection(
  path: string,
  filePath: string,
  area: DiffArea,
  action: HunkAction,
  lines: SelectedLine[],
  ranges: HunkRange[],
): Promise<CommandResult> {
  return invoke<CommandResult>("apply_diff_selection", {
    path,
    filePath,
    area,
    action,
    lines,
    ranges,
  });
}
