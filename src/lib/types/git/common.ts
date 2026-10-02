/** Shared IPC result types. Re-exported by `types/git.ts`. */

// ── Command results ───────────────────────────────────────────────────

export interface CommandResult {
  success: boolean;
  message: string;
}

/** A commit from the staging panel, with what its hooks printed. */
export interface CommitResult extends CommandResult {
  /** Executable commit hooks installed for this commit (pre-commit, …). */
  hooks: string[];
  /** On success: what hooks (and git) printed, when anything. */
  hook_output: string | null;
}
