/**
 * App-shell actions shared by the tab bar, home screen, settings screens and
 * command palette.
 */
import { get } from "svelte/store";
import { open, save, ask } from "@tauri-apps/plugin-dialog";
import { addRepo } from "./stores/repos";
import { settings, DEFAULT_SETTINGS, flushSettings } from "./stores/settings";
import { toast, toastError } from "./stores/toasts";
import * as tauri from "./tauri";

/** Pick a folder and open it as a repository tab. */
export async function openRepoWithDialog(): Promise<void> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open Git Repository",
    defaultPath: get(settings).default_repo_dir ?? undefined,
  });
  if (!selected || Array.isArray(selected)) return;
  try {
    addRepo(await tauri.openRepo(selected));
  } catch (err) {
    toastError("Open repository failed", err);
  }
}

export async function openSettingsFolder(): Promise<void> {
  try {
    await flushSettings();
    await tauri.openSettingsFolder();
  } catch (err) {
    toastError("Could not open settings folder", err);
  }
}

/** Save the current settings (never secrets) to a user-chosen JSON file. */
export async function exportSettingsToFile(): Promise<void> {
  const path = await save({
    title: "Export Twig Settings",
    defaultPath: "twig-settings.json",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path) return;
  try {
    await tauri.exportSettings(path, get(settings));
    toast("success", `Settings exported to ${path}`);
  } catch (err) {
    toastError("Export failed", err);
  }
}

/** Replace the current settings with ones read from a JSON file. */
export async function importSettingsFromFile(): Promise<void> {
  const path = await open({
    title: "Import Twig Settings",
    multiple: false,
    directory: false,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path || Array.isArray(path)) return;
  try {
    const imported = await tauri.importSettings(path);
    const confirmed = !get(settings).confirm_destructive_ops || await ask("Replace all current settings with the imported ones?", {
      title: "Import Settings",
      kind: "warning",
      okLabel: "Import",
      cancelLabel: "Cancel",
    });
    if (!confirmed) return;
    settings.set({
      ...DEFAULT_SETTINGS,
      ...imported,
      keybinding_overrides: imported.keybinding_overrides ?? {},
    });
    toast("success", "Settings imported");
  } catch (err) {
    toastError("Import failed", err);
  }
}
