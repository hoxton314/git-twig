/** Submodule and worktree types. Re-exported by `types/git.ts`. */

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
