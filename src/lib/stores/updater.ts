/**
 * Updater state shared by the update banner and Settings > About.
 *
 * `updater_supported` is false for installs owned by a system package
 * manager (AUR, dev builds); then nothing is ever checked or installed.
 */
import { writable, get } from "svelte/store";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { updaterSupported } from "../tauri";
import { settings, updateSettings } from "./settings";

export type UpdaterStatus =
  | "unknown" // support not determined yet
  | "unsupported" // managed by a package manager / dev build
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "error";

export interface UpdaterState {
  status: UpdaterStatus;
  version: string;
  notes: string;
  /** Bytes downloaded / total (0 if unknown). */
  downloaded: number;
  total: number;
  error: string;
  /** Unix ms of the last completed check. */
  lastChecked: number | null;
  /** Whether the banner should be shown for the available update. */
  bannerVisible: boolean;
}

export const updater = writable<UpdaterState>({
  status: "unknown",
  version: "",
  notes: "",
  downloaded: 0,
  total: 0,
  error: "",
  lastChecked: null,
  bannerVisible: false,
});

let pending: Update | null = null;
let supportPromise: Promise<boolean> | null = null;

function patch(p: Partial<UpdaterState>) {
  updater.update((s) => ({ ...s, ...p }));
}

/** Resolve (once) whether this install may self-update. */
export function ensureUpdaterSupport(): Promise<boolean> {
  if (!supportPromise) {
    supportPromise = updaterSupported()
      .catch(() => false)
      .then((ok) => {
        if (get(updater).status === "unknown") patch({ status: ok ? "idle" : "unsupported" });
        return ok;
      });
  }
  return supportPromise;
}

/**
 * Check for an update. Automatic checks stay silent about errors and respect
 * the "skip this version" choice; manual checks always report the result.
 */
export async function checkForUpdates(manual: boolean): Promise<void> {
  if (!(await ensureUpdaterSupport())) return;
  const s = get(updater).status;
  if (s === "checking" || s === "downloading") return;
  patch({ status: "checking", error: "" });
  try {
    const update = await check();
    if (pending && pending !== update) pending.close().catch(() => {});
    pending = update;
    if (!update) {
      patch({ status: "up-to-date", version: "", notes: "", lastChecked: Date.now(), bannerVisible: false });
      return;
    }
    const skipped = get(settings).skipped_update_version;
    patch({
      status: "available",
      version: update.version,
      notes: update.body ?? "",
      lastChecked: Date.now(),
      bannerVisible: manual || skipped !== update.version,
    });
  } catch (e) {
    console.error("Update check failed:", e);
    patch({ status: "error", error: String(e), lastChecked: Date.now(), bannerVisible: false });
  }
}

export async function installUpdate(): Promise<void> {
  if (get(updater).status === "downloading") return; // double-click guard
  patch({ status: "downloading", downloaded: 0, total: 0, error: "", bannerVisible: true });
  try {
    const update = pending ?? (await check());
    if (!update) {
      patch({ status: "up-to-date", bannerVisible: false });
      return;
    }
    pending = update;
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        patch({ total: event.data.contentLength ?? 0 });
      } else if (event.event === "Progress") {
        updater.update((s) => ({ ...s, downloaded: s.downloaded + event.data.chunkLength }));
      }
    });
    await relaunch();
  } catch (e) {
    patch({ status: "error", error: String(e), bannerVisible: true });
  }
}

/** Hide the banner until the next check. */
export function dismissUpdateBanner() {
  const s = get(updater);
  patch({ bannerVisible: false, status: s.status === "error" ? (pending ? "available" : "idle") : s.status });
}

/** Never prompt again for the currently offered version. */
export function skipUpdateVersion() {
  const v = get(updater).version;
  if (v) updateSettings({ skipped_update_version: v });
  patch({ bannerVisible: false });
}
