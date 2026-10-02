/**
 * Repository groups: pure list operations (tested) used by the
 * `repoHistory` store, which persists them in `repo_history.json`.
 */
import type { RepoGroup } from "./types/git";

export function newGroupId(existing: RepoGroup[]): string {
  const ids = new Set(existing.map((g) => g.id));
  for (;;) {
    const id = `g${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
    if (!ids.has(id)) return id;
  }
}

/** Longest group name (the backend keeps the same limit). */
export const MAX_GROUP_NAME = 100;

/** A name that isn't taken yet ("Group", "Group 2", …), at most MAX_GROUP_NAME characters. */
export function uniqueGroupName(groups: RepoGroup[], wanted: string): string {
  const base = [...(wanted.trim() || "Group")].slice(0, MAX_GROUP_NAME - 4).join("").trim();
  const taken = new Set(groups.map((g) => g.name.toLowerCase()));
  if (!taken.has(base.toLowerCase())) return base;
  for (let n = 2; ; n++) {
    const name = `${base} ${n}`;
    if (!taken.has(name.toLowerCase())) return name;
  }
}

export function createGroup(groups: RepoGroup[], id: string, name: string, paths: string[] = []): RepoGroup[] {
  return [...groups, { id, name: uniqueGroupName(groups, name), paths: [...new Set(paths)] }];
}

export function renameGroup(groups: RepoGroup[], id: string, name: string): RepoGroup[] {
  const trimmed = name.trim();
  if (!trimmed) return groups;
  const others = groups.filter((g) => g.id !== id);
  return groups.map((g) => (g.id === id ? { ...g, name: uniqueGroupName(others, trimmed) } : g));
}

export function deleteGroup(groups: RepoGroup[], id: string): RepoGroup[] {
  return groups.filter((g) => g.id !== id);
}

export function addToGroup(groups: RepoGroup[], id: string, path: string): RepoGroup[] {
  return groups.map((g) => (g.id === id && !g.paths.includes(path) ? { ...g, paths: [...g.paths, path] } : g));
}

export function removeFromGroup(groups: RepoGroup[], id: string, path: string): RepoGroup[] {
  return groups.map((g) => (g.id === id ? { ...g, paths: g.paths.filter((p) => p !== path) } : g));
}

/** Groups that contain `path`. */
export function groupsOf(groups: RepoGroup[], path: string): RepoGroup[] {
  return groups.filter((g) => g.paths.includes(path));
}
