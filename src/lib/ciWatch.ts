/**
 * Notify when CI finishes for the checked-out commit of an open repository.
 * While enabled, each open repository's HEAD is kept in the CI cache
 * (`refreshCi` honours its TTLs: 20 s while pending, minutes once settled),
 * and a pending → passed / failed transition is announced.
 */
import { get } from "svelte/store";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import * as tauri from "./tauri";
import { openRepos } from "./stores/repos";
import { globalSettings } from "./stores/settings";
import { ciCache, ciKey, refreshCi } from "./stores/ci";
import { announce } from "./notify";
import type { CiState, CiStatus } from "./types/hosting";

export type CiNotifyMode = "off" | "failures" | "all";

const TICK_MS = 30_000;

/** What to announce when a watched commit's state changes (pure; exported for tests). */
export function ciTransition(
  prev: CiState | undefined,
  next: CiState | undefined,
  mode: CiNotifyMode,
): "success" | "failure" | null {
  if (mode === "off" || prev !== "pending") return null;
  if (next === "failure") return "failure";
  if (next === "success" && mode === "all") return "success";
  return null;
}

/** Link to the run: the first failing check's page, else any check's. */
export function runUrl(status: CiStatus): string | null {
  return (status.checks.find((c) => c.state === "failure" && c.url) ?? status.checks.find((c) => c.url))?.url ?? null;
}

interface Watched {
  repoPath: string;
  name: string;
  branch: string;
  sha: string;
}

/** Cache key → the HEAD commit it belongs to. */
let watched = new Map<string, Watched>();
const lastState = new Map<string, CiState>();
let timer: ReturnType<typeof setInterval> | null = null;
let unsubscribe: (() => void) | null = null;

async function tick() {
  const mode = get(globalSettings).notify_ci as CiNotifyMode;
  if (mode === "off") {
    watched = new Map();
    return;
  }
  const next = new Map<string, Watched>();
  for (const [repoPath, info] of get(openRepos)) {
    try {
      const head = (await tauri.getBranches(repoPath)).find((b) => b.is_head && !b.is_remote);
      if (!head?.upstream) continue; // CI only runs on pushed branches
      next.set(ciKey(repoPath, head.oid), { repoPath, name: info.name, branch: head.name, sha: head.oid });
      void refreshCi(head.oid, repoPath);
    } catch {
      // Unreadable repo: skip this round.
    }
  }
  watched = next;
  for (const k of [...lastState.keys()]) if (!next.has(k)) lastState.delete(k);
}

function onCache(entries: Map<string, { status: CiStatus | null }>) {
  const mode = get(globalSettings).notify_ci as CiNotifyMode;
  for (const [k, w] of watched) {
    const status = entries.get(k)?.status ?? null;
    const state = status?.state;
    const what = ciTransition(lastState.get(k), state, mode);
    if (state) lastState.set(k, state);
    if (!what || !status) continue;
    const url = runUrl(status);
    const sha = w.sha.slice(0, 7);
    void announce(
      what === "success" ? `CI passed: ${w.name}` : `CI failed: ${w.name}`,
      `${w.branch} at ${sha}${what === "failure" ? ` — ${status.checks.filter((c) => c.state === "failure").map((c) => c.name).slice(0, 3).join(", ") || "a check failed"}` : ""}`,
      {
        kind: what === "success" ? "success" : "warning",
        action: url ? { label: "Open run", run: () => void openUrl(url).catch(() => {}) } : undefined,
      },
    );
  }
}

/** Start watching (call once at startup); returns a stop function. */
export function initCiWatch(): () => void {
  stopCiWatch();
  unsubscribe = ciCache.subscribe(onCache);
  timer = setInterval(() => void tick(), TICK_MS);
  void tick();
  return stopCiWatch;
}

export function stopCiWatch() {
  unsubscribe?.();
  unsubscribe = null;
  if (timer) clearInterval(timer);
  timer = null;
  watched = new Map();
  lastState.clear();
}
