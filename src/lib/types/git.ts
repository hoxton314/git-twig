// ── Repository ────────────────────────────────────────────────────────

export interface RepoInfo {
  path: string;
  name: string;
  head_name: string | null;
  is_bare: boolean;
  is_empty: boolean;
}

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
}

export interface CommitGraph {
  entries: GraphEntry[];
  total_lanes: number;
}

// ── Branches ──────────────────────────────────────────────────────────

export interface BranchInfo {
  name: string;
  is_remote: boolean;
  is_head: boolean;
  upstream: string | null;
  oid: string;
  short_oid: string;
  last_commit_summary: string;
  last_commit_timestamp: number;
}

// ── Diffs ─────────────────────────────────────────────────────────────

export interface DiffFile {
  old_path: string | null;
  new_path: string | null;
  status: string;
  is_binary: boolean;
  is_lfs: boolean;
  lfs_size: string | null;
  hunks: DiffHunk[];
}

export interface DiffHunk {
  header: string;
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  lines: DiffLine[];
}

export interface DiffLine {
  origin: string;
  old_lineno: number | null;
  new_lineno: number | null;
  content: string;
}

// ── Working directory status ──────────────────────────────────────────

export interface FileStatus {
  path: string;
  status: string;
  is_new: boolean;
}

export interface WorkingStatus {
  staged: FileStatus[];
  unstaged: FileStatus[];
}

// ── Command results ───────────────────────────────────────────────────

export interface CommandResult {
  success: boolean;
  message: string;
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
