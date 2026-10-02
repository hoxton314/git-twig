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
import { settings } from "./settings";

/** Commit limit for graph loads, from user settings. */
function maxCommits(): number {
  const n = get(settings).max_commits;
  return n > 0 ? n : 5000;
}

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

// Reset per-repo state whenever the active repo changes so a selection (or
// graph/branches) from repo A is never shown or queried against repo B.
let lastActivePath: string | null = null;
activeRepoPath.subscribe((p) => {
  if (p === lastActivePath) return;
  lastActivePath = p;
  commitGraph.set(null);
  // CommitGraph's mount/repo-change effect kicks off loadGraph for the new
  // path; mark loading now so it doesn't flash an error/empty state first.
  graphLoading.set(p !== null);
  branches.set([]);
  selectedCommitOid.set(null);
  selectedDiff.set([]);
  selectedWorkingFile.set(null);
  workingFileDiff.set([]);
  workingStatus.set({ staged: [], unstaged: [] });
  stashEntries.set([]);
});

/**
 * True if `p` is still the active repo. Used to discard results from a refresh
 * that the user navigated away from before it resolved, so a slow response for
 * repo A can't clobber the freshly-loaded state of repo B.
 */
function stillActive(p: string): boolean {
  return get(activeRepoPath) === p;
}

/** Refresh stash list for the active repo. */
export async function refreshStash(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  try {
    const entries = await tauri.stashList(p);
    if (stillActive(p)) stashEntries.set(entries);
  } catch {
    if (stillActive(p)) stashEntries.set([]);
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

  // "__all__" is the combined WIP diff (uncommitted-changes row), not a real file.
  if (sel.path === "__all__") {
    if (status.staged.length === 0 && status.unstaged.length === 0) {
      selectedWorkingFile.set(null);
      workingFileDiff.set([]);
      if (get(selectedCommitOid) === "__wip__") selectedCommitOid.set(null);
      return;
    }
    try {
      const diff = await tauri.getWorkingDiff(repoPath);
      if (stillActive(repoPath)) workingFileDiff.set(diff);
    } catch {
      if (stillActive(repoPath)) workingFileDiff.set([]);
    }
    return;
  }

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
    if (stillActive(repoPath)) workingFileDiff.set(diff);
  } catch {
    if (stillActive(repoPath)) workingFileDiff.set([]);
  }
}

/** Refresh working status for the active repo. */
export async function refreshStatus(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  try {
    const result = await tauri.getWorkingStatus(p);
    if (!stillActive(p)) return;
    workingStatus.set(result);
    await resyncSelectedFile(p, result);
  } catch (err) {
    console.error("Failed to load working status:", err);
  }
}

/** Load only the commit graph for `path` (used when switching repos). */
export async function loadGraph(path: string) {
  const gen = ++refreshGen;
  graphLoading.set(true);
  try {
    const graph = await tauri.getCommitGraph(path, maxCommits());
    if (gen === refreshGen && stillActive(path)) commitGraph.set(graph);
  } catch (err) {
    console.error("Failed to load commit graph:", err);
    if (gen === refreshGen && stillActive(path)) commitGraph.set(null);
  } finally {
    if (gen === refreshGen) graphLoading.set(false);
  }
}

/** Monotonic counter so an older in-flight refresh never overwrites a newer one. */
let refreshGen = 0;

/** Refresh commit graph, branches, repo info, and working status. */
export async function refreshAll(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  const gen = ++refreshGen;
  graphLoading.set(true);
  try {
    const [graph, branchList, info, status, stash] = await Promise.all([
      tauri.getCommitGraph(p, maxCommits()),
      tauri.getBranches(p),
      tauri.getRepoInfo(p),
      tauri.getWorkingStatus(p),
      tauri.stashList(p),
    ]);
    // Discard if the user switched repos (or a newer refresh started)
    // while this was in flight.
    if (gen !== refreshGen || !stillActive(p)) return;
    commitGraph.set(graph);
    branches.set(branchList);
    updateRepo(info);
    workingStatus.set(status);
    stashEntries.set(stash);
    await resyncSelectedFile(p, status);
  } catch (err) {
    console.error("Failed to refresh:", err);
  } finally {
    if (gen === refreshGen) graphLoading.set(false);
  }
}
