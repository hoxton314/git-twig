/** Stash types. Re-exported by `types/git.ts`. */
import type { DiffFile } from "./diff";

// ── Stash ────────────────────────────────────────────────────────────

export interface StashEntry {
  index: number;
  reference: string;
  message: string;
  timestamp: string;
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
