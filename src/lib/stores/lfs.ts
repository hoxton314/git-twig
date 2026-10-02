/** Git LFS panel state for the active repo. */
import { writable, get } from "svelte/store";
import * as tauri from "../tauri";
import type { LfsStatus } from "../types/git";
import { activeRepoPath } from "./repos";

export const lfsPanelOpen = writable(false);

/** git-lfs version + tracked patterns for the active repo (null until loaded). */
export const lfsStatus = writable<LfsStatus | null>(null);
/** Why the last status load failed (null when it didn't). */
export const lfsError = writable<string | null>(null);

let gen = 0;

export async function refreshLfsStatus(path?: string): Promise<void> {
  const p = path ?? get(activeRepoPath);
  const my = ++gen;
  if (!p) {
    lfsStatus.set(null);
    return;
  }
  try {
    const st = await tauri.getLfsStatus(p);
    if (my === gen && get(activeRepoPath) === p) {
      lfsStatus.set(st);
      lfsError.set(null);
    }
  } catch (err) {
    if (my === gen && get(activeRepoPath) === p) {
      lfsStatus.set(null);
      lfsError.set(err instanceof Error ? err.message : String(err));
    }
  }
}

let lastPath: string | null = null;
activeRepoPath.subscribe((p) => {
  if (p === lastPath) return;
  lastPath = p;
  lfsPanelOpen.set(false);
  lfsStatus.set(null);
  lfsError.set(null);
  void refreshLfsStatus(p ?? undefined);
});

/** What `git lfs prune --dry-run` would delete ("N files would be pruned (size)"). */
export function pruneSummary(output: string): { count: number; size: string | null } {
  const m = output.match(/(\d+)\s+files?\s+would be pruned(?:\s*\(([^)]+)\))?/i);
  return m ? { count: Number(m[1]), size: m[2] ?? null } : { count: 0, size: null };
}
