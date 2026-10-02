/** Repository info, session and app-shell types. Re-exported by `types/git.ts`. */

// ── Repository ────────────────────────────────────────────────────────

export interface RepoInfo {
  path: string;
  name: string;
  head_name: string | null;
  is_bare: boolean;
  is_empty: boolean;
  last_commit_time: number;
}

// ── Session persistence ──────────────────────────────────────────────

export interface Session {
  paths: string[];
  active: string | null;
  sidebar_width: number | null;
  staging_width: number | null;
  diff_panel_ratio: number | null;
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
  /** User-defined repository groups, in display order. */
  groups: RepoGroup[];
}

export interface RepoGroup {
  id: string;
  name: string;
  /** Repository paths, in display order. */
  paths: string[];
}

/** A `git clone` progress line (`clone-progress` event). */
export interface CloneProgress {
  op_id: number;
  line: string;
}

/** A row of the repository dashboard. */
export interface RepoStatusRow {
  path: string;
  name: string;
  /** Current branch, or a short id when detached; null when unborn. */
  branch: string | null;
  detached: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
  /** Uncommitted changes (tracked + untracked), capped at 1000. */
  changes: number;
  changes_capped: boolean;
  /** Unix seconds of the last fetch, if ever. */
  last_fetch: number | null;
  /** Why the repository couldn't be read. */
  error: string | null;
}
