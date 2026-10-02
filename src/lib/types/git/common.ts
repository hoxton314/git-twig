/** Shared IPC result types. Re-exported by `types/git.ts`. */

// ── Command results ───────────────────────────────────────────────────

export interface CommandResult {
  success: boolean;
  message: string;
}

/** A commit from the staging panel, with what its hooks printed. */
export interface CommitResult extends CommandResult {
  /** Hooks that ran (pre-commit, commit-msg, …). */
  hooks: string[];
  /** Their combined output, when non-empty. */
  hook_output: string | null;
}
