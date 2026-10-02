/**
 * Repositories found in the default repository folder (Settings > General),
 * shared by the tab bar "+" menu and the command palette.
 */
import { get, writable } from "svelte/store";
import type { RepoInfo } from "../types/git";
import { settings } from "./settings";
import * as tauri from "../tauri";

const TTL_MS = 30_000;

export const scannedRepos = writable<RepoInfo[]>([]);

let lastDir: string | null = null;
let lastAt = 0;
let inflight: Promise<void> | null = null;

/** Rescan the default folder if it changed or the last scan is stale. */
export function refreshScannedRepos(force = false): Promise<void> {
  const dir = get(settings).default_repo_dir ?? null;
  if (!dir) {
    lastDir = null;
    scannedRepos.set([]);
    return Promise.resolve();
  }
  if (inflight) return inflight;
  if (!force && dir === lastDir && Date.now() - lastAt < TTL_MS) return Promise.resolve();
  inflight = tauri
    .listReposInDir(dir)
    .then((repos) => {
      lastDir = dir;
      lastAt = Date.now();
      scannedRepos.set(repos);
    })
    .catch(() => scannedRepos.set([]))
    .finally(() => (inflight = null));
  return inflight;
}
