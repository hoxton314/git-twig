/**
 * In-progress operation state (merge / rebase / cherry-pick / revert),
 * conflict resolution and history-rewriting actions (rebase, force push).
 *
 * Other components can open the dialogs via `openRebaseDialog(target)` /
 * `openInteractiveRebase(base)` / `openConflictResolver(file)`, and run the
 * operations via `continueOperation()` / `abortOperation()` / `forcePush()`.
 */
import { writable, get } from "svelte/store";
import { ask } from "@tauri-apps/plugin-dialog";
import type { BisectInfo, RepoOperationState } from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath } from "./repos";
import { workingStatus, refreshAll } from "./graph";
import { settings } from "./settings";
import { toast, toastError } from "./toasts";
import { tr } from "../i18n";

/** Operation state of the active repo (`null` until loaded). */
export const operationState = writable<RepoOperationState | null>(null);

/** Bisect progress while `operationState.kind === "bisect"` (else null). */
export const bisectState = writable<BisectInfo | null>(null);

/** Set while a continue/abort/skip/rebase/push is running. */
export const operationBusy = writable<string | null>(null);

let opGen = 0;

/** Reload the operation state for the active repo. */
export async function refreshOperation(path?: string) {
  const p = path ?? get(activeRepoPath);
  const gen = ++opGen;
  if (!p) {
    operationState.set(null);
    bisectState.set(null);
    return;
  }
  try {
    const st = await tauri.getOperationState(p);
    const bisect = st.kind === "bisect" ? await tauri.getBisectState(p).catch(() => null) : null;
    if (gen === opGen && get(activeRepoPath) === p) {
      operationState.set(st);
      bisectState.set(bisect);
    }
  } catch (err) {
    console.error("Failed to load operation state:", err);
    if (gen === opGen && get(activeRepoPath) === p) {
      operationState.set(null);
      bisectState.set(null);
    }
  }
}

// Every status refresh (after writes, on focus, on repo switch) also
// refreshes the operation state, so the banner follows the repo.
workingStatus.subscribe(() => {
  refreshOperation();
});

export function operationLabel(kind: string): string {
  switch (kind) {
    case "merge":
      return tr("operation.kind.merge");
    case "rebase":
      return tr("operation.kind.rebase");
    case "cherry_pick":
      return tr("operation.kind.cherryPick");
    case "revert":
      return tr("operation.kind.revert");
    case "am":
      return tr("operation.kind.am");
    case "bisect":
      return tr("operation.kind.bisect");
    default:
      return tr("operation.kind.other");
  }
}

async function confirmDestructive(text: string, title: string): Promise<boolean> {
  if (!get(settings).confirm_destructive_ops) return true;
  return ask(text, { title, kind: "warning" });
}

/** Run an operation command, toast the result and refresh. */
async function runOp(
  label: string,
  op: (path: string) => Promise<{ success: boolean; message: string }>,
  successMsg: string,
): Promise<boolean> {
  const path = get(activeRepoPath);
  if (!path || get(operationBusy)) return false;
  operationBusy.set(label);
  try {
    const res = await op(path);
    await refreshAll(path);
    await refreshOperation(path);
    const st = get(operationState);
    if (res.success) {
      if (st && st.kind !== "none") {
        // e.g. the rebase continued and stopped at the next conflict/edit.
        const name = operationLabel(st.kind);
        const text =
          st.conflicts.length > 0
            ? tr("operation.pausedAtConflicts", { name })
            : tr("operation.pausedAtStop", { name });
        toast("warning", text, {
          title: label,
        });
      } else {
        toast("success", successMsg);
      }
      return true;
    }
    if (st && st.kind !== "none" && st.conflicts.length > 0) {
      toast("warning", tr("operation.conflictedFiles", { count: st.conflicts.length }), {
        title: tr("operation.labelConflicts", { label }),
      });
    } else {
      toastError(tr("operation.failed", { label }), res.message || tr("operation.unknownError"));
    }
    return false;
  } catch (err) {
    toastError(tr("operation.failed", { label }), err);
    return false;
  } finally {
    operationBusy.set(null);
  }
}

// ── Continue / abort / skip ──────────────────────────────────────────

/** Continue dialog: open with the prepared commit message. */
export const continueDialog = writable<{ open: boolean; message: string }>({
  open: false,
  message: "",
});

/**
 * Continue the operation. Opens the commit message dialog when git is about
 * to create a commit from a prepared message (merge, cherry-pick, revert, a
 * resolved rebase conflict); otherwise continues right away.
 */
export function requestContinue() {
  const st = get(operationState);
  if (!st || st.kind === "none") return;
  if (st.conflicts.length > 0) {
    toast("warning", tr("operation.resolveFirst", { count: st.conflicts.length }), {
      title: tr("operation.continue", { name: operationLabel(st.kind).toLowerCase() }),
    });
    return;
  }
  const needsMessage =
    st.kind === "merge" ||
    st.kind === "cherry_pick" ||
    st.kind === "revert" ||
    (st.kind === "rebase" && !st.stopped_for_edit && st.message !== null);
  if (needsMessage) {
    continueDialog.set({ open: true, message: st.message ?? "" });
  } else {
    continueOperation(null);
  }
}

export function continueOperation(message: string | null): Promise<boolean> {
  const st = get(operationState);
  const label = tr("operation.continue", { name: operationLabel(st?.kind ?? "").toLowerCase() });
  return runOp(
    label,
    (p) => tauri.continueOperation(p, message),
    tr("operation.completed", { name: operationLabel(st?.kind ?? "") }),
  );
}

export async function abortOperation(): Promise<boolean> {
  const st = get(operationState);
  if (!st || st.kind === "none") return false;
  if (st.kind === "bisect") {
    const ok = await confirmDestructive(tr("operation.endBisectConfirm"), tr("operation.resetBisectTitle"));
    if (!ok) return false;
    return runOp(tr("operation.resetBisect"), tauri.bisectReset, tr("operation.bisectEnded"));
  }
  const name = operationLabel(st.kind).toLowerCase();
  const ok = await confirmDestructive(
    tr("operation.abortConfirm", { name }),
    tr("operation.abort", { name: operationLabel(st.kind) }),
  );
  if (!ok) return false;
  return runOp(
    tr("operation.abort", { name }),
    tauri.abortOperation,
    tr("operation.aborted", { name: operationLabel(st.kind) }),
  );
}

export async function skipOperation(): Promise<boolean> {
  const st = get(operationState);
  if (!st || !st.can_skip) return false;
  const ok = await confirmDestructive(
    st.current_subject
      ? tr("operation.skipConfirmNamed", { subject: st.current_subject })
      : tr("operation.skipConfirm"),
    tr("operation.skipCommitTitle"),
  );
  if (!ok) return false;
  return runOp(
    tr("operation.skipCommit"),
    tauri.skipOperation,
    tr("operation.completed", { name: operationLabel(st.kind) }),
  );
}

// ── Conflicts ────────────────────────────────────────────────────────

/** File shown in the conflict resolver dialog (null = closed). */
export const conflictResolverFile = writable<string | null>(null);

export function openConflictResolver(file?: string) {
  const st = get(operationState);
  const target = file ?? st?.conflicts[0]?.path ?? null;
  if (!target) {
    toast("info", tr("operation.noConflictedFiles"));
    return;
  }
  conflictResolverFile.set(target);
}

// ── Rebase ───────────────────────────────────────────────────────────

/** Rebase-onto dialog (target prefilled when opened from a branch/commit). */
export const rebaseDialog = writable<{ open: boolean; target: string }>({
  open: false,
  target: "",
});

/** Open "Rebase current branch onto…", optionally prefilled. */
export function openRebaseDialog(target = "") {
  if (!get(activeRepoPath)) return;
  rebaseDialog.set({ open: true, target });
}

/**
 * Interactive rebase dialog. `base` is exclusive, like `git rebase -i <base>`
 * (pass `${oid}^` to include `oid` itself); `null` means from the root.
 */
export const interactiveRebaseDialog = writable<{ open: boolean; base: string | null }>({
  open: false,
  base: "",
});

export function openInteractiveRebase(base: string | null = "") {
  if (!get(activeRepoPath)) return;
  interactiveRebaseDialog.set({ open: true, base });
}

/** Rebase the current branch onto `target` (no dialog). */
export async function rebaseOnto(target: string, autostash = true): Promise<boolean> {
  return runOp(
    tr("operation.rebase"),
    (p) => tauri.rebaseOnto(p, target, autostash),
    tr("operation.rebasedOnto", { target }),
  );
}

// ── Force push ───────────────────────────────────────────────────────

/** Force push the current branch with `--force-with-lease` (confirmed). */
export async function forcePush(): Promise<boolean> {
  const path = get(activeRepoPath);
  if (!path) return false;
  let branch: string | null = null;
  try {
    branch = (await tauri.getRepoInfo(path)).head_name;
  } catch (err) {
    toastError(tr("operation.forcePushFailed"), err);
    return false;
  }
  if (!branch) {
    toastError(tr("operation.forcePushFailed"), tr("operation.noBranch"));
    return false;
  }
  const ok = await confirmDestructive(
    tr("operation.forcePushConfirm", { branch }),
    tr("operation.forcePushTitle"),
  );
  if (!ok) return false;
  const b = branch;
  return runOp(tr("operation.forcePush"), (p) => tauri.forcePushWithLease(p, b), tr("operation.forcePushed", { branch: b }));
}
