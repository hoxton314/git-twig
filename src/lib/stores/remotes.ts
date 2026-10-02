import { writable, get } from "svelte/store";
import type { RemoteInfo } from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath } from "./repos";
import { branches } from "./graph";
import { sidebarOpen } from "./ui";

/** Remotes of the active repo (name + URLs). */
export const remotes = writable<RemoteInfo[]>([]);

/** Whether the "Manage remotes" dialog is open. */
export const remotesDialogOpen = writable(false);

export type BranchListRequestKind = "create" | "rename_current" | "filter";

/**
 * One-shot requests from global actions (command palette / shortcuts) to the
 * branch list, which owns the related inputs and dialogs. `seq` makes repeated
 * identical requests distinguishable.
 */
export const branchListRequest = writable<{ kind: BranchListRequestKind; seq: number } | null>(null);

let seq = 0;
export function requestBranchList(kind: BranchListRequestKind) {
  // The branch list only exists while the sidebar is open.
  sidebarOpen.set(true);
  branchListRequest.set({ kind, seq: ++seq });
}

export async function loadRemotes(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  try {
    const list = await tauri.listRemotes(p);
    if (get(activeRepoPath) === p) remotes.set(list);
  } catch (err) {
    console.error("Failed to load remotes:", err);
    if (get(activeRepoPath) === p) remotes.set([]);
  }
}

// Remotes can change behind our back (CLI, clone, another tool); refreshing
// them alongside every branch reload keeps the grouping in sync cheaply.
let lastPath: string | null = null;
activeRepoPath.subscribe((p) => {
  if (p === lastPath) return;
  lastPath = p;
  remotes.set([]);
  remotesDialogOpen.set(false);
  if (p) loadRemotes(p);
});
branches.subscribe((list) => {
  if (list.length > 0) loadRemotes();
});
