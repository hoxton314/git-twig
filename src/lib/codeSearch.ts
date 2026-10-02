/**
 * Code search (`git grep`) panel state and its pure helpers.
 *
 *   openCodeSearch();             // working tree
 *   openCodeSearch(oid, "abc1234"); // a commit
 */
import { writable } from "svelte/store";

/** Open search panel (null = closed). `rev` null searches the working tree. */
export const codeSearch = writable<{ rev: string | null; label: string | null } | null>(null);

export function openCodeSearch(rev: string | null = null, label: string | null = null) {
  codeSearch.set({ rev, label });
}

/** Path filter text → pathspecs: split on commas and whitespace. */
export function parsePaths(text: string): string[] {
  return text
    .split(/[,\s]+/)
    .map((p) => p.trim())
    .filter(Boolean);
}

/**
 * Character ranges to highlight in `text`. Mirrors the search options; a
 * regex JavaScript can't compile (git uses POSIX ERE) just isn't highlighted.
 */
export function matchRanges(
  text: string,
  pattern: string,
  opts: { regex: boolean; ignoreCase: boolean; wholeWord: boolean },
): [number, number][] {
  if (!pattern) return [];
  let source = opts.regex ? pattern : pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  if (opts.wholeWord) source = `(?<![\\w])(?:${source})(?![\\w])`;
  let re: RegExp;
  try {
    re = new RegExp(source, opts.ignoreCase ? "gi" : "g");
  } catch {
    return [];
  }
  const out: [number, number][] = [];
  for (const m of text.matchAll(re)) {
    if (m[0].length === 0) continue;
    out.push([m.index, m.index + m[0].length]);
    if (out.length >= 50) break;
  }
  return out;
}

/** Split `text` into plain / highlighted runs for `ranges` (sorted, non-overlapping). */
export function splitRuns(text: string, ranges: [number, number][]): { text: string; hit: boolean }[] {
  const runs: { text: string; hit: boolean }[] = [];
  let at = 0;
  for (const [s, e] of ranges) {
    if (s > at) runs.push({ text: text.slice(at, s), hit: false });
    runs.push({ text: text.slice(s, e), hit: true });
    at = e;
  }
  if (at < text.length || runs.length === 0) runs.push({ text: text.slice(at), hit: false });
  return runs;
}
