import { writable } from "svelte/store";
import type { CommitGraph, BranchInfo, DiffFile } from "../types/git";

/** Commit graph for the active repo. */
export const commitGraph = writable<CommitGraph | null>(null);

/** Whether the graph is currently loading. */
export const graphLoading = writable(false);

/** Branches for the active repo. */
export const branches = writable<BranchInfo[]>([]);

/** The currently selected commit OID (for showing its diff). */
export const selectedCommitOid = writable<string | null>(null);

/** Diff files for the selected commit. */
export const selectedDiff = writable<DiffFile[]>([]);

/** Whether the diff is currently loading. */
export const diffLoading = writable(false);
