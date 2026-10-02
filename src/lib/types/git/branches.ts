/** Branch, upstream and remote types. Re-exported by `types/git.ts`. */

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
