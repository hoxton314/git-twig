/** Submodules and worktrees. */
import { invoke } from "@tauri-apps/api/core";
import type {
  CommandResult,
  SubmoduleInfo,
  WorktreeInfo,
} from "../types/git";

// ── Submodules ───────────────────────────────────────────────────────

export function listSubmodules(path: string): Promise<SubmoduleInfo[]> {
  return invoke<SubmoduleInfo[]>("list_submodules", { path });
}

export function submoduleUpdate(
  path: string,
  subPath?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("submodule_update", { path, subPath: subPath ?? null });
}

export function submoduleSync(
  path: string,
  subPath?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("submodule_sync", { path, subPath: subPath ?? null });
}

// ── Worktrees ────────────────────────────────────────────────────────

export function listWorktrees(path: string): Promise<WorktreeInfo[]> {
  return invoke<WorktreeInfo[]>("list_worktrees", { path });
}

export function worktreeAdd(
  path: string,
  worktreePath: string,
  opts: { commitish?: string; newBranch?: string },
): Promise<CommandResult> {
  return invoke<CommandResult>("worktree_add", {
    path,
    worktreePath,
    commitish: opts.commitish ?? null,
    newBranch: opts.newBranch ?? null,
  });
}

export function worktreeRemove(
  path: string,
  worktreePath: string,
  force: boolean = false,
): Promise<CommandResult> {
  return invoke<CommandResult>("worktree_remove", { path, worktreePath, force });
}

export function worktreePrune(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("worktree_prune", { path });
}
