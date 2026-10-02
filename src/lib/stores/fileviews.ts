/**
 * Per-file views (history & blame). Any component can open them:
 *
 *   import { showFileHistory, showBlame } from "../../lib/stores/fileviews";
 *   showFileHistory("src/main.ts");
 *   showBlame("src/main.ts");            // at HEAD
 *   showBlame("src/main.ts", "abc1234"); // at a commit
 *
 * Paths are repo-relative and refer to the active repo. The views render as
 * an overlay (`FileViewHost.svelte`, mounted in AppShell) closable with Esc.
 */
import { writable, get } from "svelte/store";
import { activeRepoPath } from "./repos";
import { commitGraph, selectedCommitOid } from "./graph";
import { toast } from "./toasts";

export type FileView =
  | { kind: "history"; path: string }
  | { kind: "blame"; path: string; rev?: string };

/** The open file view, or `null` when closed. */
export const fileView = writable<FileView | null>(null);

/** Open the file picker to choose a file for history or blame. */
export const filePickerFor = writable<"history" | "blame" | null>(null);

/** Show the commit history of `path` (follows renames). */
export function showFileHistory(path: string) {
  filePickerFor.set(null);
  fileView.set({ kind: "history", path });
}

/** Show line blame of `path`, at `rev` (any revision; default HEAD). */
export function showBlame(path: string, rev?: string) {
  filePickerFor.set(null);
  fileView.set({ kind: "blame", path, rev });
}

export function closeFileView() {
  fileView.set(null);
}

/** One-shot request for CommitGraph to scroll a commit into view. */
export const revealCommit = writable<string | null>(null);

/**
 * Close the file view, select `oid` in the commit graph (showing its diff)
 * and scroll to it. Returns false if the commit is not in the loaded graph.
 */
export function showInGraph(oid: string): boolean {
  const graph = get(commitGraph);
  const found = graph?.entries.some((e) => e.commit.oid === oid) ?? false;
  fileView.set(null);
  selectedCommitOid.set(oid);
  if (found) revealCommit.set(oid);
  else
    toast("info", "That commit is not in the loaded graph (it may be beyond the commit limit); showing its diff.", {
      title: "Commit not in graph",
    });
  return found;
}

/** Prompt for a tracked file, then open the given view for it. */
export function pickFileFor(kind: "history" | "blame") {
  filePickerFor.set(kind);
}

// Views belong to one repo; close them when the active tab changes.
let lastPath: string | null = null;
activeRepoPath.subscribe((p) => {
  if (p === lastPath) return;
  lastPath = p;
  fileView.set(null);
  filePickerFor.set(null);
});
