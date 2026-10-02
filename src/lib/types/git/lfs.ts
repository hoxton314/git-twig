/** Git LFS IPC types. Re-exported by `types/git.ts`. */

export interface LfsPattern {
  pattern: string;
  /** The `.gitattributes` file it comes from. */
  source: string;
  lockable: boolean;
}

export interface LfsStatus {
  /** `git lfs version`, or null when git-lfs isn't installed. */
  version: string | null;
  patterns: LfsPattern[];
}

export interface LfsLock {
  id: string;
  path: string;
  owner: string;
  locked_at: string;
  /** Held by the current user. */
  ours: boolean;
}
