/** Diff, working-tree status, staging and partial-staging types. Re-exported by `types/git.ts`. */

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
