import { writable, derived } from "svelte/store";
import type { RepoInfo } from "../types/git";

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
