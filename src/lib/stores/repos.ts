import { writable, derived, get } from "svelte/store";
import type { RepoInfo } from "../types/git";
import * as tauri from "../tauri";
import { sidebarWidth, stagingWidth, diffPanelRatio } from "./ui";

/** All open repos, keyed by path. */
export const openRepos = writable<Map<string, RepoInfo>>(new Map());

/** Path of the currently active (visible) tab. */
export const activeRepoPath = writable<string | null>(null);

/** Derived: the RepoInfo for the active tab. */
export const activeRepo = derived(
  [openRepos, activeRepoPath],
  ([$repos, $path]) => ($path ? $repos.get($path) ?? null : null)
);

/** Add a repo to the open set and make it active. */
export function addRepo(info: RepoInfo) {
  openRepos.update((m) => {
    m.set(info.path, info);
    return m;
  });
  activeRepoPath.set(info.path);
}

/** Remove a repo tab. Switches to the nearest remaining tab. */
export function removeRepo(path: string) {
  openRepos.update((m) => {
    const keys = [...m.keys()];
    const idx = keys.indexOf(path);
    m.delete(path);

    // If we just closed the active tab, pick an adjacent one
    activeRepoPath.update((current) => {
      if (current !== path) return current;
      const remaining = [...m.keys()];
      if (remaining.length === 0) return null;
      return remaining[Math.min(idx, remaining.length - 1)];
    });
    return m;
  });
}

/** Update the info for an already-open repo (e.g. after checkout). */
export function updateRepo(info: RepoInfo) {
  openRepos.update((m) => {
    m.set(info.path, info);
    return m;
  });
}

// ── Session persistence ──────────────────────────────────────────────

let saveTimeout: ReturnType<typeof setTimeout> | null = null;
let sessionReady = false;

function persistSession() {
  if (!sessionReady) return;
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(() => {
    const repos = get(openRepos);
    const active = get(activeRepoPath);
    tauri.saveSession(
      [...repos.keys()],
      active,
      get(sidebarWidth),
      get(stagingWidth),
      get(diffPanelRatio),
    ).catch((e) => {
      console.error("Failed to save session:", e);
    });
  }, 300);
}

openRepos.subscribe(() => persistSession());
activeRepoPath.subscribe(() => persistSession());
sidebarWidth.subscribe(() => persistSession());
stagingWidth.subscribe(() => persistSession());
diffPanelRatio.subscribe(() => persistSession());

/** Restore previously open repos from disk. Call once at startup. */
export async function restoreSession() {
  try {
    const session = await tauri.loadSession();

    // Restore layout values before opening repos
    if (session.sidebar_width != null) sidebarWidth.set(session.sidebar_width);
    if (session.staging_width != null) stagingWidth.set(session.staging_width);
    if (session.diff_panel_ratio != null) diffPanelRatio.set(session.diff_panel_ratio);

    if (session.paths.length === 0) {
      sessionReady = true;
      return;
    }

    for (const path of session.paths) {
      try {
        const info = await tauri.openRepo(path);
        openRepos.update((m) => {
          m.set(info.path, info);
          return m;
        });
      } catch {
        // Repo no longer exists or isn't accessible — skip it
      }
    }

    // Restore active tab (fall back to first open repo)
    const repos = get(openRepos);
    if (session.active && repos.has(session.active)) {
      activeRepoPath.set(session.active);
    } else if (repos.size > 0) {
      activeRepoPath.set([...repos.keys()][0]);
    }
  } catch {
    // No saved session or corrupt file — start fresh
  }
  sessionReady = true;
}
