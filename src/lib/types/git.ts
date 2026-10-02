// ── Repository ────────────────────────────────────────────────────────

export interface RepoInfo {
  path: string;
  name: string;
  head_name: string | null;
  is_bare: boolean;
  is_empty: boolean;
  last_commit_time: number;
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

export interface RefLabel {
  name: string;
  ref_type: "local" | "remote" | "tag";
}

export interface CommitGraph {
  entries: GraphEntry[];
  total_lanes: number;
  refs: Record<string, RefLabel[]>;
  unpushed_oids: string[];
}

// ── Branches ──────────────────────────────────────────────────────────

export interface BranchInfo {
  name: string;
  is_remote: boolean;
  is_head: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
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

// ── Session persistence ──────────────────────────────────────────────

export interface Session {
  paths: string[];
  active: string | null;
  sidebar_width: number | null;
  staging_width: number | null;
  diff_panel_ratio: number | null;
}

// ── Stash ────────────────────────────────────────────────────────────

export interface StashEntry {
  index: number;
  reference: string;
  message: string;
  timestamp: string;
}

// ── App Settings ─────────────────────────────────────────────────

export interface AppSettings {
  // General
  default_repo_dir: string | null;
  auto_fetch_interval: number;
  max_commits: number;
  confirm_destructive_ops: boolean;
  restore_tabs_on_startup: boolean;
  // Appearance
  theme: "dark" | "light";
  accent_color: string;
  font_size: number;
  diff_font_size: number;
  // Editor & Diff
  diff_view_mode: "unified" | "split";
  tab_size: number;
  show_whitespace_changes: boolean;
  word_wrap_in_diffs: boolean;
  context_lines: number;
  external_diff_tool: string | null;
  external_merge_tool: string | null;
  // Keybindings
  keybinding_overrides: Record<string, string>;
}

// ── Git Config ───────────────────────────────────────────────────

export interface GitConfig {
  user_name: string;
  user_email: string;
  /** "false" = merge, "true" = rebase, "ff-only" = fast-forward only */
  pull_rebase: "false" | "true" | "ff-only";
  fetch_prune: boolean;
  gpg_sign: boolean;
  signing_key: string;
  lfs_installed: boolean;
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

// ── Conflict resolution & history rewriting (rebase, force push) ────

export type OperationKind =
  | "none"
  | "merge"
  | "rebase"
  | "cherry_pick"
  | "revert"
  | "am"
  | "bisect";

export interface ConflictFile {
  path: string;
  kind:
    | "both_modified"
    | "both_added"
    | "deleted_by_us"
    | "deleted_by_them"
    | "added_by_us"
    | "added_by_them"
    | "both_deleted";
  has_base: boolean;
  has_ours: boolean;
  has_theirs: boolean;
}

export interface RepoOperationState {
  kind: OperationKind;
  interactive: boolean;
  conflicts: ConflictFile[];
  message: string | null;
  head_name: string | null;
  onto: string | null;
  step: number | null;
  total: number | null;
  current_commit: string | null;
  current_subject: string | null;
  stopped_for_edit: boolean;
  can_skip: boolean;
  ours_label: string;
  theirs_label: string;
}

export interface ConflictVersions {
  path: string;
  base: string | null;
  ours: string | null;
  theirs: string | null;
  merged: string | null;
  is_binary: boolean;
  too_large: boolean;
}

export interface RebaseCommit {
  oid: string;
  short_oid: string;
  summary: string;
  message: string;
  author_name: string;
  timestamp: number;
}

export interface RebaseCommitList {
  commits: RebaseCommit[];
  merges_skipped: number;
  base_oid: string | null;
}

export type RebaseAction = "pick" | "reword" | "edit" | "squash" | "fixup" | "drop";

export interface RebaseTodoItem {
  oid: string;
  action: RebaseAction;
  message: string | null;
}
