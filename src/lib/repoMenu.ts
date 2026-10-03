/**
 * Entries and filtering for the tab bar "+" repository menu: favorites &
 * recent repositories plus the repositories found in the default folder,
 * fuzzy-filtered as one list (the command palette's matcher).
 */
import { fuzzyMatch } from "./palette";

export type RepoMenuKind = "favorite" | "recent" | "scanned" | "group";

/** `path` of a group entry; selecting it opens the whole group. */
export const GROUP_PREFIX = "group:";

export interface RepoMenuEntry {
  path: string;
  name: string;
  branch: string | null;
  kind: RepoMenuKind;
  /** Number of repositories in a group entry. */
  count?: number;
  /** Already open as a tab (selecting switches to it). */
  open: boolean;
  /** Folder no longer exists: shown dimmed, never selectable. */
  missing: boolean;
}

export interface RepoMenuRow extends RepoMenuEntry {
  /** Indices into `name` to highlight. */
  nameHits: number[];
  score: number;
}

export interface RepoMenuSection {
  title: string;
  rows: RepoMenuRow[];
}

/**
 * Groups first, then favorites and recent (not open as tabs), then scanned
 * repos. A path appears once.
 */
export function buildEntries(
  quick: { path: string; name: string; favorite: boolean; missing: boolean }[],
  scanned: { path: string; name: string; head_name: string | null }[],
  openPaths: Set<string>,
  groups: { id: string; name: string; paths: string[] }[] = [],
): RepoMenuEntry[] {
  const seen = new Set<string>();
  const out: RepoMenuEntry[] = groups.map((g) => ({
    path: `${GROUP_PREFIX}${g.id}`,
    name: g.name,
    branch: null,
    count: g.paths.length,
    kind: "group",
    open: g.paths.length > 0 && g.paths.every((p) => openPaths.has(p)),
    missing: g.paths.length === 0,
  }));
  for (const q of quick) {
    if (seen.has(q.path)) continue;
    seen.add(q.path);
    out.push({
      path: q.path,
      name: q.name,
      branch: null,
      kind: q.favorite ? "favorite" : "recent",
      open: openPaths.has(q.path),
      missing: q.missing,
    });
  }
  for (const s of scanned) {
    if (seen.has(s.path)) continue;
    seen.add(s.path);
    out.push({ path: s.path, name: s.name, branch: s.head_name, kind: "scanned", open: openPaths.has(s.path), missing: false });
  }
  return out;
}

/**
 * Filter by `query` (fuzzy on the name; substring on branch, then path, with
 * decreasing weight) and
 * group into sections; empty sections are dropped. With no query every
 * entry is kept in its original order.
 */
export function filterEntries(entries: RepoMenuEntry[], query: string): RepoMenuSection[] {
  const q = query.trim();
  const rows: RepoMenuRow[] = [];
  entries.forEach((e, order) => {
    if (!q) {
      rows.push({ ...e, nameHits: [], score: -order });
      return;
    }
    // Names match fuzzily; branch and path only as substrings (subsequence
    // matching on long paths and branch names matches almost anything).
    const onName = fuzzyMatch(q, e.name);
    const lower = q.toLowerCase();
    // Groups have no branch (it holds their repo count) and their path is an id.
    const onBranch = e.kind !== "group" && e.branch?.toLowerCase().includes(lower) ? fuzzyMatch(q, e.branch) : null;
    const onPath = e.kind !== "group" && e.path.toLowerCase().includes(lower) ? fuzzyMatch(q, e.path) : null;
    let best: RepoMenuRow | null = null;
    if (onName) best = { ...e, nameHits: onName.indices, score: onName.score };
    if (onBranch && (!best || onBranch.score - 15 > best.score)) best = { ...e, nameHits: [], score: onBranch.score - 15 };
    if (onPath && (!best || onPath.score - 30 > best.score)) best = { ...e, nameHits: [], score: onPath.score - 30 };
    if (best) rows.push(best);
  });
  const byScore = (a: RepoMenuRow, b: RepoMenuRow) => b.score - a.score;
  const sections: RepoMenuSection[] = [
    { title: "Groups", rows: rows.filter((r) => r.kind === "group").sort(byScore) },
    { title: "Favorites & recent", rows: rows.filter((r) => r.kind === "favorite" || r.kind === "recent").sort(byScore) },
    { title: "Repositories", rows: rows.filter((r) => r.kind === "scanned").sort(byScore) },
  ];
  return sections.filter((s) => s.rows.length > 0);
}

/** Rows in display order (section headers excluded). */
export function flatRows(sections: RepoMenuSection[]): RepoMenuRow[] {
  return sections.flatMap((s) => s.rows);
}

/** Next selectable index from `from` in direction `dir`, wrapping; -1 if none. */
export function stepSelection(rows: RepoMenuRow[], from: number, dir: 1 | -1): number {
  const n = rows.length;
  if (n === 0) return -1;
  for (let k = 1; k <= n; k++) {
    const i = (((from + dir * k) % n) + n) % n;
    if (!rows[i].missing) return i;
  }
  return -1;
}

/** First selectable row, or -1. */
export function firstSelectable(rows: RepoMenuRow[]): number {
  return rows.findIndex((r) => !r.missing);
}

/** Split `text` into plain / highlighted runs for matched `indices`. */
export function highlightRuns(text: string, indices: number[]): { text: string; hit: boolean }[] {
  if (indices.length === 0) return [{ text, hit: false }];
  const set = new Set(indices);
  const out: { text: string; hit: boolean }[] = [];
  for (let i = 0; i < text.length; i++) {
    const hit = set.has(i);
    const last = out[out.length - 1];
    if (last && last.hit === hit) last.text += text[i];
    else out.push({ text: text[i], hit });
  }
  return out;
}
