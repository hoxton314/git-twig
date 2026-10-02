/** Shared IPC result types. Re-exported by `types/git.ts`. */

// ── Command results ───────────────────────────────────────────────────

export interface CommandResult {
  success: boolean;
  message: string;
}
