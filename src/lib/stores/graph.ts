import { writable, derived, get } from "svelte/store";
import type {
  CommitGraph,
  BranchInfo,
  DiffFile,
  WorkingStatus,
  StashEntry,
  GraphOptions,
} from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath, updateRepo } from "./repos";
import { settings } from "./settings";
import { toastError } from "./toasts";
import { tr } from "../i18n";
import { EMPTY_SELECTION, effectiveSelection, pruneSelection, type Selection } from "../graphSelection";

/** Graph page size ("commits per page" setting). */
export function graphPageSize(): number {
  const n = get(settings).max_commits;
  return n > 0 ? n : 5000;
}

/** Which refs the graph walks, from the graph view settings. */
export function graphOptions(): GraphOptions {
  const s = get(settings);
  return {
    hide_remotes: s.graph_hide_remotes,
    current_branch_only: s.graph_current_branch_only,
  };
}

/**
 * How many rows a reload should fetch: at least one page, and everything
 * the user has already paged in so a refresh doesn't drop their position.
 */
function reloadCount(minCount = 0): number {
  return Math.max(graphPageSize(), get(commitGraph)?.entries.length ?? 0, minCount);
}

/** Commit graph for the active repo (all pages loaded so far). */
export const commitGraph = writable<CommitGraph | null>(null);

/** Whether the graph is currently loading. */
export const graphLoading = writable(false);

/** Whether a further page of history is being fetched. */
export const graphLoadingMore = writable(false);

/** Branches for the active repo. */
export const branches = writable<BranchInfo[]>([]);

/** The currently selected commit OID (for showing its diff). */
export const selectedCommitOid = writable<string | null>(null);

/** Extra graph selection (Ctrl/Shift-click); see `graphSelection.ts`. */
export const commitSelection = writable<Selection>(EMPTY_SELECTION);

/** The selected commits in graph order (one, or several when multi-selecting). */
export const selectedCommits = derived([commitSelection, selectedCommitOid], ([sel, primary]) =>
  effectiveSelection(sel, primary),
);

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
  commitSelection.set(EMPTY_SELECTION);
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

/** Install a reloaded graph, dropping selected commits it no longer has. */
function setReloadedGraph(graph: CommitGraph) {
  const old = get(commitGraph);
  if (old) {
    const oids = (g: CommitGraph) => g.entries.map((e) => e.commit.oid);
    commitSelection.update((sel) => pruneSelection(sel, oids(old), oids(graph)));
  }
  commitGraph.set(graph);
}

/**
 * Load only the commit graph for `path` (used when switching repos or view
 * options). Fetches at least `minCount` rows and keeps rows already paged in.
 */
export async function loadGraph(path: string, minCount = 0) {
  const gen = ++refreshGen;
  graphLoading.set(true);
  try {
    const graph = await tauri.getCommitGraph(path, reloadCount(minCount), 0, graphOptions());
    if (gen === refreshGen && stillActive(path)) setReloadedGraph(graph);
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
      tauri.getCommitGraph(p, reloadCount(), 0, graphOptions()),
      tauri.getBranches(p),
      tauri.getRepoInfo(p),
      tauri.getWorkingStatus(p),
      tauri.stashList(p),
    ]);
    // Discard if the user switched repos (or a newer refresh started)
    // while this was in flight.
    if (gen !== refreshGen || !stillActive(p)) return;
    setReloadedGraph(graph);
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

// ── Pagination ───────────────────────────────────────────────────────

let moreInFlight: Promise<void> | null = null;

/**
 * Append the next `count` rows of history (default: one page). Lanes are
 * computed in Rust over the whole prefix, so new rows continue the existing
 * ones. If the branch tips moved meanwhile, the graph is reloaded instead so
 * pages never mix two different histories.
 */
export function loadMoreCommits(count?: number): Promise<void> {
  if (moreInFlight) return moreInFlight;
  const p = get(activeRepoPath);
  const g = get(commitGraph);
  if (!p || !g || !g.has_more) return Promise.resolve();
  const gen = refreshGen;
  const want = Math.max(1, Math.min(count ?? graphPageSize(), Number.MAX_SAFE_INTEGER));
  graphLoadingMore.set(true);
  moreInFlight = (async () => {
    try {
      const page = await tauri.getCommitGraph(p, want, g.entries.length, graphOptions());
      if (gen !== refreshGen || !stillActive(p) || get(commitGraph) !== g) return;
      if (page.tips !== g.tips) {
        await loadGraph(p, g.entries.length + want);
        return;
      }
      commitGraph.set({
        ...page,
        entries: g.entries.concat(page.entries),
        offset: 0,
        total_lanes: Math.max(g.total_lanes, page.total_lanes),
      });
    } catch (err) {
      if (stillActive(p)) toastError(tr("graph.loadMoreFailed"), err);
    } finally {
      graphLoadingMore.set(false);
      moreInFlight = null;
    }
  })();
  return moreInFlight;
}

/**
 * Make sure at least `count` rows are loaded (e.g. before jumping to a search
 * match beyond the loaded window). Resolves once loaded or history ends.
 */
export async function ensureGraphLoaded(count: number): Promise<void> {
  for (let guard = 0; guard < 100; guard++) {
    if (moreInFlight) {
      await moreInFlight;
      continue;
    }
    const g = get(commitGraph);
    if (!g || g.entries.length >= count || !g.has_more) return;
    await loadMoreCommits(count - g.entries.length);
    // Nothing changed (error or repo switch): give up instead of spinning.
    if (get(commitGraph) === g) return;
  }
}

/** Load the whole remaining history. */
export function loadEntireGraph(): Promise<void> {
  return ensureGraphLoaded(Number.MAX_SAFE_INTEGER);
}
