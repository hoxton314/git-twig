/**
 * Commit signature status for the graph badge, verified lazily for the rows
 * that are actually shown.
 *
 *   const sig = signatureFor(repoPath, oid);  // Readable<SignatureInfo | null>
 *
 * Requests from all rows rendered in the same tick are batched into one
 * `commit_signatures` call per repository. Results (including "unsigned")
 * are cached by repo + oid: a commit's signature never changes.
 */
import { derived, writable, type Readable } from "svelte/store";
import type { SignatureInfo } from "../types/git";
import * as tauri from "../tauri";

const BATCH_MS = 30;
/** Matches the backend's per-call limit. */
const MAX_BATCH = 200;

const cache = writable<Map<string, SignatureInfo | "unsigned">>(new Map());
const requested = new Set<string>();
const queues = new Map<string, Set<string>>();
let timer: ReturnType<typeof setTimeout> | null = null;

const key = (repo: string, oid: string) => `${repo}\u0000${oid}`;

async function flush() {
  timer = null;
  const work = [...queues.entries()];
  queues.clear();
  for (const [repo, set] of work) {
    const oids = [...set];
    for (let i = 0; i < oids.length; i += MAX_BATCH) {
      const chunk = oids.slice(i, i + MAX_BATCH);
      let found: SignatureInfo[] = [];
      try {
        found = await tauri.commitSignatures(repo, chunk);
      } catch {
        // Verification unavailable: show nothing rather than retrying forever.
      }
      const byOid = new Map(found.map((s) => [s.oid, s]));
      cache.update((m) => {
        const next = new Map(m);
        for (const oid of chunk) next.set(key(repo, oid), byOid.get(oid) ?? "unsigned");
        return next;
      });
    }
  }
}

function request(repo: string, oid: string) {
  const k = key(repo, oid);
  if (requested.has(k)) return;
  requested.add(k);
  let q = queues.get(repo);
  if (!q) queues.set(repo, (q = new Set()));
  q.add(oid);
  if (!timer) timer = setTimeout(() => void flush(), BATCH_MS);
}

/** Signature of `oid` in `repo` (`null` while loading or when unsigned). */
export function signatureFor(repo: string | null, oid: string): Readable<SignatureInfo | null> {
  if (repo) request(repo, oid);
  return derived(cache, (m) => {
    const v = repo ? m.get(key(repo, oid)) : undefined;
    return v && v !== "unsigned" ? v : null;
  });
}

/** Forget cached results (e.g. after the user changed trust settings). */
export function clearSignatureCache() {
  requested.clear();
  cache.set(new Map());
}
