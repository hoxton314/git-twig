import { writable, get } from "svelte/store";
import type { AppSettings } from "../types/git";
import * as tauri from "../tauri";

const defaults: AppSettings = {
  default_repo_dir: null,
  auto_fetch_interval: 0,
  max_commits: 5000,
  confirm_destructive_ops: true,
  restore_tabs_on_startup: true,
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
};

export const settings = writable<AppSettings>({ ...defaults });

let saveTimeout: ReturnType<typeof setTimeout> | null = null;
let loaded = false;

/** Load settings from disk. Call once at startup. */
export async function loadSettings() {
  try {
    const s = await tauri.loadSettings();
    settings.set(s);
  } catch {
    settings.set({ ...defaults });
  }
  loaded = true;
}

/** Persist current settings to disk (debounced). */
function persistSettings() {
  if (!loaded) return;
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(() => {
    tauri.saveSettings(get(settings)).catch((e) => {
      console.error("Failed to save settings:", e);
    });
  }, 300);
}

settings.subscribe(() => persistSettings());

/** Update one or more settings fields and auto-save. */
export function updateSettings(patch: Partial<AppSettings>) {
  settings.update((s) => ({ ...s, ...patch }));
}
