/**
 * Serialized write queue for the staging panel.
 *
 * Concurrent `git add`/`reset`/`checkout` calls race on .git/index.lock and
 * fail, and overlapping status refreshes can resolve out of order and show
 * stale lists — so every index-modifying operation goes through here.
 */
import { get } from "svelte/store";
import { ask } from "@tauri-apps/plugin-dialog";
import { refreshStatus } from "../../lib/stores/graph";
import { settings } from "../../lib/stores/settings";
import { toast, toastError } from "../../lib/stores/toasts";

let writeQueue: Promise<unknown> = Promise.resolve();

/**
 * Run a write command (then refresh status) and surface any failure as a
 * toast instead of swallowing it. Returns true on success.
 */
export function runWrite(
  op: () => Promise<{ success: boolean; message: string }>,
  title: string,
): Promise<boolean> {
  const task = writeQueue.then(async () => {
    let ok = false;
    try {
      const result = await op();
      if (!result.success) {
        toast("error", result.message.trim() || "Unknown error", { title });
      } else {
        ok = true;
      }
    } catch (err) {
      toastError(title, err);
    }
    await refreshStatus();
    return ok;
  });
  writeQueue = task;
  return task;
}

/** Resolve once every queued write has finished. */
export function waitForWrites(): Promise<unknown> {
  return writeQueue;
}

/** Confirmation for destructive actions, honoring the user's setting. */
export function confirmDestructive(text: string, title: string): Promise<boolean> {
  if (!get(settings).confirm_destructive_ops) return Promise.resolve(true);
  return ask(text, { title, kind: "warning" });
}
