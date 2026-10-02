/**
 * Per-repository settings overrides: which settings a repository may
 * override, and how the effective settings are computed. Pure (no stores),
 * so it can be tested and shared.
 */
import type { AppSettings } from "./types/git";

/** Settings a repository can override (all are read by the frontend). */
export const OVERRIDABLE_KEYS = [
  "auto_fetch_interval",
  "max_commits",
  "graph_hide_remotes",
  "graph_current_branch_only",
  "tab_size",
  "context_lines",
  "show_whitespace_changes",
  "word_wrap_in_diffs",
  "syntax_highlighting",
  "staging_tree_view",
  "external_diff_tool",
  "external_merge_tool",
] as const satisfies readonly (keyof AppSettings)[];

export type OverridableKey = (typeof OVERRIDABLE_KEYS)[number];
export type RepoOverride = Partial<Pick<AppSettings, OverridableKey>>;
/** Repository path → its overrides. */
export type RepoOverrides = Record<string, RepoOverride>;

const ALLOWED = new Set<string>(OVERRIDABLE_KEYS);
/** Overridable keys whose value may be null ("not set"). */
const NULLABLE = new Set<string>(["external_diff_tool", "external_merge_tool"]);

export function isOverridable(key: string): key is OverridableKey {
  return ALLOWED.has(key);
}

/** Keep only known keys whose value has the global value's type. */
export function sanitizeOverride(raw: unknown, global: AppSettings): RepoOverride {
  const out: Record<string, unknown> = {};
  if (!raw || typeof raw !== "object") return out;
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (!isOverridable(k)) continue;
    const g = global[k];
    const ok = NULLABLE.has(k) ? v === null || typeof v === "string" : v !== null && typeof v === typeof g;
    if (ok) out[k] = v;
  }
  return out as RepoOverride;
}

/** Global settings with `path`'s overrides applied. */
export function effectiveSettings(global: AppSettings, overrides: RepoOverrides, path: string | null): AppSettings {
  const o = path ? overrides[path] : undefined;
  return o && Object.keys(o).length > 0 ? { ...global, ...o } : global;
}

/**
 * Split a settings patch made while `path` is active: keys the repository
 * overrides update its override, the rest update the global settings.
 */
export function splitPatch(
  patch: Partial<AppSettings>,
  overrides: RepoOverrides,
  path: string | null,
): { global: Partial<AppSettings>; repo: RepoOverride } {
  const o = path ? overrides[path] : undefined;
  const global: Record<string, unknown> = {};
  const repo: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(patch)) {
    if (o && k in o && isOverridable(k)) repo[k] = v;
    else global[k] = v;
  }
  return { global: global as Partial<AppSettings>, repo: repo as RepoOverride };
}
