/**
 * App-shell actions shared by the tab bar, home screen, settings screens and
 * command palette.
 */
import { get } from "svelte/store";
import { open, save, ask } from "@tauri-apps/plugin-dialog";
import { addRepo } from "./stores/repos";
import { settings, globalSettings, DEFAULT_SETTINGS, flushSettings, replaceGlobalSettings } from "./stores/settings";
import { toast, toastError } from "./stores/toasts";
import * as tauri from "./tauri";
import { tr } from "./i18n";

/** Pick a folder and open it as a repository tab. */
export async function openRepoWithDialog(): Promise<void> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: tr("app.openRepoDialogTitle"),
    defaultPath: get(settings).default_repo_dir ?? undefined,
  });
  if (!selected || Array.isArray(selected)) return;
  try {
    addRepo(await tauri.openRepo(selected));
  } catch (err) {
    toastError(tr("tabs.openFailed"), err);
  }
}

export async function openSettingsFolder(): Promise<void> {
  try {
    await flushSettings();
    await tauri.openSettingsFolder();
  } catch (err) {
    toastError(tr("app.openSettingsFolderFailed"), err);
  }
}

/** Save the current settings (never secrets) to a user-chosen JSON file. */
export async function exportSettingsToFile(): Promise<void> {
  const path = await save({
    title: tr("app.exportSettingsTitle"),
    defaultPath: "twig-settings.json",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path) return;
  try {
    // Global settings only: per-repository overrides must not leak into an import.
    await tauri.exportSettings(path, get(globalSettings));
    toast("success", tr("app.settingsExported", { path }));
  } catch (err) {
    toastError(tr("app.exportFailed"), err);
  }
}

/** Replace the current settings with ones read from a JSON file. */
export async function importSettingsFromFile(): Promise<void> {
  const path = await open({
    title: tr("app.importSettingsTitle"),
    multiple: false,
    directory: false,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path || Array.isArray(path)) return;
  try {
    const imported = await tauri.importSettings(path);
    const confirmed = !get(settings).confirm_destructive_ops || await ask(tr("app.importConfirm"), {
      title: tr("app.importConfirmTitle"),
      kind: "warning",
      okLabel: tr("app.import"),
      cancelLabel: tr("common.cancel"),
    });
    if (!confirmed) return;
    replaceGlobalSettings({
      ...DEFAULT_SETTINGS,
      ...imported,
      keybinding_overrides: imported.keybinding_overrides ?? {},
    });
    toast("success", tr("app.settingsImported"));
  } catch (err) {
    toastError(tr("app.importFailed"), err);
  }
}

/** Open a terminal in `repoPath` (flushes settings so a just-edited command is used). */
export async function openRepoInTerminal(repoPath: string): Promise<void> {
  try {
    await flushSettings();
    await tauri.openInTerminal(repoPath);
  } catch (err) {
    toastError(tr("app.openTerminalFailed"), err);
  }
}

/** Open `repoPath` (or `file` in it) in the configured editor. */
export async function openRepoInEditor(repoPath: string, file: string | null = null): Promise<void> {
  try {
    await flushSettings();
    await tauri.openInEditor(repoPath, file);
  } catch (err) {
    toastError(tr("app.openEditorFailed"), err);
  }
}

/**
 * Open paths from the command line as tabs (a path inside a repository opens
 * that repository), switching to the last one. Errors are reported per path.
 */
export async function openPathsAsTabs(paths: string[]): Promise<void> {
  for (const p of paths) {
    try {
      addRepo(await tauri.openRepo(p));
    } catch (err) {
      toastError(tr("app.openPathFailed", { path: p }), err);
    }
  }
}
