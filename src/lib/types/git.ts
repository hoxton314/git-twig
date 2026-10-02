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
  // Branch list / remotes
  /** Remote name for remote-tracking branches (may contain "/"). */
  remote_name: string | null;
  /** Branch name without the remote prefix (equals `name` for local branches). */
  short_name: string;
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
  // Diff viewer: syntax highlighting
  syntax_highlighting: boolean;
  // Hosting integrations
  github_https_auth: boolean;
  github_host: string;
  github_api_url: string;
  gitlab_base_url: string;
  gitea_base_url: string;
  // App shell: fonts & updater
  ui_font_family: string;
  mono_font_family: string;
  check_updates_on_startup: boolean;
  skipped_update_version: string | null;
  accent_color: string;
  font_size: number;
  diff_font_size: number;
  // Editor & Diff
  diff_view_mode: "unified" | "split";
  tab_size: number;
  show_whitespace_changes: boolean;
  word_wrap_in_diffs: boolean;
  // Commit graph view
  graph_show_author: boolean;
  graph_show_date: boolean;
  graph_show_sha: boolean;
  graph_author_width: number;
  graph_sha_width: number;
  graph_date_width: number;
  graph_date_format: GraphDateFormat;
  graph_row_density: GraphRowDensity;
  graph_hide_remotes: boolean;
  graph_current_branch_only: boolean;
  context_lines: number;
  external_diff_tool: string | null;
  external_merge_tool: string | null;
  // Keybindings
  keybinding_overrides: Record<string, string>;
  // Staging panel
  staging_tree_view: boolean;
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

// ── Commit operations, undo history (reflog), tags ───────────────────

export type ResetMode = "soft" | "mixed" | "hard" | "keep";

export interface CommitOpResult {
  success: boolean;
  message: string;
  /** Cherry-pick/revert stopped on conflicts; repo is left mid-operation. */
  conflicted: boolean;
  previous_head: string | null;
  previous_branch: string | null;
  /** Stash commit created to keep uncommitted changes, if any. */
  stash_oid: string | null;
}

export interface ReflogEntry {
  index: number;
  old_oid: string;
  new_oid: string;
  short_new_oid: string;
  action: string;
  message: string;
  timestamp: number;
  commit_summary: string | null;
  checkout_branch: string | null;
}

export interface TagInfo {
  name: string;
  target_oid: string | null;
  short_target_oid: string | null;
  annotated: boolean;
  message: string | null;
  tagger_name: string | null;
  timestamp: number;
  commit_summary: string | null;
}

// ── Staging panel: commit helpers & file actions ─────────────────────

export interface HeadCommitInfo {
  oid: string;
  /** Full commit message (summary + body). */
  message: string;
  /** HEAD is already contained in a remote-tracking branch. */
  pushed: boolean;
  pushed_to: string | null;
}

export interface AuthorInfo {
  name: string;
  email: string;
  count: number;
}

export type IgnoreKind = "path" | "extension" | "folder";

export interface IgnoreResult {
  pattern: string;
  already_present: boolean;
  tracked: boolean;
}

// ── Branch list / remotes ────────────────────────────────────────────

export interface RemoteInfo {
  name: string;
  fetch_url: string | null;
  /** Effective push URL (dedicated pushurl, else the fetch URL). */
  push_url: string | null;
  has_separate_push_url: boolean;
}

export interface ComparedCommit {
  oid: string;
  short_oid: string;
  summary: string;
  author_name: string;
  timestamp: number;
}

export interface BranchComparison {
  base: string;
  other: string;
  /** Commits in `other` that `base` lacks (newest first, capped). */
  ahead: ComparedCommit[];
  /** Commits in `base` that `other` lacks (newest first, capped). */
  behind: ComparedCommit[];
  ahead_count: number;
  behind_count: number;
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
  /** The new base already has this change (same patch id). */
  already_upstream: boolean;
}

export interface RebaseCommitList {
  commits: RebaseCommit[];
  merges_skipped: number;
  base_oid: string | null;
  /** Base is not an ancestor of HEAD: an unchanged todo still moves the branch. */
  onto_new_base: boolean;
}

export type RebaseAction = "pick" | "reword" | "edit" | "squash" | "fixup" | "drop";

export interface RebaseTodoItem {
  oid: string;
  action: RebaseAction;
  message: string | null;
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

// ── File history & blame ─────────────────────────────────────────────

export interface FileHistoryEntry {
  commit: CommitInfo;
  /** Path of the file in this commit. */
  path: string;
  /** Previous path when this commit renamed the file. */
  old_path: string | null;
  status: "added" | "modified" | "deleted" | "renamed";
}

export interface FileHistoryPage {
  entries: FileHistoryEntry[];
  has_more: boolean;
}

export interface BlameHunk {
  oid: string;
  short_oid: string;
  author_name: string;
  author_email: string;
  timestamp: number;
  summary: string;
  /** 1-based first line in the blamed file. */
  start_line: number;
  line_count: number;
  orig_path: string;
  orig_start_line: number;
  is_boundary: boolean;
  has_parent: boolean;
}

export interface BlameResult {
  path: string;
  rev_oid: string;
  rev_short: string;
  lines: string[];
  hunks: BlameHunk[];
}

// ── Stash extras ─────────────────────────────────────────────────────

export interface StashDetail {
  index: number;
  reference: string;
  oid: string;
  message: string;
  timestamp: string;
  has_untracked: boolean;
}

export interface StashDiff {
  tracked: DiffFile[];
  untracked: DiffFile[];
}

// ── Submodules ───────────────────────────────────────────────────────

export interface SubmoduleInfo {
  name: string;
  path: string;
  abs_path: string;
  url: string | null;
  branch: string | null;
  head_oid: string | null;
  workdir_oid: string | null;
  status: "uninitialized" | "out_of_date" | "dirty" | "up_to_date";
}

// ── Worktrees ────────────────────────────────────────────────────────

export interface WorktreeInfo {
  name: string | null;
  path: string;
  is_main: boolean;
  is_current: boolean;
  branch: string | null;
  head_short: string | null;
  is_locked: boolean;
  is_prunable: boolean;
}

// ── App shell (status bar, recent repos) ─────────────────────────────

/** In-progress repository operation reported by git2 `repo.state()`. */
export type RepoStateKind =
  | "clean"
  | "merge"
  | "revert"
  | "cherry-pick"
  | "bisect"
  | "rebase"
  | "rebase-interactive"
  | "rebase-merge"
  | "apply-mailbox";

export interface RepoStatusSummary {
  branch: string | null;
  detached: boolean;
  unborn: boolean;
  head_short_oid: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  state: RepoStateKind;
}

export interface RecentRepo {
  path: string;
  name: string;
  /** Unix time in milliseconds. */
  last_opened: number;
}

export interface RepoHistory {
  recent: RecentRepo[];
  favorites: string[];
}

// ── Diff viewer: read options & partial staging ──────────────────────

/** Mirrors `reader::DiffReadOptions`; omitted fields use git defaults. */
export interface DiffReadOptions {
  context_lines?: number | null;
  ignore_whitespace?: boolean | null;
}

export type DiffArea = "staged" | "unstaged";
export type HunkAction = "stage" | "unstage" | "discard";

/** Mirrors `hunks::SelectedLine`: a changed line exactly as displayed. */
export interface SelectedLine {
  origin: string;
  old_lineno: number | null;
  new_lineno: number | null;
  content: string;
}

/** Mirrors `hunks::HunkRange`. */
export interface HunkRange {
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
}
