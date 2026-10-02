/** Commit operations, reflog, tags, conflicts, rebase, file history and blame. */
import { invoke } from "@tauri-apps/api/core";
import type {
  BlameResult,
  CommandResult,
  ApplyPatchResult,
  BisectInfo,
  CommitOpResult,
  PatchInfo,
  PickManyResult,
  ConflictVersions,
  DiffFile,
  FileHistoryPage,
  RebaseCommitList,
  RebaseTodoItem,
  ReflogEntry,
  RepoOperationState,
  ResetMode,
  SquashPlan,
  TagInfo,
} from "../types/git";

// ── Commit operations & undo history (reflog) ─────────────────────────

export function checkoutCommit(path: string, oid: string): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("checkout_commit", { path, oid });
}

export function cherryPickCommit(path: string, oid: string): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("cherry_pick_commit", { path, oid });
}

/** Cherry-pick `oids` onto HEAD in the given order (pass them oldest first). */
export function cherryPickCommits(path: string, oids: string[]): Promise<PickManyResult> {
  return invoke<PickManyResult>("cherry_pick_commits", { path, oids });
}

export function revertCommit(path: string, oid: string): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("revert_commit", { path, oid });
}

export function resetToCommit(path: string, oid: string, mode: ResetMode): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("reset_to_commit", { path, oid, mode });
}

export function getHeadReflog(path: string, limit?: number): Promise<ReflogEntry[]> {
  return invoke<ReflogEntry[]>("get_head_reflog", { path, limit: limit ?? null });
}

/** Move HEAD back to `oid` (checking out `branch` if it points there). */
export function restoreHead(
  path: string,
  oid: string,
  branch: string | null,
  autoStash: boolean,
): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("restore_head", { path, oid, branch, autoStash });
}

// ── Tags ──────────────────────────────────────────────────────────────

export function getTags(path: string): Promise<TagInfo[]> {
  return invoke<TagInfo[]>("get_tags", { path });
}

/** A non-empty `message` creates an annotated tag. */
export function createTag(
  path: string,
  name: string,
  target: string,
  message?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("create_tag", { path, name, target, message: message ?? null });
}

export function deleteTag(path: string, name: string): Promise<CommandResult> {
  return invoke<CommandResult>("delete_tag", { path, name });
}

/** Push one tag, or all tags when `name` is omitted. */
export function pushTag(path: string, name?: string, remote?: string): Promise<CommandResult> {
  return invoke<CommandResult>("push_tag", { path, name: name ?? null, remote: remote ?? null });
}

export function deleteRemoteTag(path: string, name: string, remote?: string): Promise<CommandResult> {
  return invoke<CommandResult>("delete_remote_tag", { path, name, remote: remote ?? null });
}

// ── Conflict resolution & history rewriting (rebase, force push) ────


export function getOperationState(path: string): Promise<RepoOperationState> {
  return invoke<RepoOperationState>("get_operation_state", { path });
}

export function getConflictVersions(path: string, filePath: string): Promise<ConflictVersions> {
  return invoke<ConflictVersions>("get_conflict_versions", { path, filePath });
}

/** Continue the in-progress merge/rebase/cherry-pick/revert. */
export function continueOperation(path: string, message?: string | null): Promise<CommandResult> {
  return invoke<CommandResult>("continue_operation", { path, message: message ?? null });
}

export function abortOperation(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("abort_operation", { path });
}

export function skipOperation(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("skip_operation", { path });
}

export function resolveTakeSide(
  path: string,
  files: string[],
  side: "ours" | "theirs",
): Promise<CommandResult> {
  return invoke<CommandResult>("resolve_take_side", { path, files, side });
}

export function markResolved(path: string, files: string[]): Promise<CommandResult> {
  return invoke<CommandResult>("mark_resolved", { path, files });
}

export function saveResolvedFile(
  path: string,
  filePath: string,
  content: string,
  stage: boolean,
): Promise<CommandResult> {
  return invoke<CommandResult>("save_resolved_file", { path, filePath, content, stage });
}

/** Opens the merge tool and resolves once it has been closed. */
export function openMergeTool(
  path: string,
  filePath: string,
  tool: string | null,
): Promise<CommandResult> {
  return invoke<CommandResult>("open_merge_tool", { path, filePath, tool });
}

/** Rebase the current branch onto a branch name or commit. */
export function rebaseOnto(path: string, upstream: string, autostash: boolean): Promise<CommandResult> {
  return invoke<CommandResult>("rebase_onto", { path, upstream, autostash });
}

/** Commits base..HEAD (oldest first); `base = null` means from the root. */
export function listRebaseCommits(path: string, base: string | null): Promise<RebaseCommitList> {
  return invoke<RebaseCommitList>("list_rebase_commits", { path, base });
}

/** Validate squashing `oids` (any order) and get the default message. */
export function planSquash(path: string, oids: string[]): Promise<SquashPlan> {
  return invoke<SquashPlan>("plan_squash", { path, oids });
}

/** Squash the contiguous run `oids` on the current branch into one commit. */
export function squashCommits(
  path: string,
  oids: string[],
  message: string,
  autostash: boolean,
): Promise<CommandResult> {
  return invoke<CommandResult>("squash_commits", { path, oids, message, autostash });
}

export function interactiveRebase(
  path: string,
  base: string | null,
  items: RebaseTodoItem[],
  autostash: boolean,
): Promise<CommandResult> {
  return invoke<CommandResult>("interactive_rebase", { path, base, items, autostash });
}

/** `git push --force-with-lease`; remote defaults to the upstream remote, then origin. */
export function forcePushWithLease(
  path: string,
  branchName: string,
  remote?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("force_push_with_lease", { path, branchName, remote: remote ?? null });
}

// ── File history & blame ─────────────────────────────────────────────

export function getFileHistory(
  path: string,
  filePath: string,
  skip: number,
  limit: number,
  rev?: string,
): Promise<FileHistoryPage> {
  return invoke<FileHistoryPage>("get_file_history", {
    path,
    filePath,
    rev: rev ?? null,
    skip,
    limit,
  });
}

export function getFileDiffAtCommit(
  path: string,
  oid: string,
  filePath: string,
  oldPath?: string | null,
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_file_diff_at_commit", {
    path,
    oid,
    filePath,
    oldPath: oldPath ?? null,
  });
}

export function getBlame(
  path: string,
  filePath: string,
  rev?: string,
): Promise<BlameResult> {
  return invoke<BlameResult>("get_blame", { path, filePath, rev: rev ?? null });
}

export function listTrackedFiles(path: string): Promise<string[]> {
  return invoke<string[]>("list_tracked_files", { path });
}

// ── Patch files ──────────────────────────────────────────────────────

/**
 * Save commits (oldest first) as patches: one file each in the folder
 * `target`, or all in the mbox file `target` when `singleFile`.
 */
export function formatPatches(
  path: string,
  oids: string[],
  target: string,
  singleFile: boolean,
): Promise<string[]> {
  return invoke<string[]>("format_patches", { path, oids, target, singleFile });
}

/** Save uncommitted changes to tracked files (vs HEAD) as a patch file. */
export function saveWorkingPatch(path: string, target: string): Promise<void> {
  return invoke<void>("save_working_patch", { path, target });
}

export function inspectPatch(path: string, file: string): Promise<PatchInfo> {
  return invoke<PatchInfo>("inspect_patch", { path, file });
}

export function applyPatch(path: string, file: string): Promise<ApplyPatchResult> {
  return invoke<ApplyPatchResult>("apply_patch", { path, file });
}

// ── Bisect ────────────────────────────────────────────────────────────

export function getBisectState(path: string): Promise<BisectInfo | null> {
  return invoke<BisectInfo | null>("get_bisect_state", { path });
}

/** Start bisecting; either end may be given later with `bisectMark`. */
export function bisectStart(path: string, bad: string | null, good: string | null): Promise<CommandResult> {
  return invoke<CommandResult>("bisect_start", { path, bad, good });
}

/** Mark `rev` (default HEAD) as good, bad or skip. */
export function bisectMark(path: string, verdict: "good" | "bad" | "skip", rev?: string): Promise<CommandResult> {
  return invoke<CommandResult>("bisect_mark", { path, verdict, rev: rev ?? null });
}

export function bisectReset(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("bisect_reset", { path });
}
