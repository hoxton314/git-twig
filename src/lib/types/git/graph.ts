/** Commit graph, lanes, search and locate types. Re-exported by `types/git.ts`. */

// ── Commit Graph ──────────────────────────────────────────────────────

export interface CommitInfo {
  oid: string;
  short_oid: string;
  summary: string;
  body: string;
  author_name: string;
  author_email: string;
  author_gravatar: string;
  timestamp: number;
  parent_oids: string[];
}

export interface GraphEntry {
  commit: CommitInfo;
  lane: number;
  has_incoming: boolean;
  /** Lane indices with pass-through lines (straight vertical) */
  rails: number[];
  /** Lane index for each parent (lines from node downward) */
  parent_lanes: number[];
  /** Extra lanes whose line enters from above and converges into this node */
  merge_ins: number[];
}

export interface RefLabel {
  name: string;
  ref_type: "local" | "remote" | "tag";
}

export interface CommitGraph {
  entries: GraphEntry[];
  total_lanes: number;
  refs: Record<string, RefLabel[]>;
  unpushed_oids: string[];
  /** Index of entries[0] in the full graph order (pagination offset). */
  offset: number;
  /** More commits exist after this page. */
  has_more: boolean;
  /** Signature of the walked branch tips; pages only line up while it matches. */
  tips: string;
}

// ── Lane colors ───────────────────────────────────────────────────────

export const LANE_COLORS = [
  "#7aa2f7",
  "#9ece6a",
  "#e0af68",
  "#f7768e",
  "#bb9af7",
  "#2ac3de",
] as const;

export function laneColor(lane: number): string {
  return LANE_COLORS[lane % LANE_COLORS.length];
}

// ── Commit graph: view options, search, locate ───────────────────────

export type GraphDateFormat = "relative" | "iso" | "locale";
export type GraphRowDensity = "compact" | "normal" | "comfortable";

/** Which refs the graph walks (mirrors Rust `GraphOptions`). */
export interface GraphOptions {
  hide_remotes: boolean;
  current_branch_only: boolean;
}

export interface SearchMatch {
  /** Row of the commit in the unfiltered graph. */
  index: number;
  commit: CommitInfo;
}

export interface CommitSearchResult {
  matches: SearchMatch[];
  /** Cut off at the result limit. */
  truncated: boolean;
  /** Number of commits examined. */
  scanned: number;
  tips: string;
}

export interface LocatedCommit {
  oid: string;
  /** Graph row, or null when the commit is hidden by the view options. */
  index: number | null;
  tips: string;
}
