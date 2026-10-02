/** Commit ops, reflog, tags, conflicts, rebase, file history and blame types. Re-exported by `types/git.ts`. */
import type { CommitInfo } from "./graph";

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

/** Result of cherry-picking several commits. */
export interface PickManyResult extends CommitOpResult {
  /** Commits applied before stopping (all of them on success). */
  picked: number;
  /** Commits left out because HEAD already contains them. */
  skipped: string[];
  /** Stopped on a commit whose changes HEAD already has (not a conflict). */
  empty: boolean;
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
  /** Some version is not UTF-8; no text is sent (editing would corrupt it). */
  not_utf8: boolean;
  /** The path is a symbolic link on some side. */
  is_symlink: boolean;
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

/** Summary of a patch file before applying it. */
export interface PatchInfo {
  /** "mbox" (format-patch, applied with git am) or "diff". */
  kind: "mbox" | "diff";
  /** Commit subjects in an mbox, in order. */
  commits: string[];
  /** `git apply --stat` summary. */
  stat: string;
  /** Plain diffs: whether it applies cleanly (null for mbox). */
  applies_cleanly: boolean | null;
  check_error: string | null;
}

export interface ApplyPatchResult {
  success: boolean;
  message: string;
  mode: "am" | "apply" | "apply-3way";
  /** git am stopped (banner), or a three-way apply left conflicts. */
  conflicted: boolean;
}

/** A validated "squash these commits" plan. */
export interface SquashPlan {
  count: number;
  /** Parent of the oldest squashed commit (null when it is the root). */
  base: string | null;
  /** Oldest squashed commit. */
  first: string;
  /** The squashed commits' messages, oldest first. */
  message: string;
  /** Newer commits rebased on top. */
  later: number;
  /** Remote-tracking branches that already contain the commits. */
  pushed_to: string[];
  /** The remote check stopped early; `pushed_to` may be incomplete. */
  pushed_unknown: boolean;
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
