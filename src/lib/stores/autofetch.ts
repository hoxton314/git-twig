import { get } from "svelte/store";
import { openRepos } from "./repos";
import { settings } from "./settings";
import * as tauri from "../tauri";

let intervalId: ReturnType<typeof setInterval> | null = null;

async function fetchAllRepos() {
  const repos = get(openRepos);
  for (const path of repos.keys()) {
    try {
      await tauri.fetchAll(path);
    } catch {
      // Silently skip repos that fail to fetch
    }
  }
}

function setupInterval(seconds: number) {
  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }
  if (seconds > 0) {
    intervalId = setInterval(fetchAllRepos, seconds * 1000);
  }
}

/** Subscribe to settings changes and manage the auto-fetch timer. Call once at startup. */
export function initAutoFetch() {
  settings.subscribe((s) => {
    setupInterval(s.auto_fetch_interval);
  });
}
