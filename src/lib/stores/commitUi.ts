/**
 * UI state for commit-level actions: the commit context menu, the create
 * branch/tag dialogs, the undo history panel, and "reveal commit in graph"
 * requests (from the tags list or undo history).
 */
import { get, writable } from "svelte/store";
import { commitGraph, selectedCommitOid } from "./graph";
import { toast } from "./toasts";

/** Open commit context menu (viewport coordinates). */
export const commitMenu = writable<{ oid: string; x: number; y: number } | null>(null);

/** "Create branch here…" dialog target. */
/** Commits the squash dialog is open for (null = closed). */
export const squashTarget = writable<string[] | null>(null);

export const createBranchTarget = writable<{ oid: string; label: string } | null>(null);

/** "Create tag here…" dialog target. */
export const createTagTarget = writable<{ oid: string; label: string } | null>(null);

/** Whether the undo history (reflog) panel is open. */
export const undoHistoryOpen = writable(false);

/** Latest request to scroll the graph to a commit; `seq` makes repeats fire. */
export const revealRequest = writable<{ oid: string; seq: number } | null>(null);

let revealSeq = 0;

/**
 * Select `oid` in the graph and scroll it into view. Returns false (and tells
 * the user) when the commit isn't part of the loaded history.
 */
export function revealCommit(oid: string): boolean {
  const graph = get(commitGraph);
  if (!graph?.entries.some((e) => e.commit.oid === oid)) {
    toast("info", "That commit is not in the loaded history (raise “max commits” in Settings to load more).");
    return false;
  }
  selectedCommitOid.set(oid);
  revealRequest.set({ oid, seq: ++revealSeq });
  return true;
}

/** Right-click handler for a commit row. */
export function openCommitMenu(e: MouseEvent, oid: string) {
  e.preventDefault();
  selectedCommitOid.set(oid);
  commitMenu.set({ oid, x: e.clientX, y: e.clientY });
}
