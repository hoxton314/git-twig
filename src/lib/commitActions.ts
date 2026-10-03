/**
 * Commit-level git actions shared by the commit context menu, the command
 * palette (via keybinding actions) and the undo history panel. Each action
 * confirms when appropriate, reports through toasts, refreshes the repo, and
 * offers "Undo" for operations that move HEAD.
 */
import { get } from "svelte/store";
import { ask } from "@tauri-apps/plugin-dialog";
import * as tauri from "./tauri";
import type { CommitOpResult, PickManyResult, ResetMode } from "./types/git";
import { refreshAll } from "./stores/graph";
import { settings } from "./stores/settings";
import { toast, toastError } from "./stores/toasts";
import { tr } from "./i18n";

const short = (oid: string) => oid.slice(0, 7);

/** Confirmation that respects the "confirm destructive operations" setting. */
export async function confirmDestructive(text: string, title: string, okLabel = tr("common.ok")): Promise<boolean> {
  if (!get(settings).confirm_destructive_ops) return true;
  return ask(text, { title, kind: "warning", okLabel, cancelLabel: tr("common.cancel") });
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
      toast("error", tr("commits.copyFailed", { what }));
      return;
    }
  }
  toast("success", tr("commits.copied", { what }), { duration: 2000 });
}

function stashNote(r: CommitOpResult): string {
  return r.stash_oid ? ` ${tr("commits.stashNote")}` : "";
}

/** Run an operation that may move HEAD; refresh and report. */
async function runHeadOp<T extends CommitOpResult>(
  path: string,
  title: string,
  op: () => Promise<T>,
  onSuccess: (r: T) => void,
  /** Replaces the default error toast for a failed, non-conflicted result. */
  onFailure?: (r: T) => void,
): Promise<T | null> {
  try {
    const r = await op();
    await refreshAll(path);
    if (r.success) onSuccess(r);
    else if (onFailure) onFailure(r);
    else if (!r.conflicted) toast("error", r.message.trim() || tr("commits.gitError"), { title });
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
      tr("commits.undoFailed"),
      async () => {
        if (r.previous_branch) {
          const res = await tauri.checkoutBranch(path, r.previous_branch);
          return { ...r, success: res.success, message: res.message, stash_oid: null };
        }
        return tauri.checkoutCommit(path, prev);
      },
      () => toast("success", tr("commits.backOn", { target: r.previous_branch ?? short(prev) })),
    );
    return;
  }
  await runHeadOp(
    path,
    tr("commits.undoFailed"),
    () => tauri.resetToCommit(path, prev, how),
    (u) =>
      toast("success", tr("commits.restoredTo", { target: r.previous_branch ?? "HEAD", sha: short(prev) }) + stashNote(u)),
  );
}

export async function checkoutCommitAction(path: string, oid: string) {
  const ok = await confirmDestructive(
    tr("commits.checkoutConfirm", { sha: short(oid) }),
    tr("commits.checkoutTitle"),
    tr("commits.checkoutOk"),
  );
  if (!ok) return;
  await runHeadOp(path, tr("commits.checkoutFailed"), () => tauri.checkoutCommit(path, oid), (r) =>
    toast("success", tr("commits.detachedAt", { sha: short(oid) }), {
      action: r.previous_head ? { label: tr("commits.undo"), run: () => undoTo(path, r, "checkout") } : undefined,
    }),
  );
}

async function applyCommit(path: string, oid: string, kind: "cherry-pick" | "revert") {
  const revert = kind === "revert";
  const r = await runHeadOp(
    path,
    tr(revert ? "commits.revertFailed" : "commits.cherryPickFailed"),
    () => (revert ? tauri.revertCommit(path, oid) : tauri.cherryPickCommit(path, oid)),
    (res) =>
      toast("success", tr(revert ? "commits.reverted" : "commits.cherryPicked", { sha: short(oid) }), {
        action: res.previous_head
          ? { label: tr("commits.undo"), run: () => undoTo(path, res, "keep") }
          : undefined,
      }),
  );
  if (r?.conflicted) {
    toast(
      "warning",
      tr(revert ? "commits.revertConflicts" : "commits.cherryPickConflicts", { sha: short(oid) }),
      { title: tr("commits.conflicts"), duration: 0 },
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
  const undo = (res: PickManyResult) =>
    res.previous_head ? { label: tr("commits.undo"), run: () => undoTo(path, res, "keep") } : undefined;
  await runHeadOp(
    path,
    tr("commits.cherryPickFailed"),
    () => tauri.cherryPickCommits(path, ordered),
    (res) => {
      const skipped = res.skipped.length;
      const text =
        skipped > 0
          ? tr("commits.cherryPickedManySkipped", { count: res.picked, skipped })
          : tr("commits.cherryPickedMany", { count: res.picked });
      toast("success", text, {
        action: undo(res),
      });
    },
    (res) => {
      const total = ordered.length - res.skipped.length;
      const counts = { picked: res.picked, total, count: total };
      if (res.conflicted) {
        toast("warning", tr("commits.pickRangeConflicts", counts), {
          title: tr("commits.conflicts"),
          duration: 0,
        });
      } else if (res.empty) {
        toast("warning", tr("commits.pickRangeEmpty", counts), {
          title: tr("commits.nothingToApply"),
          duration: 0,
        });
      } else {
        // The backend ended the sequence: what was applied stays applied.
        const kept = res.picked > 0 ? ` ${tr("commits.pickRangeKept", counts)}` : "";
        toast("error", `${res.message.trim() || tr("commits.gitError")}${kept}`, {
          title: tr("commits.cherryPickFailed"),
          duration: 0,
          action: res.picked > 0 ? undo(res) : undefined,
        });
      }
    },
  );
}
export const revertAction = (path: string, oid: string) => applyCommit(path, oid, "revert");

const RESET_KEYS = {
  soft: { text: "commits.resetSoftText", mode: "commits.modeSoft" },
  mixed: { text: "commits.resetMixedText", mode: "commits.modeMixed" },
  hard: { text: "commits.resetHardText", mode: "commits.modeHard" },
} as const;

export async function resetAction(
  path: string,
  oid: string,
  mode: Exclude<ResetMode, "keep">,
  branch: string | null,
) {
  const target = branch ?? "HEAD";
  const params = { target, sha: short(oid), mode: tr(RESET_KEYS[mode].mode) };
  const ok = await confirmDestructive(
    tr("commits.resetConfirm", { ...params, details: tr(RESET_KEYS[mode].text) }),
    tr(mode === "hard" ? "commits.hardResetTitle" : "commits.resetTitle"),
    tr("commits.resetOk"),
  );
  if (!ok) return;
  await runHeadOp(path, tr("commits.resetFailed"), () => tauri.resetToCommit(path, oid, mode), (r) =>
    toast("success", tr("commits.resetDone", params) + stashNote(r), {
      duration: 8000,
      action: r.previous_head ? { label: tr("commits.undo"), run: () => undoTo(path, r, mode) } : undefined,
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
    tr("commits.restoreFailed"),
    () => tauri.restoreHead(path, oid, branch, autoStash),
    (res) =>
      toast("success", tr("commits.restoredHead", { target: branch ?? short(oid) }) + stashNote(res), {
        duration: 8000,
        action: res.previous_head
          ? {
              label: tr("commits.undo"),
              run: () =>
                runHeadOp(
                  path,
                  tr("commits.undoFailed"),
                  () => tauri.restoreHead(path, res.previous_head as string, res.previous_branch, true),
                  (u) =>
                    toast(
                      "success",
                      tr("commits.backTo", { target: res.previous_branch ?? short(res.previous_head as string) }) +
                        stashNote(u),
                    ),
                ).then(() => {}),
            }
          : undefined,
      }),
  );
  return !!r?.success;
}
