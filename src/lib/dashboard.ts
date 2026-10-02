/**
 * Repository dashboard: which repositories it covers, and a bounded-
 * concurrency runner for "Fetch all" / "Pull all". Pure; the panel is
 * `components/dashboard/Dashboard.svelte`.
 */
import { writable } from "svelte/store";
import type { RepoHistory } from "./types/git";

/** Open dashboard: a group's repositories, or (null) every known one. */
export const dashboardScope = writable<{ groupId: string | null } | null>(null);

export function openDashboard(groupId: string | null = null) {
  dashboardScope.set({ groupId });
}

/** Repositories a dashboard shows: the group's, or open tabs + groups + favorites + recent. */
export function dashboardPaths(groupId: string | null, history: RepoHistory, openPaths: string[]): string[] {
  if (groupId) return history.groups.find((g) => g.id === groupId)?.paths ?? [];
  return [
    ...new Set([
      ...openPaths,
      ...history.groups.flatMap((g) => g.paths),
      ...history.favorites,
      ...history.recent.filter((r) => r.last_opened > 0).map((r) => r.path),
    ]),
  ];
}

/**
 * Run `worker` over `items`, at most `limit` at a time, in order. Stops
 * starting new items once `cancelled()` is true (running ones finish).
 */
export async function runPool<T>(
  items: T[],
  limit: number,
  worker: (item: T) => Promise<void>,
  cancelled: () => boolean = () => false,
): Promise<void> {
  let next = 0;
  const lanes = Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, async () => {
    while (next < items.length && !cancelled()) {
      const item = items[next++];
      await worker(item);
    }
  });
  await Promise.all(lanes);
}

/** "3 min ago" style age of a unix-seconds timestamp (null → "never"). */
export function fetchAge(unixSeconds: number | null, nowMs = Date.now()): string {
  if (unixSeconds === null) return "never";
  const s = Math.max(0, nowMs / 1000 - unixSeconds);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}
