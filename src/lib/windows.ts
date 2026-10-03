/**
 * Multiple app windows: each has its own tabs (saved per window label by
 * the backend). The main window reopens the other saved windows at startup
 * and is the only one that checks for updates.
 */
import { getCurrentWindow, getAllWindows } from "@tauri-apps/api/window";
import * as tauri from "./tauri";
import { toastError } from "./stores/toasts";

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
    toastError("Could not open a window", err);
  }
}

/** Main window at startup: reopen the windows that were open last time. */
export async function restoreSavedWindows(): Promise<void> {
  if (!isMainWindow()) return;
  try {
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
    if (others.length > 0) await tauri.forgetWindowSession();
  } catch {
    // Keep the session if we can't tell.
  }
}
