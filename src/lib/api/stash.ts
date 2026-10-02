/** Stash list and SHA-addressed stash operations. */
import { invoke } from "@tauri-apps/api/core";
import type {
  CommandResult,
  StashDetail,
  StashDiff,
  StashEntry,
} from "../types/git";

// ── Stash ────────────────────────────────────────────────────

export function stashList(path: string): Promise<StashEntry[]> {
  return invoke<StashEntry[]>("stash_list", { path });
}

export function stashPush(
  path: string,
  message?: string
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_push", {
    path,
    message: message ?? null,
  });
}

// ── Stash extras ─────────────────────────────────────────────────────

export function stashListDetailed(path: string): Promise<StashDetail[]> {
  return invoke<StashDetail[]>("stash_list_detailed", { path });
}

export function stashShow(path: string, oid: string): Promise<StashDiff> {
  return invoke<StashDiff>("stash_show", { path, oid });
}

/** Apply/pop/drop the stash with commit `oid` (index is resolved server-side). */
export function stashAct(
  path: string,
  oid: string,
  action: "apply" | "pop" | "drop",
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_act", { path, oid, action });
}

export function stashRename(
  path: string,
  oid: string,
  message: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_rename", { path, oid, message });
}

export function stashBranch(
  path: string,
  oid: string,
  branch: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_branch", { path, oid, branch });
}

export function stashPushExt(
  path: string,
  opts: {
    message?: string;
    files?: string[];
    keepIndex?: boolean;
    includeUntracked?: boolean;
  },
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_push_ext", {
    path,
    message: opts.message ?? null,
    files: opts.files ?? [],
    keepIndex: opts.keepIndex ?? false,
    includeUntracked: opts.includeUntracked ?? true,
  });
}
