/**
 * Multiple app windows: each has its own tabs (saved per window label by
 * the backend). The main window reopens the other saved windows at startup
 * and is the only one that checks for updates.
 */
import { getCurrentWindow, getAllWindows } from "@tauri-apps/api/window";
import * as tauri from "./tauri";
import { toastError } from "./stores/toasts";
import { stopSessionSaving } from "./stores/repos";
import { tr } from "./i18n";

export const MAIN_WINDOW = "main";

export function windowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return MAIN_WINDOW;
  }
}

export function isMainWindow(): boolean {
  return windowLabel() === MAIN_WINDOW;
}

/** Open a new, empty window. */
export async function openNewWindow(): Promise<void> {
  try {
    await tauri.openNewWindow(null);
  } catch (err) {
    toastError(tr("windows.openFailed"), err);
  }
}

/**
 * Main window at startup: reopen the windows that were open last time, or,
 * when tabs aren't restored, forget them.
 */
export async function restoreSavedWindows(restore: boolean): Promise<void> {
  if (!isMainWindow()) return;
  try {
    if (!restore) {
      await tauri.forgetOtherWindows();
      return;
    }
    for (const label of await tauri.savedWindows()) await tauri.openNewWindow(label);
  } catch (err) {
    console.error("restoring windows:", err);
  }
}

/**
 * This window is closing. If others stay open it won't be reopened next
 * time, so its saved tabs are dropped; closing the last window keeps them.
 */
export async function onWindowClosing(): Promise<void> {
  try {
    const others = (await getAllWindows()).filter((w) => w.label !== windowLabel());
    if (others.length > 0) {
      // A tab save still pending would write this window back after the forget.
      stopSessionSaving();
      await tauri.forgetWindowSession();
    }
  } catch {
    // Keep the session if we can't tell.
  }
}
