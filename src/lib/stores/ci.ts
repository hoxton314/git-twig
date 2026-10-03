/**
 * CI status cache (GitHub combined status + check runs, GitLab pipeline
 * statuses, Gitea commit statuses), keyed by repo + commit SHA.
 *
 * Usage from any component:
 *   const ci = ciStatusFor(sha);            // active repo, preferred remote
 *   $ci.status?.state  // "success" | "failure" | "pending" | "neutral" | "none"
 *
 * Results are cached with a TTL (shorter while CI is still running) and
 * re-fetched for live subscribers after `invalidateCi()` — call it after a
 * fetch/push. Errors (no token, no hosted remote) are cached too so they
 * don't hammer the API; `CiBadge.svelte` simply renders nothing then.
 */
import { get, readable, writable, type Readable } from "svelte/store";
import type { CiStatus } from "../types/hosting";
import { activeRepoPath } from "./repos";
import * as tauri from "../tauri";

export interface CiEntry {
  status: CiStatus | null;
  error: string | null;
  loading: boolean;
}

interface CacheEntry extends CiEntry {
  fetchedAt: number;
}

const TTL_MS = 5 * 60_000;
const PENDING_TTL_MS = 20_000;
const ERROR_TTL_MS = 2 * 60_000;
/** How often live subscribers re-check staleness. */
const TICK_MS = 15_000;

const cache = writable<Map<string, CacheEntry>>(new Map());
const inflight = new Map<string, Promise<void>>();
/** Bumped by invalidateCi() so live subscribers refetch. */
const generation = writable(0);

const EMPTY: CiEntry = { status: null, error: null, loading: false };

function key(repoPath: string, sha: string, remote: string | null): string {
  return `${repoPath}\u0000${remote ?? ""}\u0000${sha}`;
}

function isStale(e: CacheEntry | undefined): boolean {
  if (!e) return true;
  if (e.loading) return false;
  const age = Date.now() - e.fetchedAt;
  if (e.error) return age > ERROR_TTL_MS;
  if (e.status?.state === "pending") return age > PENDING_TTL_MS;
  return age > TTL_MS;
}

function setEntry(k: string, entry: CacheEntry) {
  cache.update((m) => {
    const next = new Map(m);
    next.set(k, entry);
    return next;
  });
}

/** Fetch (or refetch when stale / forced) the CI status of `sha`. */
export function refreshCi(
  sha: string,
  repoPath: string | null = get(activeRepoPath),
  remote: string | null = null,
  force = false,
): Promise<void> {
  if (!sha || !repoPath) return Promise.resolve();
  const k = key(repoPath, sha, remote);
  const current = get(cache).get(k);
  if (!force && !isStale(current)) return Promise.resolve();
  const running = inflight.get(k);
  if (running) return running;

  setEntry(k, {
    status: current?.status ?? null,
    error: null,
    loading: true,
    fetchedAt: current?.fetchedAt ?? 0,
  });
  const p = tauri
    .hostingCiStatus(repoPath, remote, sha)
    .then((status) => setEntry(k, { status, error: null, loading: false, fetchedAt: Date.now() }))
    .catch((err) =>
      setEntry(k, { status: null, error: String(err), loading: false, fetchedAt: Date.now() }),
    )
    .finally(() => inflight.delete(k));
  inflight.set(k, p);
  return p;
}

/**
 * Reactive CI status for a commit. Loads on first subscribe, refreshes
 * while subscribed when the entry goes stale or after `invalidateCi()`.
 */
export function ciStatusFor(
  sha: string,
  repoPath: string | null = get(activeRepoPath),
  remote: string | null = null,
): Readable<CiEntry> {
  if (!sha || !repoPath) return readable(EMPTY);
  const k = key(repoPath, sha, remote);
  return readable<CiEntry>(EMPTY, (set) => {
    const unsubCache = cache.subscribe((m) => {
      const e = m.get(k);
      set(e ? { status: e.status, error: e.error, loading: e.loading } : EMPTY);
    });
    const unsubGen = generation.subscribe(() => void refreshCi(sha, repoPath, remote));
    const timer = setInterval(() => void refreshCi(sha, repoPath, remote), TICK_MS);
    return () => {
      unsubCache();
      unsubGen();
      clearInterval(timer);
    };
  });
}

/** Mark cached statuses stale (all, or one repo's) and refetch live ones. */
export function invalidateCi(repoPath?: string) {
  cache.update((m) => {
    const next = new Map<string, CacheEntry>();
    for (const [k, e] of m) {
      const stale = !repoPath || k.startsWith(`${repoPath}\u0000`);
      next.set(k, stale && !e.loading ? { ...e, fetchedAt: 0 } : e);
    }
    return next;
  });
  generation.update((g) => g + 1);
}

/** Drop everything (e.g. after the token or host settings changed). */
export function clearCi() {
  cache.set(new Map());
  generation.update((g) => g + 1);
}

/** Every cached CI entry, keyed by repo path + remote + sha (for watchers). */
export const ciCache: Readable<Map<string, CiEntry>> = { subscribe: cache.subscribe };

/** Cache key of `sha` in `repoPath` (default remote), as used by `ciCache`. */
export function ciKey(repoPath: string, sha: string): string {
  return key(repoPath, sha, null);
}
