/** Git LFS: tracked patterns, locks, fetch / prune. */
import { invoke } from "@tauri-apps/api/core";
import type { LfsLock, LfsStatus } from "../types/git";

export function getLfsStatus(path: string): Promise<LfsStatus> {
  return invoke<LfsStatus>("get_lfs_status", { path });
}

/** Track `pattern` (writes .gitattributes). */
export function lfsTrack(path: string, pattern: string, lockable: boolean): Promise<string> {
  return invoke<string>("lfs_track", { path, pattern, lockable });
}

export function lfsUntrack(path: string, pattern: string): Promise<string> {
  return invoke<string>("lfs_untrack", { path, pattern });
}

/** Locks on the LFS server (network). */
export function getLfsLocks(path: string): Promise<LfsLock[]> {
  return invoke<LfsLock[]>("get_lfs_locks", { path });
}

export function lfsLock(path: string, file: string): Promise<void> {
  return invoke<void>("lfs_lock", { path, file });
}

/** `force` releases someone else's lock. */
export function lfsUnlock(path: string, id: string, force: boolean): Promise<void> {
  return invoke<void>("lfs_unlock", { path, id, force });
}

/** Download LFS objects for the checkout (`all`: every ref). */
export function lfsFetch(path: string, all: boolean): Promise<string> {
  return invoke<string>("lfs_fetch", { path, all });
}

/** Delete old local LFS objects (`dryRun`: only report). */
export function lfsPrune(path: string, dryRun: boolean): Promise<string> {
  return invoke<string>("lfs_prune", { path, dryRun });
}
