/**
 * Recently opened and pinned (favorite) repositories, persisted in
 * `repo_history.json` (app data dir) independently of the default repo folder.
 */
import { writable, derived, get } from "svelte/store";
import type { RecentRepo, RepoGroup, RepoHistory, RepoInfo } from "../types/git";
import * as groupOps from "../repoGroups";
import * as tauri from "../tauri";

const MAX_RECENT = 30;

export const repoHistory = writable<RepoHistory>({ recent: [], favorites: [], groups: [] });

/** Paths (recent or favorite) that no longer exist on disk. */
export const missingRepoPaths = writable<Set<string>>(new Set());

export const favoritePaths = derived(repoHistory, ($h) => new Set($h.favorites));

/** Favorites with display names (from the recent list, else the folder name). */
export const favoriteRepos = derived(repoHistory, ($h): RecentRepo[] =>
  $h.favorites.map((path) => {
    const known = $h.recent.find((r) => r.path === path);
    return known ?? { path, name: baseName(path), last_opened: 0 };
  }),
);

export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

let loaded = false;
let resolveReady: () => void = () => {};
const ready = new Promise<void>((r) => (resolveReady = r));
let saveTimeout: ReturnType<typeof setTimeout> | null = null;

function persist() {
  if (!loaded) return;
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(() => {
    saveTimeout = null;
    const value = get(repoHistory);
    tauri
      .saveRepoHistory(value)
      .then(() => tauri.emitSync("repo-history", value))
      .catch((e) => console.error("Failed to save repo history:", e));
  }, 300);
}

/** Load from disk and check which paths are missing. Call once at startup. */
export async function loadRepoHistory() {
  try {
    const h = await tauri.loadRepoHistory();
    repoHistory.set({ recent: h.recent ?? [], favorites: h.favorites ?? [], groups: h.groups ?? [] });
  } catch (e) {
    console.error("Failed to load repo history:", e);
  }
  loaded = true;
  resolveReady();
  await refreshMissingPaths();
}

/** Re-check which remembered repos still exist (e.g. when the home screen shows). */
export async function refreshMissingPaths() {
  const h = get(repoHistory);
  const paths = [...new Set([...h.recent.map((r) => r.path), ...h.favorites, ...h.groups.flatMap((g) => g.paths)])];
  if (paths.length === 0) {
    missingRepoPaths.set(new Set());
    return;
  }
  try {
    const exists = await tauri.repoPathsExist(paths);
    missingRepoPaths.set(new Set(paths.filter((_, i) => !exists[i])));
  } catch {
    // Leave the previous state; a failed check must not hide entries.
  }
}

async function mutate(fn: (h: RepoHistory) => RepoHistory) {
  await ready;
  repoHistory.update(fn);
  persist();
}

/** Move a just-opened repo to the top of the recent list. */
export function recordRecentRepo(info: Pick<RepoInfo, "path" | "name">) {
  missingRepoPaths.update((s) => {
    if (!s.has(info.path)) return s;
    const next = new Set(s);
    next.delete(info.path);
    return next;
  });
  return mutate((h) => ({
    ...h,
    recent: [
      { path: info.path, name: info.name, last_opened: Date.now() },
      ...h.recent.filter((r) => r.path !== info.path),
    ].slice(0, MAX_RECENT),
  }));
}

export function removeRecentRepo(path: string) {
  return mutate((h) => ({ ...h, recent: h.recent.filter((r) => r.path !== path) }));
}

export function clearRecentRepos() {
  return mutate((h) => ({ ...h, recent: [] }));
}

export function isFavoriteRepo(path: string): boolean {
  return get(repoHistory).favorites.includes(path);
}

export function setFavoriteRepo(path: string, favorite: boolean, name?: string) {
  return mutate((h) => {
    const favorites = h.favorites.filter((p) => p !== path);
    if (favorite) favorites.push(path);
    // Remember the display name even if the repo was never in "recent".
    const recent =
      favorite && name && !h.recent.some((r) => r.path === path)
        ? [...h.recent, { path, name, last_opened: 0 }].slice(0, MAX_RECENT)
        : h.recent;
    return { ...h, favorites, recent };
  });
}

export function toggleFavoriteRepo(path: string, name?: string) {
  return setFavoriteRepo(path, !isFavoriteRepo(path), name);
}

// ── Groups ───────────────────────────────────────────────────────────

export const repoGroups = derived(repoHistory, ($h) => $h.groups);

/** Display name for a remembered path (recent list, else the folder name). */
export function repoDisplayName(h: RepoHistory, path: string): string {
  return h.recent.find((r) => r.path === path)?.name ?? baseName(path);
}

/** Create a group (optionally with repositories); resolves to its id. */
export async function createRepoGroup(name: string, paths: string[] = []): Promise<string> {
  await ready;
  const id = groupOps.newGroupId(get(repoHistory).groups);
  await mutate((h) => ({ ...h, groups: groupOps.createGroup(h.groups, id, name, paths) }));
  return id;
}

export function renameRepoGroup(id: string, name: string) {
  return mutate((h) => ({ ...h, groups: groupOps.renameGroup(h.groups, id, name) }));
}

export function deleteRepoGroup(id: string) {
  return mutate((h) => ({ ...h, groups: groupOps.deleteGroup(h.groups, id) }));
}

/** Add `path` to a group, remembering its display name like favorites do. */
export function addRepoToGroup(id: string, path: string, name?: string) {
  return mutate((h) => {
    const recent =
      name && !h.recent.some((r) => r.path === path)
        ? [...h.recent, { path, name, last_opened: 0 }].slice(0, MAX_RECENT)
        : h.recent;
    return { ...h, recent, groups: groupOps.addToGroup(h.groups, id, path) };
  });
}

export function removeRepoFromGroup(id: string, path: string) {
  return mutate((h) => ({ ...h, groups: groupOps.removeFromGroup(h.groups, id, path) }));
}

export function findGroup(id: string): RepoGroup | undefined {
  return get(repoHistory).groups.find((g) => g.id === id);
}

/** Take the history another window just saved (not re-saved). */
export function applyRemoteHistory(payload: unknown) {
  const h = payload as RepoHistory;
  repoHistory.set({ recent: h.recent ?? [], favorites: h.favorites ?? [], groups: h.groups ?? [] });
}
