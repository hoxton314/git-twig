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
import type { RepoOperationState } from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath } from "./repos";
import { workingStatus, refreshAll } from "./graph";
import { settings } from "./settings";
import { toast, toastError } from "./toasts";

/** Operation state of the active repo (`null` until loaded). */
export const operationState = writable<RepoOperationState | null>(null);

/** Set while a continue/abort/skip/rebase/push is running. */
export const operationBusy = writable<string | null>(null);

let opGen = 0;

/** Reload the operation state for the active repo. */
export async function refreshOperation(path?: string) {
  const p = path ?? get(activeRepoPath);
  const gen = ++opGen;
  if (!p) {
    operationState.set(null);
    return;
  }
  try {
    const st = await tauri.getOperationState(p);
    if (gen === opGen && get(activeRepoPath) === p) operationState.set(st);
  } catch (err) {
    console.error("Failed to load operation state:", err);
    if (gen === opGen && get(activeRepoPath) === p) operationState.set(null);
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
      return "Merge";
    case "rebase":
      return "Rebase";
    case "cherry_pick":
      return "Cherry-pick";
    case "revert":
      return "Revert";
    case "am":
      return "Patch application";
    case "bisect":
      return "Bisect";
    default:
      return "Operation";
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
        const what = st.conflicts.length > 0 ? "conflicts" : "a stop";
        toast("warning", `${operationLabel(st.kind)} paused at ${what}. Resolve and continue.`, {
          title: label,
        });
      } else {
        toast("success", successMsg);
      }
      return true;
    }
    if (st && st.kind !== "none" && st.conflicts.length > 0) {
      toast("warning", `${st.conflicts.length} conflicted file(s). Resolve them, then continue.`, {
        title: `${label}: conflicts`,
      });
    } else {
      toastError(`${label} failed`, res.message || "Unknown error");
    }
    return false;
  } catch (err) {
    toastError(`${label} failed`, err);
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
    toast("warning", `Resolve ${st.conflicts.length} conflicted file(s) first.`, {
      title: `Continue ${operationLabel(st.kind).toLowerCase()}`,
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
  const label = `Continue ${operationLabel(st?.kind ?? "").toLowerCase()}`;
  return runOp(label, (p) => tauri.continueOperation(p, message), `${operationLabel(st?.kind ?? "")} completed`);
}

export async function abortOperation(): Promise<boolean> {
  const st = get(operationState);
  if (!st || st.kind === "none") return false;
  const name = operationLabel(st.kind).toLowerCase();
  const ok = await confirmDestructive(
    `Abort the ${name}? Your branch returns to its state before the ${name} started and any conflict resolutions are lost.`,
    `Abort ${operationLabel(st.kind)}`,
  );
  if (!ok) return false;
  return runOp(`Abort ${name}`, tauri.abortOperation, `${operationLabel(st.kind)} aborted`);
}

export async function skipOperation(): Promise<boolean> {
  const st = get(operationState);
  if (!st || !st.can_skip) return false;
  const subject = st.current_subject ? ` "${st.current_subject}"` : "";
  const ok = await confirmDestructive(
    `Skip the current commit${subject}? Its changes will not be applied.`,
    "Skip Commit",
  );
  if (!ok) return false;
  return runOp("Skip commit", tauri.skipOperation, `${operationLabel(st.kind)} completed`);
}

// ── Conflicts ────────────────────────────────────────────────────────

/** File shown in the conflict resolver dialog (null = closed). */
export const conflictResolverFile = writable<string | null>(null);

export function openConflictResolver(file?: string) {
  const st = get(operationState);
  const target = file ?? st?.conflicts[0]?.path ?? null;
  if (!target) {
    toast("info", "There are no conflicted files.");
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
    "Rebase",
    (p) => tauri.rebaseOnto(p, target, autostash),
    `Rebased onto ${target}`,
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
    toastError("Force push failed", err);
    return false;
  }
  if (!branch) {
    toastError("Force push failed", "No branch is checked out.");
    return false;
  }
  const ok = await confirmDestructive(
    `Force push "${branch}"?\n\nThis overwrites the remote branch with your local history. ` +
      `--force-with-lease refuses if someone else pushed commits you have not seen.`,
    "Force Push (with lease)",
  );
  if (!ok) return false;
  const b = branch;
  return runOp("Force push", (p) => tauri.forcePushWithLease(p, b), `Force pushed ${b}`);
}
