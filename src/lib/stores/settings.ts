import { writable, get } from "svelte/store";
import type { AppSettings } from "../types/git";
import { setOverrides } from "../keybindings";
import { diffViewMode } from "./ui";
import * as tauri from "../tauri";

const defaults: AppSettings = {
  default_repo_dir: null,
  auto_fetch_interval: 0,
  max_commits: 5000,
  confirm_destructive_ops: true,
  restore_tabs_on_startup: true,
  theme: "dark",
  accent_color: "#7aa2f7",
  font_size: 13,
  diff_font_size: 13,
  diff_view_mode: "unified",
  tab_size: 4,
  show_whitespace_changes: true,
  word_wrap_in_diffs: false,
  context_lines: 3,
  external_diff_tool: null,
  external_merge_tool: null,
  terminal_command: null,
  editor_command: null,
  keybinding_overrides: {},
  staging_tree_view: false,
  // Commit graph view
  graph_show_author: true,
  graph_show_date: true,
  graph_show_sha: true,
  graph_author_width: 120,
  graph_sha_width: 64,
  graph_date_width: 90,
  graph_date_format: "relative",
  graph_row_density: "normal",
  graph_hide_remotes: false,
  graph_current_branch_only: false,
  // App shell: fonts & updater
  ui_font_family: "",
  mono_font_family: "",
  check_updates_on_startup: true,
  skipped_update_version: null,
  // Hosting integrations
  github_https_auth: true,
  github_host: "github.com",
  github_api_url: "",
  gitlab_base_url: "https://gitlab.com",
  gitea_base_url: "",
  // Diff viewer: syntax highlighting
  syntax_highlighting: true,
};

/** Default values for every setting (used by reset/import). */
export const DEFAULT_SETTINGS: Readonly<AppSettings> = defaults;

export const settings = writable<AppSettings>({ ...defaults });

/** True once settings have been loaded from disk (or defaults applied). */
export const settingsReady = writable(false);

let saveTimeout: ReturnType<typeof setTimeout> | null = null;
let loaded = false;

/** Load settings from disk. Call once at startup. */
export async function loadSettings() {
  try {
    const s = await tauri.loadSettings();
    // Merge over defaults so fields missing from older backends/files are filled.
    settings.set({ ...defaults, ...s, keybinding_overrides: s.keybinding_overrides ?? {} });
  } catch (e) {
    console.error("Failed to load settings, using defaults:", e);
    settings.set({ ...defaults });
  }
  // Apply the persisted default diff view at startup.
  diffViewMode.set(get(settings).diff_view_mode);
  loaded = true;
  settingsReady.set(true);
}

/** Persist current settings to disk (debounced). */
function persistSettings() {
  if (!loaded) return;
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(() => {
    saveTimeout = null;
    tauri.saveSettings(get(settings)).catch((e) => {
      console.error("Failed to save settings:", e);
    });
  }, 300);
}

/**
 * Write any pending (debounced) settings to disk immediately. Use before
 * invoking backend commands that read settings.json themselves (e.g. GitHub).
 */
export async function flushSettings(): Promise<void> {
  if (saveTimeout) {
    clearTimeout(saveTimeout);
    saveTimeout = null;
  }
  if (!loaded) return;
  await tauri.saveSettings(get(settings));
}

/** Apply visual settings to CSS custom properties. */
function applyVisualSettings(s: AppSettings) {
  const root = document.documentElement;
  root.setAttribute("data-theme", s.theme);
  root.style.setProperty("--color-accent", s.accent_color);
  root.style.setProperty("font-size", `${s.font_size}px`);
  root.style.setProperty("--diff-font-size", `${s.diff_font_size}px`);
  applyFontFamily(root, "--font-sans", s.ui_font_family, UI_FONT_FALLBACK);
  applyFontFamily(root, "--font-mono", s.mono_font_family, MONO_FONT_FALLBACK);
}

// ── Font families ────────────────────────────────────────────────────

/** Fallback stacks appended after a user-chosen font (mirror app.css). */
export const UI_FONT_FALLBACK =
  'ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
export const MONO_FONT_FALLBACK =
  'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace';

const GENERIC_FAMILIES = new Set([
  "serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui",
  "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded", "math", "emoji",
]);

/**
 * Turn a user-entered family list ("JetBrains Mono, Fira Code") into a safe
 * CSS font-family value: each name is quoted unless it is a generic family,
 * and characters that could escape the declaration are dropped.
 */
export function cssFontFamily(input: string): string {
  return input
    .split(",")
    .map((name) => name.replace(/[;{}<>\\"']/g, "").trim())
    .filter(Boolean)
    .map((name) => (GENERIC_FAMILIES.has(name.toLowerCase()) ? name.toLowerCase() : `"${name}"`))
    .join(", ");
}

function applyFontFamily(root: HTMLElement, prop: string, value: string | undefined, fallback: string) {
  const family = cssFontFamily(value ?? "");
  if (family) root.style.setProperty(prop, `${family}, ${fallback}`);
  else root.style.removeProperty(prop);
}

settings.subscribe((s) => {
  persistSettings();
  applyVisualSettings(s);
  setOverrides(s.keybinding_overrides ?? {});
  // Diff viewer: context lines / whitespace apply to every diff read.
  tauri.setDiffReadDefaults({
    context_lines: s.context_lines,
    ignore_whitespace: !s.show_whitespace_changes,
  });
});

/** Update one or more settings fields and auto-save. */
export function updateSettings(patch: Partial<AppSettings>) {
  settings.update((s) => ({ ...s, ...patch }));
}
