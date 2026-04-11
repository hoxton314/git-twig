import { writable, get } from "svelte/store";
import type {
  CommitGraph,
  BranchInfo,
  DiffFile,
  WorkingStatus,
  StashEntry,
} from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath, updateRepo } from "./repos";

/** Commit graph for the active repo. */
export const commitGraph = writable<CommitGraph | null>(null);

/** Whether the graph is currently loading. */
export const graphLoading = writable(false);

/** Branches for the active repo. */
export const branches = writable<BranchInfo[]>([]);

/** The currently selected commit OID (for showing its diff). */
export const selectedCommitOid = writable<string | null>(null);

/** Diff files for the selected commit. */
export const selectedDiff = writable<DiffFile[]>([]);

/** Whether the diff is currently loading. */
export const diffLoading = writable(false);

// ── Working directory ─────────────────────────────────────────────────

/** Working directory status (staged/unstaged file lists). */
export const workingStatus = writable<WorkingStatus>({
  staged: [],
  unstaged: [],
});

/** Currently selected working directory file for diff preview. */
export const selectedWorkingFile = writable<{
  path: string;
  area: "staged" | "unstaged";
} | null>(null);

/** Diff for the selected working directory file. */
export const workingFileDiff = writable<DiffFile[]>([]);

// ── Stash ────────────────────────────────────────────────────────────

/** Stash entries for the active repo. */
export const stashEntries = writable<StashEntry[]>([]);

/** Refresh stash list for the active repo. */
export async function refreshStash(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  try {
    const entries = await tauri.stashList(p);
    stashEntries.set(entries);
  } catch {
    stashEntries.set([]);
  }
}

// ── Refresh helpers ──────────────────────────────────────────────────

/**
 * After any working status change, resync the selected file's area label
 * and diff content so the diff viewer never shows stale data.
 */
async function resyncSelectedFile(repoPath: string, status: WorkingStatus) {
  const sel = get(selectedWorkingFile);
  if (!sel) return;

  const inStaged = status.staged.some((f) => f.path === sel.path);
  const inUnstaged = status.unstaged.some((f) => f.path === sel.path);

  if (!inStaged && !inUnstaged) {
    // File no longer has any changes
    selectedWorkingFile.set(null);
    workingFileDiff.set([]);
    return;
  }

  // If file moved out of its current area, flip to where it now lives
  let newArea = sel.area;
  if (newArea === "staged" && !inStaged) newArea = "unstaged";
  if (newArea === "unstaged" && !inUnstaged) newArea = "staged";

  if (newArea !== sel.area) {
    selectedWorkingFile.set({ path: sel.path, area: newArea });
  }

  // Always reload the diff to reflect current content
  try {
    const diff =
      newArea === "staged"
        ? await tauri.getStagedDiff(repoPath, sel.path)
        : await tauri.getUnstagedDiff(repoPath, sel.path);
    workingFileDiff.set(diff);
  } catch {
    workingFileDiff.set([]);
  }
}

/** Refresh working status for the active repo. */
export async function refreshStatus(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  try {
    const result = await tauri.getWorkingStatus(p);
    workingStatus.set(result);
    await resyncSelectedFile(p, result);
  } catch (err) {
    console.error("Failed to load working status:", err);
  }
}

/** Refresh commit graph, branches, repo info, and working status. */
export async function refreshAll(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  graphLoading.set(true);
  try {
    const [graph, branchList, info, status, stash] = await Promise.all([
      tauri.getCommitGraph(p, 5000),
      tauri.getBranches(p),
      tauri.getRepoInfo(p),
      tauri.getWorkingStatus(p),
      tauri.stashList(p),
    ]);
    commitGraph.set(graph);
    branches.set(branchList);
    updateRepo(info);
    workingStatus.set(status);
    stashEntries.set(stash);
    await resyncSelectedFile(p, status);
  } catch (err) {
    console.error("Failed to refresh:", err);
  } finally {
    graphLoading.set(false);
  }
}
