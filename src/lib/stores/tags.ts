import { get, writable } from "svelte/store";
import type { TagInfo } from "../types/git";
import * as tauri from "../tauri";
import { activeRepoPath } from "./repos";

/** Tags of the active repo, newest first. */
export const tags = writable<TagInfo[]>([]);

/** True until the first tag load for the active repo finishes. */
export const tagsLoading = writable(false);

activeRepoPath.subscribe(() => {
  tags.set([]);
});

let gen = 0;

/** Reload tags for `path` (defaults to the active repo). */
export async function refreshTags(path?: string) {
  const p = path ?? get(activeRepoPath);
  if (!p) return;
  const my = ++gen;
  tagsLoading.set(true);
  try {
    const list = await tauri.getTags(p);
    if (my === gen && get(activeRepoPath) === p) tags.set(list);
  } catch (err) {
    console.error("Failed to load tags:", err);
    if (my === gen && get(activeRepoPath) === p) tags.set([]);
  } finally {
    if (my === gen) tagsLoading.set(false);
  }
}
