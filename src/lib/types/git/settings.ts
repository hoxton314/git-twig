/** App settings and git config types (mirror `commands/settings.rs`). Re-exported by `types/git.ts`. */
import type { GraphDateFormat, GraphRowDensity } from "./graph";

// ── App Settings ─────────────────────────────────────────────────

export interface AppSettings {
  // General
  default_repo_dir: string | null;
  auto_fetch_interval: number;
  max_commits: number;
  confirm_destructive_ops: boolean;
  restore_tabs_on_startup: boolean;
  // Appearance
  theme: "dark" | "light";
  // Diff viewer: syntax highlighting
  syntax_highlighting: boolean;
  /** System notification for new upstream commits after auto-fetch. */
  notify_new_commits: "off" | "current" | "all";
  /** System notification when CI finishes on an open repository's HEAD. */
  notify_ci: "off" | "failures" | "all";
  // Hosting integrations
  github_https_auth: boolean;
  github_host: string;
  github_api_url: string;
  gitlab_base_url: string;
  gitea_base_url: string;
  // App shell: fonts & updater
  ui_font_family: string;
  mono_font_family: string;
  check_updates_on_startup: boolean;
  skipped_update_version: string | null;
  accent_color: string;
  font_size: number;
  diff_font_size: number;
  // Editor & Diff
  diff_view_mode: "unified" | "split";
  tab_size: number;
  show_whitespace_changes: boolean;
  word_wrap_in_diffs: boolean;
  // Commit graph view
  graph_show_author: boolean;
  graph_show_date: boolean;
  graph_show_sha: boolean;
  graph_author_width: number;
  graph_sha_width: number;
  graph_date_width: number;
  graph_date_format: GraphDateFormat;
  graph_row_density: GraphRowDensity;
  graph_hide_remotes: boolean;
  graph_current_branch_only: boolean;
  context_lines: number;
  external_diff_tool: string | null;
  external_merge_tool: string | null;
  /** "Open in terminal" command (null = platform default). */
  terminal_command: string | null;
  /** "Open in editor" command; `{path}` is replaced, else appended (null = default). */
  editor_command: string | null;
  // Keybindings
  keybinding_overrides: Record<string, string>;
  // Staging panel
  staging_tree_view: boolean;
}

// ── Git Config ───────────────────────────────────────────────────

export interface GitConfig {
  user_name: string;
  user_email: string;
  /** "false" = merge, "true" = rebase, "ff-only" = fast-forward only */
  pull_rebase: "false" | "true" | "ff-only";
  fetch_prune: boolean;
  gpg_sign: boolean;
  signing_key: string;
  /** `gpg.format`: "openpgp" (gpg), "ssh", or "x509" (gpgsm; kept as-is). */
  gpg_format: string;
  lfs_installed: boolean;
}

/** A key usable for commit signing (`value` goes into `user.signingkey`). */
export interface SigningKey {
  value: string;
  label: string;
}
