/**
 * Commit-level git actions shared by the commit context menu, the command
 * palette (via keybinding actions) and the undo history panel. Each action
 * confirms when appropriate, reports through toasts, refreshes the repo, and
 * offers "Undo" for operations that move HEAD.
 */
import { get } from "svelte/store";
import { ask } from "@tauri-apps/plugin-dialog";
import * as tauri from "./tauri";
import type { CommitOpResult, ResetMode } from "./types/git";
import { refreshAll } from "./stores/graph";
import { settings } from "./stores/settings";
import { toast, toastError } from "./stores/toasts";

const short = (oid: string) => oid.slice(0, 7);

/** Confirmation that respects the "confirm destructive operations" setting. */
export async function confirmDestructive(text: string, title: string, okLabel = "OK"): Promise<boolean> {
  if (!get(settings).confirm_destructive_ops) return true;
  return ask(text, { title, kind: "warning", okLabel, cancelLabel: "Cancel" });
}

/** Copy text to the clipboard, falling back to execCommand for older WebKit. */
export async function copyText(text: string, what: string) {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      throw new Error("Clipboard API unavailable");
    }
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    if (!ok) {
      toast("error", `Could not copy ${what} to the clipboard.`);
      return;
    }
  }
  toast("success", `Copied ${what}`, { duration: 2000 });
}

function stashNote(r: CommitOpResult): string {
  return r.stash_oid ? " Your uncommitted changes were saved to the stash list." : "";
}

/** Run an operation that may move HEAD; refresh and report. */
async function runHeadOp<T extends CommitOpResult>(
  path: string,
  title: string,
  op: () => Promise<T>,
  onSuccess: (r: T) => void,
): Promise<T | null> {
  try {
    const r = await op();
    await refreshAll(path);
    if (r.success) onSuccess(r);
    else if (!r.conflicted) toast("error", r.message.trim() || "Git reported an error.", { title });
    return r;
  } catch (err) {
    await refreshAll(path);
    toastError(title, err);
    return null;
  }
}

/** Undo by restoring HEAD to where an operation started from. */
async function undoTo(path: string, r: CommitOpResult, how: "checkout" | ResetMode) {
  const prev = r.previous_head;
  if (!prev) return;
  if (how === "checkout") {
    await runHeadOp(
      path,
      "Undo Failed",
      async () => {
        if (r.previous_branch) {
          const res = await tauri.checkoutBranch(path, r.previous_branch);
          return { ...r, success: res.success, message: res.message, stash_oid: null };
        }
        return tauri.checkoutCommit(path, prev);
      },
      () => toast("success", `Back on ${r.previous_branch ?? short(prev)}`),
    );
    return;
  }
  await runHeadOp(
    path,
    "Undo Failed",
    () => tauri.resetToCommit(path, prev, how),
    (u) => toast("success", `Restored ${r.previous_branch ?? "HEAD"} to ${short(prev)}.${stashNote(u)}`),
  );
}

export async function checkoutCommitAction(path: string, oid: string) {
  const ok = await confirmDestructive(
    `Check out ${short(oid)} with a detached HEAD?\n\nNew commits made there won't belong to any branch unless you create one.`,
    "Checkout Commit",
    "Checkout",
  );
  if (!ok) return;
  await runHeadOp(path, "Checkout Failed", () => tauri.checkoutCommit(path, oid), (r) =>
    toast("success", `HEAD is now detached at ${short(oid)}`, {
      action: r.previous_head ? { label: "Undo", run: () => undoTo(path, r, "checkout") } : undefined,
    }),
  );
}

async function applyCommit(path: string, oid: string, kind: "cherry-pick" | "revert") {
  const verb = kind === "revert" ? "Revert" : "Cherry-pick";
  const r = await runHeadOp(
    path,
    `${verb} Failed`,
    () => (kind === "revert" ? tauri.revertCommit(path, oid) : tauri.cherryPickCommit(path, oid)),
    (res) =>
      toast("success", `${verb === "Revert" ? "Reverted" : "Cherry-picked"} ${short(oid)}`, {
        action: res.previous_head
          ? { label: "Undo", run: () => undoTo(path, res, "keep") }
          : undefined,
      }),
  );
  if (r?.conflicted) {
    toast(
      "warning",
      `${verb} of ${short(oid)} stopped with conflicts. Resolve them, then continue or abort from the banner above the graph.`,
      { title: "Conflicts", duration: 0 },
    );
  }
}

export const cherryPickAction = (path: string, oid: string) => applyCommit(path, oid, "cherry-pick");

/**
 * Cherry-pick several commits. `oids` are in graph order (newest first);
 * they're applied oldest first.
 */
export async function cherryPickRangeAction(path: string, oids: string[]) {
  if (oids.length === 1) return cherryPickAction(path, oids[0]);
  const ordered = [...oids].reverse();
  const r = await runHeadOp(
    path,
    "Cherry-pick Failed",
    () => tauri.cherryPickCommits(path, ordered),
    (res) => {
      const skipped = res.skipped.length;
      const note = skipped > 0 ? ` (${skipped} already on HEAD, skipped)` : "";
      toast("success", `Cherry-picked ${res.picked} commit${res.picked === 1 ? "" : "s"}${note}`, {
        action: res.previous_head ? { label: "Undo", run: () => undoTo(path, res, "keep") } : undefined,
      });
    },
  );
  if (r?.conflicted) {
    const total = ordered.length - r.skipped.length;
    toast(
      "warning",
      `Cherry-pick stopped with conflicts after ${r.picked} of ${total} commits. Resolve them, then continue (the rest of the range follows), skip or abort from the banner above the graph.`,
      { title: "Conflicts", duration: 0 },
    );
  }
}
export const revertAction = (path: string, oid: string) => applyCommit(path, oid, "revert");

const RESET_TEXT: Record<Exclude<ResetMode, "keep">, string> = {
  soft: "Changes from later commits stay staged.",
  mixed: "Changes from later commits stay in the working tree, unstaged.",
  hard: "The working tree and index are reset to that commit. Uncommitted changes to tracked files are saved to the stash list first.",
};

export async function resetAction(
  path: string,
  oid: string,
  mode: Exclude<ResetMode, "keep">,
  branch: string | null,
) {
  const target = branch ?? "HEAD";
  const ok = await confirmDestructive(
    `Reset ${target} to ${short(oid)} (${mode})?\n\n${RESET_TEXT[mode]}`,
    mode === "hard" ? "Hard Reset" : "Reset",
    "Reset",
  );
  if (!ok) return;
  await runHeadOp(path, "Reset Failed", () => tauri.resetToCommit(path, oid, mode), (r) =>
    toast("success", `Reset ${target} to ${short(oid)} (${mode}).${stashNote(r)}`, {
      duration: 8000,
      action: r.previous_head ? { label: "Undo", run: () => undoTo(path, r, mode) } : undefined,
    }),
  );
}

/** Restore HEAD to a reflog entry (used by the undo history panel). */
export async function restoreHeadAction(
  path: string,
  oid: string,
  branch: string | null,
  autoStash: boolean,
): Promise<boolean> {
  const r = await runHeadOp(
    path,
    "Restore Failed",
    () => tauri.restoreHead(path, oid, branch, autoStash),
    (res) =>
      toast("success", `Restored to ${branch ?? short(oid)}.${stashNote(res)}`, {
        duration: 8000,
        action: res.previous_head
          ? {
              label: "Undo",
              run: () =>
                runHeadOp(
                  path,
                  "Undo Failed",
                  () => tauri.restoreHead(path, res.previous_head as string, res.previous_branch, true),
                  (u) => toast("success", `Back to ${res.previous_branch ?? short(res.previous_head as string)}.${stashNote(u)}`),
                ).then(() => {}),
            }
          : undefined,
      }),
  );
  return !!r?.success;
}
