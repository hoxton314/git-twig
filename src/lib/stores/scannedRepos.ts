/**
 * Repositories found in the default repository folder (Settings > General),
 * shared by the tab bar "+" menu and the command palette.
 */
import { get, writable } from "svelte/store";
import type { RepoInfo } from "../types/git";
import { settings } from "./settings";
import * as tauri from "../tauri";

/** The palette re-uses a scan this long; the "+" menu always rescans. */
const TTL_MS = 30_000;

export const scannedRepos = writable<RepoInfo[]>([]);

let lastDir: string | null = null;
let lastAt = 0;
let inflight: { dir: string; promise: Promise<void> } | null = null;

const currentDir = () => get(settings).default_repo_dir ?? null;

/** Rescan the default folder if forced, it changed, or the last scan is stale. */
export function refreshScannedRepos(force = false): Promise<void> {
  const dir = currentDir();
  if (!dir) {
    lastDir = null;
    scannedRepos.set([]);
    return Promise.resolve();
  }
  if (inflight && inflight.dir === dir) return inflight.promise;
  if (!force && dir === lastDir && Date.now() - lastAt < TTL_MS) return Promise.resolve();
  const promise = tauri
    .listReposInDir(dir)
    .catch(() => [] as RepoInfo[])
    .then((repos) => {
      // Failures are cached too, so an unreadable folder isn't rescanned on
      // every palette keystroke.
      lastDir = dir;
      lastAt = Date.now();
      // The setting may have changed while scanning: drop stale results.
      if (currentDir() === dir) scannedRepos.set(repos);
    })
    .finally(() => {
      if (inflight?.promise === promise) inflight = null;
    });
  inflight = { dir, promise };
  return promise;
}
