/**
 * Global registry of long-running git operations (fetch/pull/push, ...) so
 * the status bar can show what is in flight, plus per-repo last-fetch info.
 *
 * Usage: `await trackOperation(path, "fetch", "Fetching…", () => tauri.fetchAll(path))`.
 * Results of type `CommandResult` with `success: false` are recorded as failures.
 */
import { writable, derived } from "svelte/store";

export type OperationKind = "fetch" | "pull" | "push" | "checkout" | "clone" | (string & {});

export interface RunningOperation {
  id: number;
  repoPath: string;
  kind: OperationKind;
  /** Human-readable progress text, e.g. "Pushing main to origin…". */
  label: string;
  startedAt: number;
  /** True for background work (auto-fetch); shown more quietly. */
  background: boolean;
}

export interface FetchRecord {
  /** Unix ms of the last completed fetch attempt. */
  at: number;
  ok: boolean;
  error?: string;
}

export const operations = writable<RunningOperation[]>([]);

/** Repo path -> last fetch (or pull, which fetches) result. */
export const lastFetch = writable<Record<string, FetchRecord>>({});

/** Operations grouped by repo path. */
export const operationsByRepo = derived(operations, ($ops) => {
  const map = new Map<string, RunningOperation[]>();
  for (const op of $ops) map.set(op.repoPath, [...(map.get(op.repoPath) ?? []), op]);
  return map;
});

let nextId = 1;

/** Register an operation manually; call the returned function when it ends. */
export function beginOperation(
  repoPath: string,
  kind: OperationKind,
  label: string,
  background = false,
): () => void {
  const id = nextId++;
  operations.update((list) => [...list, { id, repoPath, kind, label, startedAt: Date.now(), background }]);
  return () => operations.update((list) => list.filter((op) => op.id !== id));
}

function failureOf(result: unknown): string | null {
  if (result && typeof result === "object" && "success" in result) {
    const r = result as { success: boolean; message?: string };
    if (!r.success) return r.message ?? "failed";
  }
  return null;
}

/** Record a fetch outcome (used by trackOperation and directly by callers). */
export function recordFetch(repoPath: string, ok: boolean, error?: string) {
  lastFetch.update((m) => ({ ...m, [repoPath]: { at: Date.now(), ok, error } }));
}

/**
 * Run `fn` while showing it as a running operation. Fetches and pulls also
 * update `lastFetch`. Errors are re-thrown to the caller unchanged.
 */
export async function trackOperation<T>(
  repoPath: string,
  kind: OperationKind,
  label: string,
  fn: () => Promise<T>,
  opts: { background?: boolean } = {},
): Promise<T> {
  const end = beginOperation(repoPath, kind, label, opts.background ?? false);
  const recordsFetch = kind === "fetch" || kind === "pull";
  try {
    const result = await fn();
    if (recordsFetch) {
      const failure = failureOf(result);
      recordFetch(repoPath, failure === null, failure ?? undefined);
    }
    return result;
  } catch (err) {
    if (recordsFetch) recordFetch(repoPath, false, err instanceof Error ? err.message : String(err));
    throw err;
  } finally {
    end();
  }
}

/** A fetch or pull is already running in `repoPath` (so another would collide on ref locks). */
export function isSyncing(ops: RunningOperation[], repoPath: string): boolean {
  return ops.some((op) => op.repoPath === repoPath && (op.kind === "fetch" || op.kind === "pull"));
}
