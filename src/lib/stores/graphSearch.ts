/**
 * Commit search state for the graph. Searches run in Rust over the whole
 * history (not just the loaded pages); each match carries its graph row so
 * the graph can page in up to it before jumping.
 */
import { writable, derived, get } from "svelte/store";
import type { CommitSearchResult } from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath } from "./repos";
import { commitGraph, graphOptions } from "./graph";

export type GraphSearchMode = "highlight" | "filter";
/** What to search: commit metadata, or code changes (pickaxe). */
export type GraphSearchKind = "commits" | "changes";

/** Most matches a search returns; the UI says when results were cut off. */
export const SEARCH_MAX_RESULTS = 5000;

export const searchOpen = writable(false);
export const searchQuery = writable("");
export const searchMode = writable<GraphSearchMode>("highlight");
export const searchKind = writable<GraphSearchKind>("commits");
/** Options for code-change searches. */
export const changeOptions = writable({ regex: false, matchCase: false, paths: "" });
export const searchResult = writable<CommitSearchResult | null>(null);
export const searchBusy = writable(false);
export const searchError = writable<string | null>(null);
/** Position in `searchResult.matches` of the current match, or -1. */
export const currentMatch = writable(-1);

export const matchOids = derived(
  searchResult,
  (r) => new Set(r?.matches.map((m) => m.commit.oid) ?? []),
);

/** True while there is a query with results to highlight or filter by. */
export const searchActive = derived(
  [searchOpen, searchQuery, searchResult],
  ([open, q, r]) => open && q.trim() !== "" && r !== null,
);

let gen = 0;
let timer: ReturnType<typeof setTimeout> | null = null;

/**
 * Debounced search (use while typing). Code-change searches walk every
 * diff in history, so typing only clears them; they run on Enter, or when
 * `force`d (repo or history changed).
 */
export function scheduleSearch(delay = 250, force = false) {
  if (timer) clearTimeout(timer);
  timer = null;
  if (get(searchKind) === "changes" && !force) {
    gen++;
    searchResult.set(null);
    searchError.set(null);
    if (get(searchBusy)) stopChangeSearch();
    searchBusy.set(false);
    return;
  }
  timer = setTimeout(() => {
    timer = null;
    void runSearch();
  }, delay);
}

/** Run the search for the current query now. */
export async function runSearch(): Promise<void> {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  const path = get(activeRepoPath);
  const q = get(searchQuery).trim();
  const my = ++gen;
  if (!path || !q) {
    searchResult.set(null);
    searchError.set(null);
    searchBusy.set(false);
    currentMatch.set(-1);
    return;
  }
  // Keep the current match on the same commit across re-runs.
  const prev = get(searchResult)?.matches[get(currentMatch)]?.commit.oid ?? null;
  searchBusy.set(true);
  try {
    const opts = get(changeOptions);
    const r =
      get(searchKind) === "changes"
        ? await tauri.searchChanges(
            path,
            q,
            { regex: opts.regex, ignoreCase: !opts.matchCase, paths: opts.paths.split(/[,\s]+/).filter(Boolean) },
            graphOptions(),
            SEARCH_MAX_RESULTS,
          )
        : await tauri.searchCommits(path, q, graphOptions(), SEARCH_MAX_RESULTS);
    if (my !== gen || get(activeRepoPath) !== path) return;
    searchResult.set(r);
    searchError.set(null);
    currentMatch.set(prev ? r.matches.findIndex((m) => m.commit.oid === prev) : -1);
  } catch (err) {
    if (my !== gen) return;
    searchResult.set(null);
    currentMatch.set(-1);
    searchError.set(err instanceof Error ? err.message : String(err));
  } finally {
    if (my === gen) searchBusy.set(false);
  }
}

/** Kill a running code-change search in the backend (best effort). */
function stopChangeSearch() {
  const path = get(activeRepoPath);
  if (path) tauri.cancelSearchChanges(path).catch(() => {});
}

/** Close the search bar and clear everything. */
export function closeSearch() {
  if (get(searchBusy) && get(searchKind) === "changes") stopChangeSearch();
  gen++;
  if (timer) clearTimeout(timer);
  timer = null;
  searchOpen.set(false);
  searchQuery.set("");
  searchResult.set(null);
  searchError.set(null);
  searchBusy.set(false);
  currentMatch.set(-1);
}

// Results belong to one repo/history: re-run when either changes.
let lastPath: string | null = null;
activeRepoPath.subscribe((p) => {
  if (p === lastPath) return;
  // A code-change search still scanning the previous repo's history.
  if (lastPath && get(searchBusy) && get(searchKind) === "changes") {
    tauri.cancelSearchChanges(lastPath).catch(() => {});
  }
  lastPath = p;
  gen++;
  searchResult.set(null);
  currentMatch.set(-1);
  if (get(searchQuery).trim()) scheduleSearch(0, true);
});

commitGraph.subscribe((g) => {
  const r = get(searchResult);
  if (g && r && g.tips !== r.tips && get(searchQuery).trim()) scheduleSearch(0, true);
});
