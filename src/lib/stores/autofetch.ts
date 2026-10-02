import { get } from "svelte/store";
import { openRepos, activeRepoPath } from "./repos";
import { globalSettings, repoOverrides } from "./settings";
import { effectiveSettings } from "../repoSettings";
import { refreshAll } from "./graph";
import * as tauri from "../tauri";
import { trackOperation } from "./operations";
import { invalidateCi } from "./ci";

let intervalId: ReturnType<typeof setInterval> | null = null;
let fetching = false;

/** How often the scheduler checks which repositories are due. */
const TICK_MS = 15_000;

/** When each open repository was last auto-fetched (or first seen). */
const lastFetched = new Map<string, number>();

/** Auto-fetch interval (seconds) for `path`, honouring its overrides. */
function intervalFor(path: string): number {
  return effectiveSettings(get(globalSettings), get(repoOverrides), path).auto_fetch_interval;
}

/** Open repositories whose interval has elapsed (pure; exported for tests). */
export function dueRepos(paths: string[], now: number, interval: (p: string) => number, last: Map<string, number>): string[] {
  return paths.filter((p) => {
    const secs = interval(p);
    if (secs <= 0) return false;
    if (!last.has(p)) {
      // First seen: wait a full interval, like a fresh timer would.
      last.set(p, now);
      return false;
    }
    return now - (last.get(p) ?? now) >= secs * 1000;
  });
}

async function tick() {
  // Skip this tick if the previous round is still running (slow remotes),
  // so fetches never pile up on top of each other.
  if (fetching) return;
  const paths = [...get(openRepos).keys()];
  for (const p of [...lastFetched.keys()]) if (!paths.includes(p)) lastFetched.delete(p);
  const due = dueRepos(paths, Date.now(), intervalFor, lastFetched);
  if (due.length === 0) return;
  fetching = true;
  try {
    for (const path of due) {
      // The tab may have been closed while an earlier fetch was running.
      if (!get(openRepos).has(path)) continue;
      lastFetched.set(path, Date.now());
      try {
        // Failures are recorded in `lastFetch` and shown in the status bar
        // rather than interrupting the user from a background timer.
        await trackOperation(path, "fetch", "Auto-fetching…", () => tauri.fetchAll(path), { background: true });
        invalidateCi(path);
      } catch {
        // Recorded by trackOperation; skip to the next repo.
      }
      // Reflect new remote refs / ahead-behind counts in the visible repo.
      if (get(activeRepoPath) === path) {
        refreshAll(path);
      }
    }
  } finally {
    fetching = false;
  }
}

/**
 * Start the auto-fetch scheduler: every repository is fetched on its own
 * interval (the global one, or its per-repository override). Call once at
 * startup; returns a cleanup function that stops it.
 */
export function initAutoFetch(): () => void {
  stopAutoFetch();
  intervalId = setInterval(() => void tick(), TICK_MS);
  return stopAutoFetch;
}

/** Stop the auto-fetch scheduler. */
export function stopAutoFetch() {
  if (intervalId) clearInterval(intervalId);
  intervalId = null;
  lastFetched.clear();
}
