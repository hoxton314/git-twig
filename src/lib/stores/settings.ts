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
  show_whitespace_changes: false,
  word_wrap_in_diffs: false,
  context_lines: 3,
  external_diff_tool: null,
  external_merge_tool: null,
  keybinding_overrides: {},
  staging_tree_view: false,
};

export const settings = writable<AppSettings>({ ...defaults });

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
}

settings.subscribe((s) => {
  persistSettings();
  applyVisualSettings(s);
  setOverrides(s.keybinding_overrides ?? {});
});

/** Update one or more settings fields and auto-save. */
export function updateSettings(patch: Partial<AppSettings>) {
  settings.update((s) => ({ ...s, ...patch }));
}
