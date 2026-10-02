import { get } from "svelte/store";
import { openRepos, activeRepoPath } from "./repos";
import { settings } from "./settings";
import { refreshAll } from "./graph";
import * as tauri from "../tauri";
import { invalidateCi } from "./ci";

let intervalId: ReturnType<typeof setInterval> | null = null;
let currentSeconds = -1;
let fetching = false;
let unsubscribe: (() => void) | null = null;

async function fetchAllRepos() {
  // Skip this tick if the previous round is still running (slow remotes),
  // so fetches never pile up on top of each other.
  if (fetching) return;
  fetching = true;
  try {
    const paths = [...get(openRepos).keys()];
    for (const path of paths) {
      // The tab may have been closed while an earlier fetch was running.
      if (!get(openRepos).has(path)) continue;
      try {
        await tauri.fetchAll(path);
        invalidateCi(path);
      } catch {
        // Silently skip repos that fail to fetch
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

function setupInterval(seconds: number) {
  // Settings fire on every change (theme, accent, ...); only restart the
  // timer when the interval itself changed, or it may never get to fire.
  if (seconds === currentSeconds) return;
  currentSeconds = seconds;
  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }
  if (seconds > 0) {
    intervalId = setInterval(fetchAllRepos, seconds * 1000);
  }
}

/**
 * Subscribe to settings changes and manage the auto-fetch timer. Call once at
 * startup; returns a cleanup function that stops the timer.
 */
export function initAutoFetch(): () => void {
  stopAutoFetch();
  unsubscribe = settings.subscribe((s) => {
    setupInterval(s.auto_fetch_interval);
  });
  return stopAutoFetch;
}

/** Stop the auto-fetch timer and settings subscription. */
export function stopAutoFetch() {
  unsubscribe?.();
  unsubscribe = null;
  if (intervalId) clearInterval(intervalId);
  intervalId = null;
  currentSeconds = -1;
}
