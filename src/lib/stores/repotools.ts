/**
 * Submodules & worktrees of the active repo, plus shared actions
 * (open as tab, update/sync, add-worktree dialog).
 */
import { writable, get } from "svelte/store";
import type { SubmoduleInfo, WorktreeInfo } from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath, addRepo } from "./repos";
import { commitGraph, refreshAll } from "./graph";
import { toast, toastError } from "./toasts";

export const submodules = writable<SubmoduleInfo[]>([]);
export const worktrees = writable<WorktreeInfo[]>([]);

/** Whether the "Add worktree" dialog is open. */
export const addWorktreeOpen = writable(false);

let gen = 0;

export async function refreshRepoTools(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  const g = ++gen;
  const [subs, wts] = await Promise.allSettled([tauri.listSubmodules(p), tauri.listWorktrees(p)]);
  if (g !== gen || get(activeRepoPath) !== p) return;
  submodules.set(subs.status === "fulfilled" ? subs.value : []);
  worktrees.set(wts.status === "fulfilled" ? wts.value : []);
}

// Reset on tab switch; reload whenever the graph is (re)loaded, which
// happens on every refresh (focus, fetch, checkout, ...).
let lastPath: string | null = null;
activeRepoPath.subscribe((p) => {
  if (p === lastPath) return;
  lastPath = p;
  submodules.set([]);
  worktrees.set([]);
});
commitGraph.subscribe((g) => {
  if (g) refreshRepoTools();
});

/** Open a directory (submodule / worktree) as a new repo tab. */
export async function openAsTab(dir: string) {
  try {
    const info = await tauri.openRepo(dir);
    addRepo(info);
  } catch (err) {
    toastError("Could not open repository", err);
  }
}

async function runOp(title: string, op: () => Promise<{ success: boolean; message: string }>, okMsg: string) {
  const p = get(activeRepoPath);
  try {
    const res = await op();
    if (res.success) toast("success", okMsg);
    else toast("error", res.message.trim() || "Unknown error", { title });
  } catch (err) {
    toastError(title, err);
  } finally {
    if (p) {
      await refreshRepoTools(p);
      refreshAll(p);
    }
  }
}

let submoduleBusy = false;

export async function updateSubmodules(subPath?: string) {
  const p = get(activeRepoPath);
  if (!p || submoduleBusy) return;
  submoduleBusy = true;
  try {
    toast("info", subPath ? `Updating ${subPath}…` : "Updating submodules…", { duration: 2000 });
    await runOp("Submodule update failed", () => tauri.submoduleUpdate(p, subPath), subPath ? `Updated ${subPath}` : "Submodules updated");
  } finally {
    submoduleBusy = false;
  }
}

export async function syncSubmodules(subPath?: string) {
  const p = get(activeRepoPath);
  if (!p) return;
  await runOp("Submodule sync failed", () => tauri.submoduleSync(p, subPath), "Submodule URLs synced");
}
